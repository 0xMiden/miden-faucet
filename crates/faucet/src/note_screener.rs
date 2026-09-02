//! Implements a custom note screener that keeps the faucet's tracked output notes and the P2ID
//! notes that fund the operator. It discards all other notes.
use std::sync::Arc;

use miden_client::account::AccountId;
use miden_client::rpc::domain::note::CommittedNote;
use miden_client::store::{InputNoteRecord, NoteFilter, Store};
use miden_client::sync::{NoteUpdateAction, OnNoteReceived};
use miden_client::{ClientError, async_trait};

use crate::is_p2id_payable_to;

/// Provides functionality for testing whether a note is relevant to the faucet.
///
/// A note is relevant if it is a tracked output note, or a public P2ID note payable to the
/// operator. The latter fund the operator and are consumed by the next mint transaction.
#[derive(Clone)]
pub struct NoteScreener {
    /// A reference to the faucet's store, used to fetch tracked output notes.
    store: Arc<dyn Store>,
    /// The operator account that executes the mint transactions.
    operator_account_id: AccountId,
}

impl NoteScreener {
    pub fn new(store: Arc<dyn Store>, operator_account_id: AccountId) -> Self {
        Self { store, operator_account_id }
    }
}

#[async_trait(?Send)]
impl OnNoteReceived for NoteScreener {
    /// Queries the store for the committed note to check if it's a tracked output note, and
    /// otherwise checks whether it is a P2ID note payable to the operator.
    async fn on_note_received(
        &self,
        committed_note: CommittedNote,
        public_note: Option<InputNoteRecord>,
    ) -> Result<NoteUpdateAction, ClientError> {
        let note_id = *committed_note.note_id();

        let output_note_present =
            !self.store.get_output_notes(NoteFilter::Unique(note_id)).await?.is_empty();

        if output_note_present {
            // The note is being tracked by the client so it is relevant
            return Ok(NoteUpdateAction::Commit(committed_note));
        }

        // A private note carries no details, so it cannot be recognized as a funding note.
        match public_note {
            Some(note) if is_p2id_payable_to(note.details(), self.operator_account_id) => {
                Ok(NoteUpdateAction::Insert(note))
            },
            _ => Ok(NoteUpdateAction::Discard),
        }
    }
}
