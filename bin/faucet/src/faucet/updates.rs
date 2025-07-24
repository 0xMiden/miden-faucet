use std::convert::Infallible;

use axum::response::sse::Event;
use miden_client::{account::NetworkId, note::NoteId, transaction::TransactionId};
use tokio::sync::mpsc;

use crate::network::ExplorerUrl;

pub type MintResponseSender = mpsc::Sender<Result<Event, Infallible>>;

/// Sends updates on the minting process to all the clients waiting for a batch of mint requests to
/// be processed.
pub struct ClientUpdater {
    clients: Vec<MintResponseSender>,
}

impl ClientUpdater {
    /// Creates a new client updater.
    pub fn new(clients: Vec<MintResponseSender>) -> Self {
        Self { clients }
    }

    /// Sends an update to all the batch clients.
    /// Errors when sending through the channel are ignored since the client may have cancelled the
    /// request.
    pub async fn send_updates(&self, update: MintUpdate) {
        let event = update.into_event();
        for sender in &self.clients {
            let _ = sender.send(Ok(event.clone())).await;
        }
    }
}

/// The different stages of the minting process.
pub enum MintUpdate {
    Built,
    Executed,
    Submitted,
    Minted(NoteId, TransactionId, NetworkId),
}

impl MintUpdate {
    /// Converts the mint update into an sse event.
    /// Event types:
    /// - `MintUpdate::Built`: event type "update"
    /// - `MintUpdate::Executed`: event type "update"
    /// - `MintUpdate::Submitted`: event type "update"
    /// - `MintUpdate::Minted`: event type "minted". Contains the note id, transaction id and
    ///   explorer url.
    pub fn into_event(self) -> Event {
        match self {
            MintUpdate::Minted(note_id, tx_id, network_id) => {
                let event_payload = serde_json::json!({
                    "note_id": note_id.to_string(),
                    "transaction_id": tx_id.to_string(),
                    "explorer_url": ExplorerUrl::from_network_id(network_id),
                });

                Event::default().event("minted").data(event_payload.to_string())
            },
            MintUpdate::Built => Event::default().event("update").data("Built"),
            MintUpdate::Executed => Event::default().event("update").data("Executed"),
            MintUpdate::Submitted => Event::default().event("update").data("Submitted"),
        }
    }
}
