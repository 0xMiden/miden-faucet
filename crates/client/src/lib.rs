use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Context;
use miden_client::account::component::{BasicFungibleFaucet, FungibleFaucetExt};
use miden_client::account::{Account, AccountId, NetworkId};
use miden_client::asset::FungibleAsset;
use miden_client::auth::AuthSecretKey;
use miden_client::builder::ClientBuilder;
use miden_client::crypto::RpoRandomCoin;
use miden_client::keystore::FilesystemKeyStore;
use miden_client::note::{NoteError, create_p2id_note};
use miden_client::rpc::Endpoint;
use miden_client::transaction::{
    LocalTransactionProver, OutputNote, TransactionId, TransactionProver, TransactionRequestBuilder,
};
use miden_client::{Client, ClientError, Felt, RemoteTransactionProver, Word};
use rand::rngs::StdRng;
use rand::{Rng, rng};
use serde::Serialize;
use tokio::sync::mpsc::Receiver;
use tracing::{error, info, instrument, warn};
use url::Url;
pub mod requests;
pub mod types;

use crate::requests::{MintError, MintRequest, MintResponse, MintResponseSender};

// FAUCET CLIENT
// ================================================================================================

/// The faucet's account ID and network ID.
///
/// Used as a type safety mechanism to avoid confusion with user account IDs, and allows us to
/// implement traits.
#[derive(Clone, Copy)]
pub struct FaucetId {
    pub account_id: AccountId,
    pub network_id: NetworkId,
}

impl FaucetId {
    pub fn new(account_id: AccountId, network_id: NetworkId) -> Self {
        Self { account_id, network_id }
    }
}

impl Serialize for FaucetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.account_id.to_bech32(self.network_id))
    }
}

/// Stores the current faucet state and handles minting requests.
pub struct Faucet {
    id: FaucetId,
    decimals: u8,
    client: Client<FilesystemKeyStore<StdRng>>,
    tx_prover: Arc<dyn TransactionProver>,
    issuance: Arc<AtomicU64>,
    max_supply: u64,
}

/// Configuration for initializing and loading a faucet.
pub struct FaucetConfig {
    pub account_id: AccountId,
    pub store_path: String,
    pub keystore_path: String,
    pub node_endpoint: Endpoint,
    pub timeout: Duration,
    /// If a remote transaction prover url is provided, it is used to prove transactions.
    /// Otherwise, a local transaction prover is used.
    pub remote_tx_prover_url: Option<Url>,
}

impl Faucet {
    /// Initializes a new faucet client, creating the keystore and the database.
    #[instrument(name = "faucet.init", skip_all)]
    pub async fn init(config: &FaucetConfig, secret_key: &AuthSecretKey) -> anyhow::Result<()> {
        let keystore = FilesystemKeyStore::<StdRng>::new(config.keystore_path.clone().into())
            .context("failed to create keystore")?;
        keystore.add_key(secret_key)?;

        ClientBuilder::<FilesystemKeyStore<StdRng>>::new()
            .tonic_rpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .authenticator(Arc::new(keystore))
            .sqlite_store(&config.store_path)
            .build()
            .await?;
        Ok(())
    }

    /// Adds an account to the faucet database.
    #[instrument(name = "faucet.import_account", skip_all)]
    pub async fn import_account(
        config: &FaucetConfig,
        account: Account,
        account_seed: Word,
    ) -> Result<(), ClientError> {
        let mut client: Client<FilesystemKeyStore<StdRng>> = ClientBuilder::new()
            .tonic_rpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .sqlite_store(&config.store_path)
            .build()
            .await?;

        client.add_account(&account, Some(account_seed), false).await
    }

    /// Loads the faucet.
    ///
    /// The account is loaded from the local store. If it is not tracked, it is fetched from the
    /// node and added to the local store.
    #[instrument(name = "faucet.load", skip_all)]
    pub async fn load(config: FaucetConfig) -> anyhow::Result<Self> {
        let mut client = ClientBuilder::new()
            .tonic_rpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .filesystem_keystore(&config.keystore_path)
            .sqlite_store(&config.store_path)
            .build()
            .await
            .context("failed to build client")?;

        let record = match client.get_account(config.account_id).await? {
            Some(record) => {
                info!("Loaded account from local db");
                record
            },
            None => {
                // If the account is not tracked, we try to fetch it from the node.
                client
                    .import_account_by_id(config.account_id)
                    .await
                    .context("failed to fetch account from the node")?;
                info!("Fetched faucet account state from the node");
                client
                    .get_account(config.account_id)
                    .await?
                    .context("failed to load fetched account")?
            },
        };
        let account = record.account();
        info!(
            commitment = %account.commitment(),
            nonce = %account.nonce(),
        );

        client.ensure_genesis_in_place().await?;

        let tx_prover: Arc<dyn TransactionProver> = match config.remote_tx_prover_url {
            Some(url) => Arc::new(RemoteTransactionProver::new(url)),
            None => Arc::new(LocalTransactionProver::default()),
        };
        let id = FaucetId::new(config.account_id, config.node_endpoint.to_network_id()?);
        let faucet = BasicFungibleFaucet::try_from(account)?;
        let decimal_divisor = 10u64.pow(faucet.decimals().into());
        let issuance = account.get_token_issuance()?.as_int() / decimal_divisor;
        let max_supply = faucet.max_supply().as_int() / decimal_divisor;

        Ok(Self {
            id,
            decimals: faucet.decimals(),
            client,
            tx_prover,
            issuance: Arc::new(AtomicU64::new(issuance)),
            max_supply,
        })
    }

    /// Runs the faucet minting process until the request source is closed, or it encounters a fatal
    /// error.
    ///
    /// It receives new minting requests and handles them in batches. For each request, it builds a
    /// minting note and updates the issuance counter. A transaction is created and submitted with
    /// all the notes from the batch. A `MintResponse` is sent through each response sender with the
    /// new note id and transaction id.
    ///
    /// Once the available supply is exceeded, any requests that exceed the supply will return an
    /// error. The request stream is closed and the minter shuts down.
    pub async fn run(
        mut self,
        mut requests: Receiver<(MintRequest, MintResponseSender)>,
    ) -> anyhow::Result<()> {
        let mut buffer = Vec::new();
        let limit = 256; // also limited by the queue size `REQUESTS_QUEUE_SIZE`

        while requests.recv_many(&mut buffer, limit).await > 0 {
            // Check if there are enough tokens available and update the supply counter for each
            // request.
            let mut valid_requests = vec![];
            let mut response_senders = vec![];
            for (request, response_sender) in buffer.drain(..) {
                let requested_amount = request.asset_amount.inner();
                let available_amount = self.available_supply();
                if available_amount < requested_amount {
                    error!(requested_amount, available_amount, request.account_id = %request.account_id, "Requested amount exceeds available supply");
                    let _ = response_sender.send(Err(MintError::AvailableSupplyExceeded));
                    continue;
                }
                valid_requests.push(request);
                response_senders.push(response_sender);
                self.issuance.fetch_add(requested_amount, Ordering::Relaxed);
            }
            if self.available_supply() == 0 {
                error!("Faucet has run out of tokens");
            }
            if valid_requests.is_empty() {
                continue;
            }

            let mut rng = {
                let auth_seed: [u64; 4] = rng().random();
                let rng_seed = Word::from(auth_seed.map(Felt::new));
                RpoRandomCoin::new(rng_seed)
            };
            let notes = build_p2id_notes(self.id, self.decimals, &valid_requests, &mut rng)?;
            let note_ids = notes.iter().map(OutputNote::id).collect::<Vec<_>>();
            let tx_id = self.create_transaction(notes).await?;

            for (sender, note_id) in response_senders.into_iter().zip(note_ids) {
                // Ignore errors if the request was dropped.
                let _ = sender.send(Ok(MintResponse { tx_id, note_id }));
            }
            self.client.sync_state().await?;
        }

        tracing::info!("Request stream closed, shutting down minter");

        Ok(())
    }

    /// Creates a transaction with the given notes, executes it, proves it, and submits using the
    /// local miden-client. This results in submitting the transaction to the node and updating the
    /// local db to track the created notes.
    async fn create_transaction(
        &mut self,
        notes: Vec<OutputNote>,
    ) -> Result<TransactionId, ClientError> {
        // Build the transaction
        let tx = TransactionRequestBuilder::new().own_output_notes(notes).build()?;

        // Execute the transaction
        let tx_result = self.client.new_transaction(self.id.account_id, tx).await?;
        let tx_id = tx_result.executed_transaction().id();

        // Prove and submit the transaction
        let prover_failed = self
            .client
            .submit_transaction_with_prover(tx_result.clone(), self.tx_prover.clone())
            .await
            .is_err();
        if prover_failed {
            warn!("Failed to prove transaction with remote prover, falling back to local prover");
            self.client.submit_transaction(tx_result).await?;
        }

        Ok(tx_id)
    }

    /// Returns the id of the faucet account.
    pub fn faucet_id(&self) -> FaucetId {
        self.id
    }

    /// Returns the available supply of the faucet.
    pub fn available_supply(&self) -> u64 {
        self.max_supply - self.issuance.load(Ordering::Relaxed)
    }

    /// Returns the amount of tokens issued by the faucet.
    pub fn issuance(&self) -> Arc<AtomicU64> {
        self.issuance.clone()
    }
}

// HELPER FUNCTIONS
// ================================================================================================

/// Builds a collection of `P2ID` notes from a set of mint requests.
///
/// # Errors
///
/// Returns an error if creating any p2id note fails.
fn build_p2id_notes(
    source: FaucetId,
    decimals: u8,
    requests: &[MintRequest],
    rng: &mut RpoRandomCoin,
) -> Result<Vec<OutputNote>, NoteError> {
    // If building a note fails, we discard the whole batch. Should never happen, since account
    // ids are validated on the request level.
    let mut notes = Vec::new();
    for request in requests {
        let amount = request.asset_amount.inner() * 10u64.pow(decimals.into());
        // SAFETY: source is definitely a faucet account, and the amount is valid.
        let asset = FungibleAsset::new(source.account_id, amount).unwrap();
        let note = create_p2id_note(
                source.account_id,
                request.account_id,
                vec![asset.into()],
                request.note_type.into(),
                Felt::default(),
                rng,
            ).inspect_err(|err| tracing::error!(request.account_id=%request.account_id, ?err, "failed to build note"))?;
        notes.push(OutputNote::Full(note));
    }
    Ok(notes)
}
