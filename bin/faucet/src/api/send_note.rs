use axum::extract::{Query, State};
use axum::response::IntoResponse;
use http::StatusCode;
use miden_client::ClientError;
use miden_client::account::{AccountId, Address};
use miden_client::note::Note;
use miden_client::store::NoteFilter;
use tracing::{Instrument, info_span, instrument};

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
    let request = request.validate().map_err(|_| SendNoteError::InvalidNoteId)?;

    let note_record = server
        .store
        .get_output_notes(NoteFilter::Unique(request.note_id))
        .instrument(info_span!(target: COMPONENT, "store.get_output_notes"))
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to read note from store");
            SendNoteError::NoteNotFound
        })?
        .pop()
        .ok_or(SendNoteError::NoteNotFound)?;
    let note = Note::try_from(note_record).expect("note record should be valid");

    // Write lock to send via note transport.
    // TODO: use actual recipient address when e2ee is implemented.
    let address = Address::new(AccountId::try_from(0).expect("valid account id"));
    let mut client = server.client.write().await;
    client.send_private_note(note, &address).await?;

    Ok(())
}

// ERRORS
// ================================================================================================

#[derive(Debug, thiserror::Error)]
pub enum SendNoteError {
    #[error("client error: {0}")]
    ClientError(#[from] ClientError),
    #[error("invalid note ID")]
    InvalidNoteId,
    #[error("note not found")]
    NoteNotFound,
}

impl SendNoteError {
    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::ClientError(_) => "Failed to send note through note transport layer".to_owned(),
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
