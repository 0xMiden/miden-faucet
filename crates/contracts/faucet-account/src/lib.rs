#![no_std]
#![feature(alloc_error_handler)]

use miden::{Felt, NoteIdx, NoteType, Recipient, Tag, component, faucet, output_note};

#[component]
struct FaucetAccount;

#[component]
impl FaucetAccount {
    /// Mints fungible assets and sends them to the provided recipient by creating a note.
    ///
    /// This mirrors the `mint_and_send` procedure from the `BasicFungibleFaucet` MASM component.
    pub fn mint_and_send(
        &mut self,
        amount: Felt,
        tag: Tag,
        note_type: NoteType,
        recipient: Recipient,
    ) -> NoteIdx {
        let asset = faucet::create_fungible_asset(amount);
        faucet::mint(asset);
        let note_idx = output_note::create(tag, note_type, recipient);
        output_note::add_asset(asset, note_idx);
        note_idx
    }
}
