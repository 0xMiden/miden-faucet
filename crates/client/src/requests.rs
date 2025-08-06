use miden_client::{account::AccountId, note::NoteId, transaction::TransactionId};
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use crate::types::{AssetAmount, ExplorerUrl, NoteType};

pub type MintResponseSender = oneshot::Sender<MintResponse>;
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
