use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose};
use http::StatusCode;
use miden_client::{
    note::NoteId,
    store::{NoteExportType, NoteFilter},
    utils::Serializable,
};
use serde::Deserialize;
use tracing::instrument;

use crate::{COMPONENT, server::Server};

// ENDPOINT
// ================================================================================================

#[instrument(
    parent = None, target = COMPONENT, name = "faucet.server.get_notes", skip_all,
    fields(
        note_id = %request.note_id,
    )
)]
pub async fn get_notes(
    State(server): State<Server>,
    Query(request): Query<RawNoteRequest>,
) -> Result<impl IntoResponse, NoteRequestError> {
    let request = request.validate()?;
    let note = server
        .store
        .get_output_notes(NoteFilter::Unique(request.note_id))
        .await
        .map_err(|e| {
            tracing::error!("failed to read note from store: {}", e);
            NoteRequestError::NoteNotFound
        })?
        .pop()
        .ok_or(NoteRequestError::NoteNotFound)?;
    let note_file = note.clone().into_note_file(&NoteExportType::NoteDetails).unwrap();
    let encoded_note = general_purpose::STANDARD.encode(note_file.to_bytes());
    let note_json = serde_json::json!({
        "note_id": request.note_id.to_string(),
        "data_base64": encoded_note,
    });

    Ok(Json(note_json))
}

// REQUEST VALIDATION
// ================================================================================================

/// Used to receive the initial `get_pow` request from the user.
#[derive(Deserialize)]
pub struct RawNoteRequest {
    pub note_id: String,
}

impl RawNoteRequest {
    pub fn validate(self) -> Result<NoteRequest, NoteRequestError> {
        let note_id =
            NoteId::try_from_hex(&self.note_id).map_err(|_| NoteRequestError::InvalidNoteId)?;
        Ok(NoteRequest { note_id })
    }
}

/// Validated and parsed `RawPowRequest`.
pub struct NoteRequest {
    pub note_id: NoteId,
}

#[derive(Debug, thiserror::Error)]
pub enum NoteRequestError {
    #[error("note ID failed to parse")]
    InvalidNoteId,
    #[error("note not found")]
    NoteNotFound,
}

impl NoteRequestError {
    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::InvalidNoteId => "Invalid Note ID".to_owned(),
            Self::NoteNotFound => "Note not found".to_owned(),
        }
    }
}

impl IntoResponse for NoteRequestError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.user_facing_error()).into_response()
    }
}
