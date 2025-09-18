use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use miden_client::account::component::{BasicFungibleFaucet, FungibleFaucetExt};
use miden_client::account::{
    AccountFile,
    AccountId,
    AccountIdAddress,
    Address,
    AddressInterface,
    NetworkId,
};
use miden_client::asset::FungibleAsset;
use miden_client::builder::ClientBuilder;
use miden_client::crypto::{Rpo256, RpoRandomCoin};
use miden_client::keystore::FilesystemKeyStore;
use miden_client::note::{Note, NoteError, create_p2id_note};
use miden_client::rpc::Endpoint;
use miden_client::transaction::{
    LocalTransactionProver,
    TransactionId,
    TransactionProver,
    TransactionRequestBuilder,
    TransactionScript,
};
use miden_client::utils::{Deserializable, RwLock};
use miden_client::{Client, ClientError, Felt, RemoteTransactionProver, Word};
use miden_client_sqlite_store::SqliteStore;
use rand::rngs::StdRng;
use rand::{Rng, rng};
use tokio::sync::mpsc::Receiver;
use tracing::{error, info, instrument, warn};
use url::Url;

pub mod requests;
pub mod types;

use crate::requests::{MintError, MintRequest, MintResponse, MintResponseSender};
use crate::types::AssetAmount;

const COMPONENT: &str = "miden-faucet-client";
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

impl Faucet {
    /// Loads the faucet.
    ///
    /// A client is instantiated with the provided store path, node url and timeout. The account is
    /// loaded from the provided account file. If the account is already tracked by the current
    /// store, it is loaded. Otherwise, the account is added from the file state.
    ///
    /// If a remote transaction prover url is provided, it is used to prove transactions. Otherwise,
    /// a local transaction prover is used.
    #[instrument(target = COMPONENT, name = "faucet.load", fields(id), skip_all)]
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
        let url: &str = node_url.as_str().trim_end_matches('/');
        let endpoint = Endpoint::try_from(url)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("failed to parse node url: {node_url}"))?;

        let sqlite_store = SqliteStore::new(store_path).await?;

        let mut client = ClientBuilder::new()
            .tonic_rpc_client(&endpoint, Some(timeout.as_millis() as u64))
            .authenticator(Arc::new(keystore))
            .store(Arc::new(sqlite_store))
            .build()
            .await?;

        info!("Fetching faucet state from node");

        let issuance = match client.import_account_by_id(account.id()).await {
            Ok(()) => {
                // SAFETY: if import was successful, the account is tracked by the client
                let record = client.get_account(account.id()).await?.unwrap();
                info!(
                    commitment = %record.account().commitment(),
                    nonce = %record.account().nonce(),
                    "Received faucet account state from the node",
                );
                record.account().get_token_issuance()?
            },
            Err(_) => match client.add_account(&account, false).await {
                Ok(()) => {
                    info!(
                        commitment = %account.commitment(),
                        nonce = %account.nonce(),
                        "Loaded state from account file"
                    );
                    account.get_token_issuance()?
                },
                Err(ClientError::AccountAlreadyTracked(_)) => {
                    // SAFETY: account is tracked, so its present in the db
                    let record = client.get_account(account.id()).await?.unwrap();
                    info!(
                        commitment = %record.account().commitment(),
                        nonce = %record.account().nonce(),
                        "Loaded state from existing local client db"
                    );
                    record.account().get_token_issuance()?
                },
                Err(err) => anyhow::bail!("failed to add account from file: {err}"),
            },
        };

        client.ensure_genesis_in_place().await?;

        let faucet = BasicFungibleFaucet::try_from(&account)?;
        let tx_prover: Arc<dyn TransactionProver> = match remote_tx_prover_url {
            Some(url) => Arc::new(RemoteTransactionProver::new(url)),
            None => Arc::new(LocalTransactionProver::default()),
        };
        let id = FaucetId::new(account.id(), network_id);
        let max_supply = AssetAmount::new(faucet.max_supply().as_int())?;
        let issuance = Arc::new(RwLock::new(AssetAmount::new(issuance.as_int())?));

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
    #[instrument(target = COMPONENT, name = "faucet.run", skip_all, err)]
    pub async fn run(
        mut self,
        mut requests: Receiver<(MintRequest, MintResponseSender)>,
        batch_size: usize,
    ) -> anyhow::Result<()> {
        let mut buffer = Vec::new();

        while requests.recv_many(&mut buffer, batch_size).await > 0 {
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

            // Root span for this mint attempt (batch)
            let total_requested: u64 =
                valid_requests.iter().map(|r| r.asset_amount.base_units()).sum();
            let num_private = valid_requests
                .iter()
                .filter(|r| matches!(r.note_type, crate::types::NoteType::Private))
                .count() as u64;
            let num_public = valid_requests.len() as u64 - num_private;
            let mint_span = tracing::info_span!(
                target: COMPONENT,
                "faucet.mint",
                faucet_id = %self.id.account_id,
                num_requests = valid_requests.len() as u64,
                total_requested = total_requested,
                num_private = num_private,
                num_public = num_public,
                tx_id = tracing::field::Empty,
            );
            let _mint_enter = mint_span.enter();

            // Build notes
            let build_span = tracing::info_span!(
                target: COMPONENT,
                "faucet.mint.build_notes",
                num_requests = valid_requests.len() as u64
            );
            let notes = {
                let _enter = build_span.enter();
                let mut rng = {
                    let auth_seed: [u64; 4] = rng().random();
                    let rng_seed = Word::from(auth_seed.map(Felt::new));
                    RpoRandomCoin::new(rng_seed)
                };
                build_p2id_notes(&self.faucet_id(), &valid_requests, &mut rng)?
            };
            let note_ids = notes.iter().map(Note::id).collect::<Vec<_>>();
            let tx_id = Box::pin(self.create_transaction(notes))
                .await
                .context("faucet failed to create transaction")?;
            tracing::Span::current().record("tx_id", tx_id.to_string());

            let send_resp_span =
                tracing::info_span!(target: COMPONENT, "faucet.mint.send_responses");
            let _enter = send_resp_span.enter();
            for (sender, note_id) in response_senders.into_iter().zip(note_ids) {
                // Ignore errors if the request was dropped.
                let _ = sender.send(Ok(MintResponse { tx_id, note_id }));
            }
            let sync_span = tracing::info_span!(target: COMPONENT, "faucet.mint.state_sync");
            {
                let _enter = sync_span.enter();
                self.client.sync_state().await.context("faucet failed to sync state")?;
            }
        }

        tracing::info!("Request stream closed, shutting down minter");

        Ok(())
    }

    /// Creates a transaction with the given notes, executes it, proves it, and submits using the
    /// local miden-client. This results in submitting the transaction to the node and updating the
    /// local db to track the created notes.
    #[instrument(target = COMPONENT, name = "faucet.mint.create_tx", skip_all, err, fields(num_notes, tx_id))]
    async fn create_transaction(&mut self, notes: Vec<Note>) -> Result<TransactionId, ClientError> {
        let span = tracing::Span::current();
        span.record("num_notes", notes.len() as u64);
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

        let tx_request = TransactionRequestBuilder::new()
            .custom_script(self.script.clone())
            .extend_advice_map(advice_map)
            .expected_output_recipients(expected_output_recipients)
            .script_arg(note_data_commitment)
            .build()?;

        // Execute the transaction
        let exec_span = tracing::info_span!(target: COMPONENT, "faucet.mint.execute");
        let tx_result = {
            let _enter = exec_span.enter();
            Box::pin(self.client.new_transaction(self.id.account_id, tx_request)).await?
        };
        let tx_id = tx_result.executed_transaction().id();
        tracing::Span::current().record("tx_id", tx_id.to_string());

        // Prove and submit the transaction
        let prove_remote_span = tracing::info_span!(target: COMPONENT, "faucet.mint.prove_remote");
        let prover_failed = {
            let _enter = prove_remote_span.enter();
            Box::pin(
                self.client
                    .submit_transaction_with_prover(tx_result.clone(), self.tx_prover.clone()),
            )
            .await
            .is_err()
        };
        if prover_failed {
            warn!("Failed to prove transaction with remote prover, falling back to local prover");
            let submit_local_span =
                tracing::info_span!(target: COMPONENT, "faucet.mint.prove_local_and_submit");
            {
                let _enter = submit_local_span.enter();
                Box::pin(self.client.submit_transaction(tx_result)).await?;
            }
        }

        Ok(tx_id)
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
