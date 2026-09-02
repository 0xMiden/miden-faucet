//! Implements a custom note screener that only marks notes as relevant if the faucet already
//! tracks them. It discards all other notes.
use std::sync::Arc;

use miden_client::rpc::domain::note::CommittedNote;
use miden_client::store::{InputNoteRecord, NoteFilter, Store};
use miden_client::sync::{NoteUpdateAction, OnNoteReceived};
use miden_client::{ClientError, async_trait};

/// Provides functionality for testing whether a note is relevant to the faucet.
///
/// Relevance is based on whether the note is already tracked: an output note the faucet created,
/// or an input note it expects. The faucet tracks the P2ID notes that fund its operator when it
/// requests them, so those are recognized here without any further checks.
#[derive(Clone)]
pub struct NoteScreener {
    /// A reference to the faucet's store, used to fetch tracked notes.
    store: Arc<dyn Store>,
}

impl NoteScreener {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl OnNoteReceived for NoteScreener {
    /// Queries the store for the committed note to check whether the faucet tracks it.
    async fn on_note_received(
        &self,
        committed_note: CommittedNote,
        public_note: Option<InputNoteRecord>,
    ) -> Result<NoteUpdateAction, ClientError> {
        let note_id = *committed_note.note_id();

        let output_note_present =
            !self.store.get_output_notes(NoteFilter::Unique(note_id)).await?.is_empty();
        if output_note_present {
            return Ok(NoteUpdateAction::Commit(committed_note));
        }

        // An expected note imported from its details carries no metadata, and so no note id. It is
        // matched by its details commitment instead, which the note the node served carries.
        if let Some(note) = public_note {
            let input_note_present = !self
                .store
                .get_input_notes(NoteFilter::DetailsCommitments(vec![note.details_commitment()]))
                .await?
                .is_empty();
            if input_note_present {
                return Ok(NoteUpdateAction::Commit(committed_note));
            }
        }

        Ok(NoteUpdateAction::Discard)
    }
}
