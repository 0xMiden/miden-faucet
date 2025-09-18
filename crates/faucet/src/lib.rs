use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use miden_client::account::component::{BasicFungibleFaucet, FungibleFaucetExt};
use miden_client::account::{
    Account,
    AccountId,
    AccountIdAddress,
    Address,
    AddressInterface,
    NetworkId,
};
use miden_client::asset::FungibleAsset;
use miden_client::auth::AuthSecretKey;
use miden_client::builder::ClientBuilder;
use miden_client::crypto::{Rpo256, RpoRandomCoin};
use miden_client::keystore::FilesystemKeyStore;
use miden_client::note::{Note, NoteError, create_p2id_note};
use miden_client::rpc::Endpoint;
use miden_client::transaction::{
    LocalTransactionProver,
    TransactionId,
    TransactionProver,
    TransactionRequest,
    TransactionRequestBuilder,
    TransactionRequestError,
    TransactionScript,
};
use miden_client::utils::{Deserializable, RwLock};
use miden_client::{Client, ClientError, Felt, RemoteTransactionProver, Word};
use rand::rngs::StdRng;
use rand::{Rng, rng};
use tokio::sync::mpsc::Receiver;
use tracing::{error, instrument, warn};
use url::Url;

pub mod requests;
pub mod types;

use crate::requests::{MintError, MintRequest, MintResponse, MintResponseSender};
use crate::types::AssetAmount;

const KEYSTORE_PATH: &str = "keystore";
const BATCH_SIZE: usize = 64;
const TX_SCRIPT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/assets/tx_scripts/mint.txs"));

// FAUCET CLIENT
// ================================================================================================

/// The faucet's account ID and network ID.
///
/// Used as a type safety mechanism to avoid confusion with user account IDs, and allows us to
/// implement traits.
#[derive(Clone)]
pub struct FaucetId {
    pub account_id: AccountId,
    pub network_id: NetworkId,
}

impl FaucetId {
    pub fn new(account_id: AccountId, network_id: NetworkId) -> Self {
        Self { account_id, network_id }
    }

    pub fn to_bech32(&self) -> String {
        let address = AccountIdAddress::new(self.account_id, AddressInterface::Unspecified);
        Address::from(address).to_bech32(self.network_id.clone())
    }
}

/// Stores the current faucet state and handles minting requests.
pub struct Faucet {
    id: FaucetId,
    client: Client<FilesystemKeyStore<StdRng>>,
    tx_prover: Arc<dyn TransactionProver>,
    issuance: Arc<RwLock<AssetAmount>>,
    max_supply: AssetAmount,
    script: TransactionScript,
}

/// Configuration for initializing and loading a faucet.
pub struct FaucetConfig {
    /// The path to the client store file.
    pub store_path: String,
    /// The endpoint of the node to connect to.
    pub node_endpoint: Endpoint,
    /// The network ID of the node to connect to.
    pub network_id: NetworkId,
    /// The timeout for the node connection.
    pub timeout: Duration,
    /// The remote prover url to use for proving transactions. If set to none, a local transaction
    /// prover is used.
    pub remote_tx_prover_url: Option<Url>,
}

impl Faucet {
    /// Initializes a new faucet client, creating the keystore and the database with the given
    /// account. If set to deploy, an empty transaction is created and submitted to the node.
    #[instrument(name = "faucet.init", skip_all)]
    pub async fn init(
        config: &FaucetConfig,
        account: Account,
        account_seed: Word,
        secret_key: &AuthSecretKey,
        deploy: bool,
    ) -> anyhow::Result<()> {
        let keystore = FilesystemKeyStore::<StdRng>::new(KEYSTORE_PATH.into())
            .context("failed to create keystore")?;
        keystore.add_key(secret_key)?;

        let mut client = ClientBuilder::<FilesystemKeyStore<StdRng>>::new()
            .tonic_rpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .authenticator(Arc::new(keystore))
            .sqlite_store(&config.store_path)
            .build()
            .await?;
        client.add_account(&account, Some(account_seed), false).await?;
        client.set_default_account_id(account.id()).await?;

        if deploy {
            let mut faucet = Self::load(config).await?;
            let empty_tx_request = TransactionRequestBuilder::new().build()?;
            faucet.submit_transaction(empty_tx_request).await?;
        }

        Ok(())
    }

    /// Loads the faucet.
    ///
    /// The account is loaded from the local store. If it is not tracked, it is fetched from the
    /// node and added to the local store.
    #[instrument(name = "faucet.load", skip_all)]
    pub async fn load(config: &FaucetConfig) -> anyhow::Result<Self> {
        let mut client = ClientBuilder::new()
            .tonic_rpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .filesystem_keystore(KEYSTORE_PATH)
            .sqlite_store(&config.store_path)
            .build()
            .await
            .context("failed to build client")?;

        let account_id =
            client.get_default_account_id().await?.context("no default account id found")?;
        let record = client.get_account(account_id).await?.context("no account found")?;
        let account = record.account();

        client.ensure_genesis_in_place().await?;

        let faucet = BasicFungibleFaucet::try_from(account)?;
        let tx_prover: Arc<dyn TransactionProver> = match config.remote_tx_prover_url.clone() {
            Some(url) => Arc::new(RemoteTransactionProver::new(url)),
            None => Arc::new(LocalTransactionProver::default()),
        };
        let id = FaucetId::new(account.id(), config.network_id.clone());
        let max_supply = AssetAmount::new(faucet.max_supply().as_int())?;
        let issuance =
            Arc::new(RwLock::new(AssetAmount::new(account.get_token_issuance()?.as_int())?));

        let script = TransactionScript::read_from_bytes(TX_SCRIPT)?;

        Ok(Self {
            id,
            client,
            tx_prover,
            issuance,
            max_supply,
            script,
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
    #[instrument(name = "faucet.run", skip_all, err)]
    pub async fn run(
        mut self,
        mut requests: Receiver<(MintRequest, MintResponseSender)>,
    ) -> anyhow::Result<()> {
        let mut buffer = Vec::new();

        while requests.recv_many(&mut buffer, BATCH_SIZE).await > 0 {
            // Check if there are enough tokens available and update the supply counter for each
            // request.
            let mut valid_requests = vec![];
            let mut response_senders = vec![];
            for (request, response_sender) in buffer.drain(..) {
                let available_amount = self.available_supply().unwrap_or_default();
                let requested_amount = request.asset_amount;
                if available_amount < requested_amount {
                    error!(
                        requested_amount = requested_amount.base_units(),
                        available_amount = available_amount.base_units(),
                        account_id = %request.account_id,
                        "Requested amount exceeds available supply",
                    );
                    let _ = response_sender.send(Err(MintError::AvailableSupplyExceeded));
                    continue;
                }
                valid_requests.push(request);
                response_senders.push(response_sender);
                let mut issuance = self.issuance.write();
                *issuance = issuance
                    .checked_add(requested_amount)
                    .expect("issuance should never be an invalid amount");
            }
            if self.available_supply().is_none() {
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
            let notes = build_p2id_notes(&self.id, &valid_requests, &mut rng)?;
            let note_ids = notes.iter().map(Note::id).collect::<Vec<_>>();
            let tx_request =
                self.create_transaction(notes).context("faucet failed to create transaction")?;
            let tx_id = self
                .submit_transaction(tx_request)
                .await
                .context("faucet failed to submit transaction")?;

            for (sender, note_id) in response_senders.into_iter().zip(note_ids) {
                // Ignore errors if the request was dropped.
                let _ = sender.send(Ok(MintResponse { tx_id, note_id }));
            }
            self.client.sync_state().await.context("faucet failed to sync state")?;
        }

        tracing::info!("Request stream closed, shutting down minter");

        Ok(())
    }

    /// Creates a transaction that generates the given p2id notes.
    fn create_transaction(
        &mut self,
        notes: Vec<Note>,
    ) -> Result<TransactionRequest, TransactionRequestError> {
        let expected_output_recipients = notes.iter().map(Note::recipient).cloned().collect();
        let n = notes.len() as u64;
        let mut note_data = vec![Felt::new(n)];
        for note in notes {
            // SAFETY: these are p2id notes with only one fungible asset
            let amount = note.assets().iter().next().unwrap().unwrap_fungible().amount();

            note_data.extend(note.recipient().digest().iter());
            note_data.push(Felt::from(note.metadata().note_type()));
            note_data.push(Felt::from(note.metadata().tag()));
            note_data.push(Felt::new(amount));
        }
        let note_data_commitment = Rpo256::hash_elements(&note_data);
        let advice_map = [(note_data_commitment, note_data)];

        TransactionRequestBuilder::new()
            .custom_script(self.script.clone())
            .extend_advice_map(advice_map)
            .expected_output_recipients(expected_output_recipients)
            .script_arg(note_data_commitment)
            .build()
    }

    /// Executes, proves, and then submits a transaction using the local miden-client.
    /// This results in submitting the transaction to the node and updating the local db to track
    /// the created notes.
    async fn submit_transaction(
        &mut self,
        tx_request: TransactionRequest,
    ) -> Result<TransactionId, ClientError> {
        // Execute the transaction
        let tx_result =
            Box::pin(self.client.new_transaction(self.id.account_id, tx_request)).await?;
        let tx_id = tx_result.executed_transaction().id();

        // Prove and submit the transaction
        let prover_failed = Box::pin(
            self.client
                .submit_transaction_with_prover(tx_result.clone(), self.tx_prover.clone()),
        )
        .await
        .is_err();
        if prover_failed {
            warn!("Failed to prove transaction with remote prover, falling back to local prover");
            Box::pin(self.client.submit_transaction(tx_result)).await?;
        }

        Ok(tx_id)
    }

    /// Returns the faucet account.
    pub async fn faucet_account(&self) -> Result<Account, ClientError> {
        Ok(self
            .client
            .get_account(self.id.account_id)
            .await?
            .ok_or(ClientError::AccountDataNotFound(self.id.account_id))?
            .account()
            .clone())
    }

    /// Returns the id of the faucet account.
    pub fn faucet_id(&self) -> FaucetId {
        self.id.clone()
    }

    /// Returns the available supply of the faucet.
    pub fn available_supply(&self) -> Option<AssetAmount> {
        self.max_supply.checked_sub(*self.issuance.read())
    }

    /// Returns the amount of tokens issued by the faucet.
    pub fn issuance(&self) -> Arc<RwLock<AssetAmount>> {
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
    source: &FaucetId,
    requests: &[MintRequest],
    rng: &mut RpoRandomCoin,
) -> Result<Vec<Note>, NoteError> {
    // If building a note fails, we discard the whole batch. Should never happen, since account
    // ids are validated on the request level.
    let mut notes = Vec::new();
    for request in requests {
        // SAFETY: source is definitely a faucet account, and the amount is valid.
        let asset =
            FungibleAsset::new(source.account_id, request.asset_amount.base_units()).unwrap();
        let note = create_p2id_note(
                source.account_id,
                request.account_id,
                vec![asset.into()],
                request.note_type.into(),
                Felt::default(),
                rng,
            ).inspect_err(|err| tracing::error!(request.account_id=%request.account_id, ?err, "failed to build note"))?;
        notes.push(note);
    }
    Ok(notes)
}
