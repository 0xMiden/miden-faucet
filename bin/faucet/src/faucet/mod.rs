use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use miden_client::{
    Client, ClientError, Felt, RemoteTransactionProver,
    account::{AccountFile, AccountId, NetworkId, component::BasicFungibleFaucet},
    asset::FungibleAsset,
    builder::ClientBuilder,
    crypto::RpoRandomCoin,
    keystore::FilesystemKeyStore,
    note::{NoteError, NoteId, create_p2id_note},
    rpc::Endpoint,
    transaction::{
        LocalTransactionProver, OutputNote, TransactionId, TransactionProver,
        TransactionRequestBuilder,
    },
};
use miden_node_utils::crypto::get_rpo_random_coin;
use rand::{rng, rngs::StdRng};
use serde::Serialize;
use tokio::sync::mpsc::Receiver;
use tracing::{info, instrument, warn};
use updates::{ClientUpdater, MintUpdate};
use url::Url;

use crate::types::{AssetAmount, NoteType};

mod updates;

pub use updates::MintResponseSender;

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

/// A request for minting to the [`Faucet`].
pub struct MintRequest {
    /// Destination account.
    pub account_id: AccountId,
    /// Whether to generate a public or private note to hold the minted asset.
    pub note_type: NoteType,
    /// The amount to mint.
    pub asset_amount: AssetAmount,
}

/// Stores the current faucet state and handles minting requests.
pub struct Faucet {
    id: FaucetId,
    decimals: u8,
    client: Client,
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
    pub async fn run(
        mut self,
        mut requests: Receiver<(MintRequest, MintResponseSender)>,
    ) -> anyhow::Result<()> {
        let mut buffer = Vec::new();
        let limit = 256; // also limited by the queue size `REQUESTS_QUEUE_SIZE`

        while requests.recv_many(&mut buffer, limit).await > 0 {
            // Skip requests where the user no longer cares about the result.
            let (requests, response_senders): (Vec<MintRequest>, Vec<MintResponseSender>) = buffer
                .drain(..)
                .filter(|(request, response_sender)| {
                    if response_sender.is_closed() {
                        tracing::info!(request.account_id=%request.account_id, "request cancelled");
                        false
                    } else {
                        true
                    }
                })
                .unzip();

            let updater = ClientUpdater::new(response_senders);

            let (tx_id, note_ids) = self.handle_request_batch(&requests, &updater).await?;

            for note_id in note_ids {
                updater
                    .send_updates(MintUpdate::Minted(note_id, tx_id, self.id.network_id))
                    .await;
            }

            self.client.sync_state().await?;
        }

        tracing::info!("Request stream closed, shutting down minter");

        Ok(())
    }

    /// Fully handles a batch of requests to create and submit a transaction.
    ///
    /// For each mint request, a mint note is built. Then, with these notes, a transaction is
    /// created, executed, and submitted using the local miden-client. This results in submitting
    /// the transaction to the node and updating the local db to track the created notes.
    async fn handle_request_batch(
        &mut self,
        requests: &[MintRequest],
        updater: &ClientUpdater,
    ) -> Result<(TransactionId, Vec<NoteId>), ClientError> {
        let mut rng = get_rpo_random_coin(&mut rng());

        // Build the notes
        let notes = build_p2id_notes(self.id, self.decimals, requests, &mut rng)?;
        let note_ids = notes.iter().map(OutputNote::id).collect();
        let tx = TransactionRequestBuilder::new().own_output_notes(notes).build()?;
        updater.send_updates(MintUpdate::Built).await;

        // Execute the transaction
        let tx_result = self.client.new_transaction(self.id.account_id, tx).await?;
        let tx_id = tx_result.executed_transaction().id();
        updater.send_updates(MintUpdate::Executed).await;

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
        updater.send_updates(MintUpdate::Submitted).await;

        Ok((tx_id, note_ids))
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

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use miden_client::{
        account::{AccountBuilder, AccountStorageMode, AccountType, component::RpoFalcon512},
        asset::TokenSymbol,
        auth::AuthSecretKey,
        crypto::SecretKey,
        note::BlockNumber,
        rpc::{NodeRpcClient, TonicRpcClient},
        store::TransactionFilter,
    };
    use miden_node_utils::crypto::get_rpo_random_coin;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use tokio::time::{Instant, sleep};
    use url::Url;

    use super::*;
    use crate::{stub_rpc_api::serve_stub, types::AssetOptions};

    // This test ensures that the faucet can create a transaction that outputs a batch of notes.
    #[allow(clippy::cast_sign_loss)]
    #[tokio::test]
    async fn faucet_batches_requests() {
        let stub_node_url = Url::from_str("http://localhost:50052").unwrap();

        // Start the stub node
        tokio::spawn({
            let stub_node_url = stub_node_url.clone();
            async move { serve_stub(&stub_node_url).await.unwrap() }
        });
        // Wait for the stub node to serve requests
        let rpc_client =
            TonicRpcClient::new(&Endpoint::try_from(stub_node_url.as_str()).unwrap(), 1000);
        let start = Instant::now();
        while rpc_client
            .get_block_header_by_number(Some(BlockNumber::GENESIS), false)
            .await
            .is_err()
        {
            sleep(Duration::from_millis(100)).await;
            assert!(start.elapsed() < Duration::from_secs(5), "stub node took too long to start");
        }

        // Create the faucet
        let mut faucet = {
            let mut rng = ChaCha20Rng::from_seed(rand::random());
            let secret = SecretKey::with_rng(&mut get_rpo_random_coin(&mut rng));
            let symbol = TokenSymbol::try_from("MIDEN").unwrap();
            let decimals = 2;
            let max_supply = Felt::try_from(1_000_000_000_000u64).unwrap();
            let (account, account_seed) = AccountBuilder::new(rng.random())
                .account_type(AccountType::FungibleFaucet)
                .storage_mode(AccountStorageMode::Public)
                .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap())
                .with_auth_component(RpoFalcon512::new(secret.public_key()))
                .build()
                .unwrap();
            let account_file = AccountFile::new(
                account,
                Some(account_seed),
                vec![AuthSecretKey::RpoFalcon512(secret)],
            );

            Faucet::load(
                PathBuf::from("faucet_client_store.sqlite3"),
                NetworkId::Testnet,
                account_file,
                &stub_node_url,
                Duration::from_secs(10),
                None,
            )
            .await
            .unwrap()
        };

        // Create a set of mint requests
        let num_requests = 5;
        let requests = (0..num_requests)
            .map(|i| {
                let account_id = (i as u128).try_into().unwrap();
                MintRequest {
                    account_id,
                    asset_amount: AssetOptions::new(vec![100]).unwrap().validate(100).unwrap(),
                    note_type: NoteType::Public,
                }
            })
            .collect::<Vec<_>>();

        let (tx_id, note_ids) = faucet
            .handle_request_batch(&requests, &ClientUpdater::new(vec![]))
            .await
            .unwrap();

        let tx = faucet
            .client
            .get_transactions(TransactionFilter::Ids(vec![tx_id]))
            .await
            .unwrap()
            .first()
            .unwrap()
            .clone();
        assert_eq!(tx.details.output_notes.num_notes(), num_requests as usize);
        assert_eq!(
            tx.details.output_notes.iter().map(OutputNote::id).collect::<Vec<_>>(),
            note_ids
        );
    }
}
