use miden_client::account::AccountId;
use miden_client::note::NoteId;
use miden_client::transaction::TransactionId;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::types::{AssetAmount, NoteType};

pub type MintResponseSender = oneshot::Sender<Result<MintResponse, MintError>>;
pub type MintRequestSender = mpsc::Sender<(MintRequest, MintResponseSender)>;

/// Response from the `/pow` endpoint.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetPowResponse {
    pub challenge: String,
    pub target: u64,
    pub timestamp: u64,
}

/// Response from the `/get_tokens` endpoint.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetTokensResponse {
    pub tx_id: String,
    pub note_id: String,
}

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
}

#[derive(Debug, thiserror::Error)]
pub enum MintError {
    #[error("faucet supply exceeded")]
    AvailableSupplyExceeded,
}
