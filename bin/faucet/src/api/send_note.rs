use axum::extract::{Query, State};
use axum::response::IntoResponse;
use http::StatusCode;
use miden_client::note::NoteDetails;
use miden_client::note_transport::NoteTransportError;
use miden_client::utils::Serializable;
use tracing::instrument;

use crate::COMPONENT;
use crate::api::ApiServer;
use crate::api::get_note::RawNoteRequest;

// ENDPOINT
// ================================================================================================

#[instrument(
    parent = None, target = COMPONENT, name = "server.send_note", skip_all, err,
    fields(
        note_id = %request.note_id,
    )
)]
pub async fn send_note(
    State(server): State<ApiServer>,
    Query(request): Query<RawNoteRequest>,
) -> Result<(), SendNoteError> {
    let note_transport_client = server
        .note_transport_client
        .ok_or(SendNoteError::NoteTransportError(NoteTransportError::Disabled))?;

    let request = request.validate().map_err(|_| SendNoteError::InvalidNoteId)?;

    // The P2ID note is minted by the network from the faucet's MINT note, so it never reaches the
    // client store. It is served from the cache the faucet populates at mint time instead.
    let note = {
        let cache = server.p2id_notes.read().expect("p2id note cache is poisoned");
        cache.get(&request.note_id.to_hex()).cloned()
    }
    .ok_or(SendNoteError::NoteNotFound)?;

    let header = *note.header();
    let details: NoteDetails = note.into();

    note_transport_client.send_note(header, details.to_bytes()).await?;
    Ok(())
}

// ERRORS
// ================================================================================================

#[derive(Debug, thiserror::Error)]
pub enum SendNoteError {
    #[error("note transport layer error: {0}")]
    NoteTransportError(#[from] NoteTransportError),
    #[error("invalid note ID")]
    InvalidNoteId,
    #[error("note not found")]
    NoteNotFound,
}

impl SendNoteError {
    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::NoteTransportError(_) => {
                "Failed to send note through note transport layer".to_owned()
            },
            Self::InvalidNoteId => "Invalid Note ID".to_owned(),
            Self::NoteNotFound => "Note not found".to_owned(),
        }
    }
}

impl IntoResponse for SendNoteError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.user_facing_error()).into_response()
    }
}
