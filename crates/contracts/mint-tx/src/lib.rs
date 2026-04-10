#![no_std]
#![feature(alloc_error_handler)]

use miden::intrinsics::advice::adv_push_mapvaln;
use miden::tx::update_expiration_block_delta;
use miden::{
    Felt,
    Recipient,
    StorageSlotId,
    Word,
    faucet,
    output_note,
    pipe_words_to_memory,
    storage,
    tx_script,
};

/// Number of felts per note: 4 (recipient) + 1 (note_type) + 1 (tag) + 1 (amount) = 7.
const NOTE_ARGS_SIZE: usize = 7;

/// Returns the metadata storage slot ID for `"miden::standards::fungible_faucets::metadata"`.
/// Layout: [token_symbol, decimals, max_supply, token_supply].
///
/// The slot ID is computed at build time from the slot name via blake3 (see build.rs).
fn metadata_slot() -> StorageSlotId {
    const PREFIX: u64 = {
        let bytes = env!("METADATA_SLOT_PREFIX").as_bytes();
        parse_u64(bytes)
    };
    const SUFFIX: u64 = {
        let bytes = env!("METADATA_SLOT_SUFFIX").as_bytes();
        parse_u64(bytes)
    };
    StorageSlotId::from_prefix_suffix(Felt::new(PREFIX), Felt::new(SUFFIX))
}

/// Parses a u64 from a byte slice of ASCII digits at compile time.
const fn parse_u64(bytes: &[u8]) -> u64 {
    let mut result: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        result = result * 10 + (bytes[i] - b'0') as u64;
        i += 1;
    }
    result
}

#[tx_script]
fn run(arg: Word) {
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

        // Read the metadata slot and update token supply.
        let metadata = storage::get_item(metadata_slot());
        let token_supply = metadata[3];
        let max_supply = metadata[2];

        let new_supply = token_supply.as_canonical_u64() + amount.as_canonical_u64();
        assert!(new_supply <= max_supply.as_canonical_u64());

        let new_metadata =
            Word::from([metadata[0], metadata[1], metadata[2], Felt::new(new_supply)]);
        storage::set_item(metadata_slot(), new_metadata);

        // Mint the asset and create an output note.
        let asset = faucet::create_fungible_asset(amount);
        faucet::mint(asset);

        let note_idx = output_note::create(tag, note_type, recipient);
        output_note::add_asset(asset, note_idx);
    }
}
