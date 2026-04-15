#![no_std]
#![feature(alloc_error_handler)]

use miden::intrinsics::advice::adv_push_mapvaln;
use miden::tx::update_expiration_block_delta;
use miden::{Felt, Recipient, Word, pipe_words_to_memory, tx_script};

use crate::bindings::Account;

/// Number of felts per note: 4 (recipient) + 1 (note_type) + 1 (tag) + 1 (amount) = 7.
const NOTE_ARGS_SIZE: usize = 7;

#[tx_script]
fn run(arg: Word, account: &mut Account) {
    update_expiration_block_delta(Felt::from_u32(10));

    // Push note data from advice map onto the advice stack.
    let num_felts = adv_push_mapvaln(arg);
    let num_felts_u64 = num_felts.as_canonical_u64();

    // Pop the data from the advice stack into memory.
    let num_words = Felt::new((num_felts_u64 + 3) / 4);
    let (_hash, input) = pipe_words_to_memory(num_words);

    let num_notes = num_felts_u64 as usize / NOTE_ARGS_SIZE;

    for idx in 0..num_notes {
        let start = idx * NOTE_ARGS_SIZE;
        let recipient = Recipient::from(Word::from([
            input[start],
            input[start + 1],
            input[start + 2],
            input[start + 3],
        ]));
        let note_type = input[start + 4].into();
        let tag = input[start + 5].into();
        let amount = input[start + 6];

        account.mint_and_send(amount, tag, note_type, recipient);
    }
}
