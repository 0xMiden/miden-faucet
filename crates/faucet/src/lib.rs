use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use miden_client::account::component::{BasicFungibleFaucet, FungibleFaucetExt};
use miden_client::account::{Account, AccountId, Address, NetworkId};
use miden_client::asset::FungibleAsset;
use miden_client::auth::AuthSecretKey;
use miden_client::builder::ClientBuilder;
use miden_client::crypto::{Rpo256, RpoRandomCoin};
use miden_client::keystore::FilesystemKeyStore;
use miden_client::note::{Note, NoteError, NoteId, create_p2id_note};
use miden_client::rpc::{Endpoint, GrpcClient, RpcError};
use miden_client::store::{NoteFilter, TransactionFilter};
use miden_client::sync::{StateSync, SyncSummary};
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
use miden_client_sqlite_store::SqliteStore;
use rand::rngs::StdRng;
use rand::{Rng, rng};
use tokio::sync::mpsc::Receiver;
use tracing::{Instrument, error, info, info_span, instrument, warn};
use url::Url;

mod note_screener;
pub mod requests;
pub mod types;

use crate::note_screener::NoteScreener;
use crate::requests::{MintError, MintRequest, MintResponse, MintResponseSender};
use crate::types::{AssetAmount, NoteType};

const COMPONENT: &str = "miden-faucet-client";

const TX_SCRIPT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/assets/tx_scripts/mint.txs"));
const KEYSTORE_PATH: &str = "keystore";
const DEFAULT_ACCOUNT_ID_SETTING: &str = "faucet_default_account_id";

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
        Address::new(self.account_id).encode(self.network_id.clone())
    }
}

/// Stores the current faucet state and handles minting requests.
pub struct Faucet {
    id: FaucetId,
    client: Client<FilesystemKeyStore<StdRng>>,
    state_sync_component: StateSync,
    tx_prover: Arc<dyn TransactionProver>,
    issuance: Arc<RwLock<AssetAmount>>,
    max_supply: AssetAmount,
    script: TransactionScript,
}

/// Configuration for initializing and loading a faucet.
pub struct FaucetConfig {
    /// The path to the client store file.
    pub store_path: PathBuf,
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
    pub async fn init(
        config: &FaucetConfig,
        account: Account,
        secret_key: &AuthSecretKey,
        deploy: bool,
    ) -> anyhow::Result<()> {
        let keystore = FilesystemKeyStore::<StdRng>::new(KEYSTORE_PATH.into())
            .context("failed to create keystore")?;
        keystore.add_key(secret_key)?;

        let sqlite_store = Arc::new(SqliteStore::new(config.store_path.clone()).await?);

        let mut client = ClientBuilder::new()
            .grpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .authenticator(Arc::new(keystore))
            .store(sqlite_store.clone())
            .build()
            .await?;

        client.ensure_genesis_in_place().await?;

        // We sync to the chain tip before importing the account to avoid matching too many notes
        // tags from the genesis block (in case this is a fresh store).
        let note_screener = NoteScreener::new(sqlite_store.clone());
        let grpc_client =
            Arc::new(GrpcClient::new(&config.node_endpoint, config.timeout.as_millis() as u64));
        let state_sync_component =
            StateSync::new(grpc_client.clone(), Arc::new(note_screener), None);
        Self::sync_state(account.id(), &mut client, &state_sync_component).await?;

        let add_result = client.add_account(&account, false).await;
        match add_result {
            Ok(()) => (),
            Err(ClientError::AccountAlreadyTracked(_)) => {
                warn!("Account already tracked, skipping import");
            },
            Err(error) => anyhow::bail!("failed to add account: {error}"),
        }
        client.set_setting(DEFAULT_ACCOUNT_ID_SETTING.to_owned(), account.id()).await?;

        if deploy {
            let mut faucet = Self::load(config).await?;

            // TODO: This is a workaround to deploy the account on the chain. Ideally this would be
            // done with an empty transaction, but that currently fails due to: https://github.com/0xMiden/miden-base/issues/2072
            // Once that change is included in the next release, we can revert this workaround.
            let mut rng = {
                let auth_seed: [u64; 4] = rng().random();
                let rng_seed = Word::from(auth_seed.map(Felt::new));
                RpoRandomCoin::new(rng_seed)
            };
            let notes = build_p2id_notes(
                &faucet.faucet_id(),
                &[MintRequest {
                    account_id: faucet.id.account_id,
                    note_type: NoteType::Private,
                    asset_amount: AssetAmount::new(1).unwrap(),
                }],
                &mut rng,
            )?;

            // Build and submit transaction
            let tx_request = faucet
                .create_transaction(notes)
                .context("faucet failed to create transaction")?;

            faucet.submit_new_transaction(tx_request).await?;
        }

        Ok(())
    }

    /// Loads the faucet with the given config.
    ///
    /// The account used is the default account set in the store, that is set on `Faucet::init`.
    #[instrument(target = COMPONENT, name = "faucet.load", fields(account_id), skip_all, err)]
    pub async fn load(config: &FaucetConfig) -> anyhow::Result<Self> {
        let span = tracing::Span::current();
        let mut client = ClientBuilder::new()
            .grpc_client(&config.node_endpoint, Some(config.timeout.as_millis() as u64))
            .filesystem_keystore(KEYSTORE_PATH)
            .store(Arc::new(SqliteStore::new(config.store_path.clone()).await?))
            .build()
            .await
            .context("failed to build client")?;

        let account_id: AccountId = client
            .get_setting(DEFAULT_ACCOUNT_ID_SETTING.to_owned())
            .await?
            .context("no default account id found")?;
        span.record("account_id", account_id.to_hex());

        // Try to update the account state with the node.
        let _ = client.import_account_by_id(account_id).await.inspect(|_| {
            info!("Received faucet account state from the node");
        });

        let record = client.get_account(account_id).await?.context("no account found")?;
        let account = record.account();

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

        let note_screener =
            NoteScreener::new(Arc::new(SqliteStore::new(config.store_path.clone()).await?));
        let grpc_client =
            Arc::new(GrpcClient::new(&config.node_endpoint, config.timeout.as_millis() as u64));
        let state_sync_component = StateSync::new(grpc_client, Arc::new(note_screener), None);

        Ok(Self {
            id,
            client,
            state_sync_component,
            tx_prover,
            issuance,
            max_supply,
            script,
        })
    }

    /// Syncs the state of the client.
    #[instrument(target = COMPONENT, name = "faucet.sync_state", skip_all, err)]
    async fn sync_state(
        account_id: AccountId,
        client: &mut Client<FilesystemKeyStore<StdRng>>,
        state_sync: &StateSync,
    ) -> anyhow::Result<SyncSummary> {
        // Get current state of the client
        let accounts = client
            .get_account_header_by_id(account_id)
            .await?
            .map(|(header, _)| vec![header])
            .unwrap_or_default();
        let note_tags = BTreeSet::new();
        let input_notes = vec![];
        let expected_output_notes = client.get_output_notes(NoteFilter::Expected).await?;
        let uncommitted_transactions =
            client.get_transactions(TransactionFilter::Uncommitted).await?;

        // Build current partial MMR
        let current_partial_mmr = client.get_current_partial_mmr().await?;

        // Get the sync update from the network
        let state_sync_update = state_sync
            .sync_state(
                current_partial_mmr,
                accounts,
                note_tags,
                input_notes,
                expected_output_notes,
                uncommitted_transactions,
            )
            .await
            .context("failed to sync state")?;
        let sync_summary: SyncSummary = (&state_sync_update).into();

        // Apply received and computed updates to the store
        client
            .apply_state_sync(state_sync_update)
            .await
            .context("failed to apply state sync")?;

        Ok(sync_summary)
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
        batch_size: usize,
    ) -> anyhow::Result<()> {
        let mut buffer = Vec::new();

        while requests.recv_many(&mut buffer, batch_size).await > 0 {
            match self.mint(buffer.drain(..)).await {
                Ok(()) => (),
                Err(error) => {
                    if let Some(ClientError::RpcError(RpcError::ConnectionError(_))) =
                        error.downcast_ref::<ClientError>()
                    {
                        error!(?error, "connection error, discarding batch");
                    } else {
                        anyhow::bail!("failed to mint batch: {error}");
                    }
                },
            }
        }
        info!(target = COMPONENT, "Request stream closed, shutting down minter");

        Ok(())
    }

    /// Mints a batch of requests.
    ///
    /// The requests size is guaranteed to be smaller or equal to the batch size set in
    /// `Faucet::run`.
    #[instrument(parent = None, target = COMPONENT, name = "faucet.mint", skip_all, fields(num_requests, tx_id), err)]
    async fn mint(
        &mut self,
        requests: impl IntoIterator<Item = (MintRequest, MintResponseSender)>,
    ) -> anyhow::Result<()> {
        // We sync before creating the transaction to ensure the state is up to date. If the
        // previous transaction somehow failed to be included in the block, our state would
        // be out of sync.
        Self::sync_state(self.id.account_id, &mut self.client, &self.state_sync_component).await?;

        let span = tracing::Span::current();

        let (valid_requests, response_senders) = self.filter_requests_by_supply(requests);
        span.record("num_requests", valid_requests.len());

        if valid_requests.is_empty() {
            return Ok(());
        }

        // Build notes
        let mut rng = {
            let auth_seed: [u64; 4] = rng().random();
            let rng_seed = Word::from(auth_seed.map(Felt::new));
            RpoRandomCoin::new(rng_seed)
        };
        let notes = build_p2id_notes(&self.faucet_id(), &valid_requests, &mut rng)?;
        let note_ids = notes.iter().map(Note::id).collect::<Vec<_>>();

        // Build and submit transaction
        let tx_request =
            self.create_transaction(notes).context("faucet failed to create transaction")?;
        let tx_id = self
            .submit_new_transaction(tx_request)
            .await
            .context("faucet failed to submit transaction")?;
        span.record("tx_id", tx_id.to_string());

        Self::send_responses(response_senders, note_ids, tx_id);
        Ok(())
    }

    /// Sends a `MintResponse` with the transaction id and note id through each of the response
    /// senders. Any errors while sending the response are ignored.
    #[instrument(target = COMPONENT, name = "faucet.mint.send_responses", skip_all)]
    fn send_responses(
        response_senders: Vec<MintResponseSender>,
        note_ids: Vec<NoteId>,
        tx_id: TransactionId,
    ) {
        for (sender, note_id) in response_senders.into_iter().zip(note_ids) {
            // Ignore errors if the request was dropped.
            let _ = sender.send(Ok(MintResponse { tx_id, note_id }));
        }
    }

    /// Updates the issuance counter for the requested amounts and filters the requests that exceed
    /// the available supply. For the filtered requests, the response sender is notified with an
    /// error.
    ///
    /// Returns a tuple of valid requests and response senders.
    #[instrument(target = COMPONENT, name = "faucet.mint.filter_requests_by_supply", skip_all)]
    fn filter_requests_by_supply(
        &self,
        requests: impl IntoIterator<Item = (MintRequest, MintResponseSender)>,
    ) -> (Vec<MintRequest>, Vec<MintResponseSender>) {
        let mut valid_requests = vec![];
        let mut response_senders = vec![];
        for (request, response_sender) in requests {
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
        (valid_requests, response_senders)
    }

    /// Creates a transaction that generates the given p2id notes.
    #[instrument(target = COMPONENT, name = "faucet.mint.create_tx", skip_all, err)]
    fn create_transaction(
        &mut self,
        notes: Vec<Note>,
    ) -> Result<TransactionRequest, TransactionRequestError> {
        // Build the transaction
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
    #[instrument(target = COMPONENT, name = "faucet.mint.submit_new_transaction", skip_all, err)]
    async fn submit_new_transaction(
        &mut self,
        tx_request: TransactionRequest,
    ) -> Result<TransactionId, ClientError> {
        // Execute the transaction
        let tx_result = self
            .client
            .execute_transaction(self.id.account_id, tx_request)
            .instrument(info_span!(target: COMPONENT, "faucet.mint.execute"))
            .await?;
        let tx_id = tx_result.executed_transaction().id();

        let proven_transaction = {
            let remote_proven_transaction = self
                .client
                .prove_transaction_with(&tx_result, self.tx_prover.clone())
                .instrument(info_span!(target: COMPONENT, "faucet.mint.prove_remote"))
                .await;
            match remote_proven_transaction {
                Ok(proven_transaction) => proven_transaction,
                Err(error) => {
                    error!(?error, "Failed to prove transaction with remote prover");
                    self.client
                        .prove_transaction(&tx_result)
                        .instrument(info_span!(target: COMPONENT, "faucet.mint.prove_local"))
                        .await?
                },
            }
        };

        let submission_height = self
            .client
            .submit_proven_transaction(proven_transaction, &tx_result)
            .instrument(info_span!(target: COMPONENT, "faucet.mint.submit_transaction"))
            .await?;

        self.client.apply_transaction(&tx_result, submission_height).await?;

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
#[instrument(target = COMPONENT, name = "faucet.mint.build_notes", skip_all)]
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
        )
        .inspect_err(
            |err| error!(request.account_id=%request.account_id, ?err, "failed to build note"),
        )?;
        notes.push(note);
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use miden_client::ExecutionOptions;
    use miden_client::account::component::AuthRpoFalcon512;
    use miden_client::account::{AccountBuilder, AccountStorageMode, AccountType};
    use miden_client::asset::TokenSymbol;
    use miden_client::auth::AuthSecretKey;
    use miden_client::crypto::rpo_falcon512::SecretKey;
    use miden_client::store::{NoteFilter, Store};
    use miden_client::testing::MockChain;
    use miden_client::testing::mock::{MockClient, MockRpcApi};
    use miden_client_sqlite_store::SqliteStore;
    use tokio::sync::{mpsc, oneshot};

    use super::*;
    use crate::types::NoteType;

    #[tokio::test]
    async fn batch_requests() {
        let batch_size = 32;

        let (tx_mint_requests, rx_mint_requests) = mpsc::channel(1000);
        let mut receivers = vec![];
        for i in 0..batch_size {
            let (sender, receiver) = oneshot::channel();
            let mint_request = MintRequest {
                account_id: AccountId::try_from(1).unwrap(),
                note_type: if i % 2 == 0 {
                    NoteType::Public
                } else {
                    NoteType::Private
                },
                asset_amount: AssetAmount::new(100_000_000).unwrap(),
            };
            tx_mint_requests.send((mint_request, sender)).await.unwrap();
            receivers.push(receiver);
        }

        let store =
            Arc::new(SqliteStore::new(temp_dir().join("batch_requests.sqlite3")).await.unwrap());
        run_faucet(rx_mint_requests, batch_size, store.clone());

        for receiver in receivers {
            let response = receiver.await.unwrap().unwrap();
            let notes = store.get_output_notes(NoteFilter::Unique(response.note_id)).await.unwrap();
            assert_eq!(notes.len(), 1);
        }
    }

    // TESTING HELPERS
    // ---------------------------------------------------------------------------------------------

    /// Runs a faucet on a separate thread using a mock client.
    fn run_faucet(
        rx_mint_requests: mpsc::Receiver<(MintRequest, MintResponseSender)>,
        batch_size: usize,
        store: Arc<dyn Store>,
    ) {
        let secret = SecretKey::new();
        let symbol = TokenSymbol::new("TEST").unwrap();
        let max_supply = Felt::try_from(1_000_000_000_000_u64).unwrap();
        let account = AccountBuilder::new(rand::random())
            .account_type(AccountType::FungibleFaucet)
            .storage_mode(AccountStorageMode::Public)
            .with_component(BasicFungibleFaucet::new(symbol, 6, max_supply).unwrap())
            .with_auth_component(AuthRpoFalcon512::new(secret.public_key().to_commitment().into()))
            .build()
            .unwrap();
        let key = AuthSecretKey::RpoFalcon512(secret);

        std::thread::spawn(move || {
            // Create a new runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .enable_io()
                .build()
                .expect("Failed to build runtime");

            // Run the faucet on this thread's runtime
            rt.block_on(async {
                let keystore =
                    FilesystemKeyStore::<StdRng>::new(PathBuf::from("keystore")).unwrap();
                keystore.add_key(&key).unwrap();

                let mut client = MockClient::new(
                    Arc::new(MockRpcApi::new(MockChain::new())),
                    Box::new(RpoRandomCoin::new(Word::empty())),
                    store.clone(),
                    Some(Arc::new(keystore)),
                    ExecutionOptions::new(None, 4096, false, false).unwrap(),
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .unwrap();
                client.ensure_genesis_in_place().await.unwrap();
                client.add_account(&account, false).await.unwrap();
                let faucet = Faucet {
                    id: FaucetId::new(account.id(), NetworkId::Testnet),
                    client,
                    state_sync_component: StateSync::new(
                        Arc::new(MockRpcApi::new(MockChain::new())),
                        Arc::new(NoteScreener::new(store.clone())),
                        None,
                    ),
                    tx_prover: Arc::new(LocalTransactionProver::default()),
                    issuance: Arc::new(RwLock::new(AssetAmount::new(0).unwrap())),
                    max_supply: AssetAmount::new(1_000_000_000_000).unwrap(),
                    script: TransactionScript::read_from_bytes(TX_SCRIPT).unwrap(),
                };
                faucet.run(rx_mint_requests, batch_size).await.unwrap();
            });
        });
    }
}
