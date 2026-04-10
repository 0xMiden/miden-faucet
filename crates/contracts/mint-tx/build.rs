/// Computes `StorageSlotId` felts from a slot name string using the same algorithm as
/// `miden_core::utils::hash_string_to_word` (blake3 → 4 little-endian u64s mod Goldilocks prime).
fn main() {
    let name = "miden::standards::fungible_faucets::metadata";
    let digest = blake3::hash(name.as_bytes());
    let bytes = digest.as_bytes();

    let felt = |i: usize| -> u64 {
        let raw = u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap());
        // Goldilocks prime: 2^64 - 2^32 + 1
        const PRIME: u128 = (1 << 64) - (1 << 32) + 1;
        (raw as u128 % PRIME) as u64
    };

    // StorageSlotId: suffix = word[0], prefix = word[1]
    println!("cargo::rustc-env=METADATA_SLOT_SUFFIX={}", felt(0));
    println!("cargo::rustc-env=METADATA_SLOT_PREFIX={}", felt(1));
}
