use miden_client::account::{AccountId, AccountIdError};
use miden_client::note::NoteId;
use miden_client::transaction::TransactionId;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use crate::types::{AssetAmount, ExplorerUrl, NoteType};

pub type MintResponseSender = oneshot::Sender<Result<MintResponse, MintRequestError>>;
pub type MintRequestSender = mpsc::Sender<(MintRequest, MintResponseSender)>;

/// A request for minting to the Faucet.
pub struct MintRequest {
    /// Destination account.
    pub account_id: AccountId,
    /// Whether to generate a public or private note to hold the minted asset.
    pub note_type: NoteType,
    /// The amount to mint.
    pub asset_amount: AssetAmount,
}

pub struct MintResponse {
    pub tx_id: TransactionId,
    pub note_id: NoteId,
    pub explorer_url: Option<ExplorerUrl>,
}

impl Serialize for MintResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MintResponse", 3)?;
        state.serialize_field("tx_id", &self.tx_id.to_string())?;
        state.serialize_field("note_id", &self.note_id.to_string())?;
        state.serialize_field("explorer_url", &self.explorer_url)?;
        state.end()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MintRequestError {
    #[error("account ID failed to parse")]
    AccountId(#[source] AccountIdError),
    #[error("asset amount {0} is not one of the provided options")]
    AssetAmount(u64),
    #[error("API key {0} is invalid")]
    InvalidApiKey(String),
    #[error("invalid POW solution")]
    InvalidPoW,
    #[error("POW parameters are missing")]
    MissingPowParameters,
    #[error("server signatures do not match")]
    ServerSignaturesDoNotMatch,
    #[error("server timestamp expired, received: {0}, current time: {1}")]
    ExpiredServerTimestamp(u64, u64),
    #[error("challenge already used")]
    ChallengeAlreadyUsed,
    #[error("account is rate limited")]
    RateLimited,
    #[error("faucet supply exceeded")]
    AvailableSupplyExceeded,
}
