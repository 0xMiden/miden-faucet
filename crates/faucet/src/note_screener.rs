// Implement a custom note screener that only marks notes as relevant if they are tagged. It
// discards all other notes.
use std::sync::Arc;

use miden_client::ClientError;
use miden_client::rpc::domain::note::CommittedNote;
use miden_client::store::{InputNoteRecord, NoteFilter, Store};
use miden_client::sync::{NoteUpdateAction, OnNoteReceived};

/// Provides functionality for testing whether a note is relevant to the faucet.
///
/// Relevance is based on whether the note is a tracked output note.
pub struct NoteScreener {
    /// A reference to the faucet's store, used to fetch tracked output notes.
    store: Arc<dyn Store>,
}

impl NoteScreener {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait(?Send)]
impl OnNoteReceived for NoteScreener {
    /// Queries the store for the committed note to check if it's a tracked output note.
    async fn on_note_received(
        &self,
        committed_note: CommittedNote,
        _public_note: Option<InputNoteRecord>,
    ) -> Result<NoteUpdateAction, ClientError> {
        let note_id = *committed_note.note_id();

        let output_note_present =
            !self.store.get_output_notes(NoteFilter::Unique(note_id)).await?.is_empty();

        if output_note_present {
            // The note is being tracked by the client so it is relevant
            Ok(NoteUpdateAction::Commit(committed_note))
        } else {
            Ok(NoteUpdateAction::Discard)
        }
    }
}
