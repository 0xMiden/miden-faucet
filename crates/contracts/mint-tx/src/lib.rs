#![no_std]
#![feature(alloc_error_handler)]

use miden::intrinsics::advice::adv_push_mapvaln;
use miden::tx::update_expiration_block_delta;
use miden::{Felt, Word, active_account, adv_load_preimage, felt, tx_script};

const NOTE_ARGS_SIZE: usize = 4;

#[tx_script]
fn run(arg: Word) {
    update_expiration_block_delta(Felt::from_u32(10));

    let commitment = arg.reverse();
    let num_felts = adv_push_mapvaln(commitment);
    let num_felts_u64 = num_felts.as_u64();
    assert_eq!(Felt::from_u32((num_felts_u64 % 7) as u32), felt!(0));

    let num_words = Felt::from_u64_unchecked(num_felts_u64 / 4);
    let mut input = adv_load_preimage(num_words, commitment);
    input.reverse(); // TODO: check if reverse is needed due to pop

    let num_words_usize = num_words.as_u64() as usize;
    for idx in 0..num_words_usize {
        let start = idx * NOTE_ARGS_SIZE;
        let recipient: [Felt; 4] =
            input[start..start + 4].try_into().expect("invalid input length");
        let note_type = input[start + 4];
        let tag = input[start + 5];
        let amount = input[start + 6];

        // TODO: call distribute account procedure
        let num_procedures = active_account::get_num_procedures();
        assert_ne!(num_procedures, Felt::from_u32(0));
    }
}
