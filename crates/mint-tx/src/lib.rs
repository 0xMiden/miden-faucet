#![no_std]
#![feature(alloc_error_handler)]

use miden::faucet::mint;
use miden::intrinsics::advice::adv_push_mapvaln;
use miden::tx::update_expiration_block_delta;
use miden::{Asset, Felt, Word, adv_load_preimage, felt, tx_script};

const ASSET_WORD_SIZE: usize = 4;

#[tx_script]
fn run(arg: Word) {
    update_expiration_block_delta(Felt::from_u32(10));

    let num_felts = adv_push_mapvaln(arg);
    let num_felts_u64 = num_felts.as_u64();
    assert_eq!(Felt::from_u32((num_felts_u64 % 4) as u32), felt!(0));

    let num_words = Felt::from_u64_unchecked(num_felts_u64 / 4);
    let commitment = arg;
    let input = adv_load_preimage(num_words, commitment);

    let num_words_usize = num_words.as_u64() as usize;
    for idx in 0..num_words_usize {
        let start = idx * ASSET_WORD_SIZE;
        let end = start + ASSET_WORD_SIZE;
        let asset: [Felt; ASSET_WORD_SIZE] =
            input[start..end].try_into().expect("invalid input length");
        mint(Asset::new(asset));
    }
}
