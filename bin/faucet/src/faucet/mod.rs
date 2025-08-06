use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use miden_client::account::component::BasicFungibleFaucet;
use miden_client::account::{AccountFile, AccountId, NetworkId};
use miden_client::asset::FungibleAsset;
use miden_client::builder::ClientBuilder;
use miden_client::crypto::RpoRandomCoin;
use miden_client::keystore::FilesystemKeyStore;
use miden_client::note::{NoteError, create_p2id_note};
use miden_client::rpc::Endpoint;
use miden_client::transaction::{
    LocalTransactionProver,
    OutputNote,
    TransactionId,
    TransactionProver,
    TransactionRequestBuilder,
};
use miden_client::{Client, ClientError, Felt, RemoteTransactionProver, Word};
use rand::rngs::StdRng;
use rand::{Rng, rng};
use serde::Serialize;
use tokio::sync::mpsc::Receiver;
use tracing::{info, instrument, warn};
use url::Url;

use crate::network::ExplorerUrl;
use crate::server::{MintRequest, MintResponse, MintResponseSender};

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
}

impl Faucet {
    /// Loads the faucet state from the node and the account file.
    #[instrument(name = "faucet.load", fields(id), skip_all)]
    pub async fn load(
        store_path: PathBuf,
        network_id: NetworkId,
        account_file: AccountFile,
        node_url: &Url,
        timeout: Duration,
        remote_tx_prover_url: Option<Url>,
    ) -> anyhow::Result<Self> {
        let account = account_file.account;
        tracing::Span::current().record("id", account.id().to_string());

        let keystore = FilesystemKeyStore::<StdRng>::new(PathBuf::from("keystore"))
            .context("failed to create keystore")?;
        for key in account_file.auth_secret_keys {
            keystore.add_key(&key)?;
        }
        let endpoint = Endpoint::try_from(node_url.as_str())
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("failed to parse node url: {node_url}"))?;

        let mut client = ClientBuilder::new()
            .tonic_rpc_client(&endpoint, Some(timeout.as_millis() as u64))
            .authenticator(Arc::new(keystore))
            .sqlite_store(store_path.to_str().context("invalid store path")?)
            .build()
            .await?;

        info!("Fetching faucet state from node");

        match client.import_account_by_id(account.id()).await {
            Ok(()) => {
                // SAFETY: if import was successful, the account is tracked by the client
                let record = client.get_account(account.id()).await?.unwrap();
                info!(
                    commitment = %record.account().commitment(),
                    nonce = %record.account().nonce(),
                    "Received faucet account state from the node",
                );
            },
            Err(_) => match client.add_account(&account, account_file.account_seed, false).await {
                Ok(()) => {
                    info!(
                        commitment = %account.commitment(),
                        nonce = %account.nonce(),
                        "Loaded state from account file"
                    );
                },
                Err(ClientError::AccountAlreadyTracked(_)) => {
                    // SAFETY: account is tracked, so its present in the db
                    let record = client.get_account(account.id()).await?.unwrap();
                    info!(
                        commitment = %record.account().commitment(),
                        nonce = %record.account().nonce(),
                        "Loaded state from existing local client db"
                    );
                },
                Err(err) => anyhow::bail!("failed to add account from file: {err}"),
            },
        }

        client.ensure_genesis_in_place().await?;

        let faucet = BasicFungibleFaucet::try_from(&account)?;
        let tx_prover: Arc<dyn TransactionProver> = match remote_tx_prover_url {
            Some(url) => Arc::new(RemoteTransactionProver::new(url)),
            None => Arc::new(LocalTransactionProver::default()),
        };
        let id = FaucetId::new(account.id(), network_id);

        Ok(Self {
            id,
            decimals: faucet.decimals(),
            client,
            tx_prover,
        })
    }

    /// Runs the faucet minting process until the request source is closed, or it encounters a fatal
    /// error.
    ///
    /// It receives new minting requests and handles them in batches. For each request, it builds a
    /// minting note that is included in a new transaction. For each request, sends the
    /// resulting `MintResponse` through the response sender.
    pub async fn run(
        mut self,
        mut requests: Receiver<(MintRequest, MintResponseSender)>,
    ) -> anyhow::Result<()> {
        let mut buffer = Vec::new();
        let limit = 256; // also limited by the queue size `REQUESTS_QUEUE_SIZE`
        let explorer_url = ExplorerUrl::from_network_id(self.id.network_id);

        while requests.recv_many(&mut buffer, limit).await > 0 {
            let (requests, response_senders): (Vec<MintRequest>, Vec<MintResponseSender>) =
                buffer.drain(..).unzip();

            let mut rng = {
                let auth_seed: [u64; 4] = rng().random();
                let rng_seed = Word::from(auth_seed.map(Felt::new));
                RpoRandomCoin::new(rng_seed)
            };
            let notes = build_p2id_notes(self.id, self.decimals, &requests, &mut rng)?;
            let note_ids = notes.iter().map(OutputNote::id).collect::<Vec<_>>();
            let tx_id = self.create_transaction(notes).await?;

            for (sender, note_id) in response_senders.into_iter().zip(note_ids) {
                // Ignore errors if the request was dropped.
                let _ = sender.send(MintResponse {
                    tx_id,
                    note_id,
                    explorer_url: explorer_url.clone(),
                });
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
