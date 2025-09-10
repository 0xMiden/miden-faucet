/// Parses a hex string into an array of bytes of known size.
pub fn hex_to_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    // This code is inspired by miden-crypto::utils::hex_to_bytes
    let expected: usize = (N * 2) + 2;
    if value.len() != expected {
        return None;
    }

    if !value.starts_with("0x") {
        return None;
    }

    let mut data = value.bytes().skip(2).map(|v| match v {
        b'0'..=b'9' => Ok(v - b'0'),
        b'a'..=b'f' => Ok(v - b'a' + 10),
        b'A'..=b'F' => Ok(v - b'A' + 10),
        _ => Err(()),
    });

    let mut decoded = [0u8; N];
    for byte in decoded.iter_mut() {
        // These `unwrap` calls are okay because the length was checked above
        let high: u8 = data.next().unwrap().ok()?;
        let low: u8 = data.next().unwrap().ok()?;
        *byte = (high << 4) + low;
    }

    Some(decoded)
}

/// Converts a byte array to a hex string with a leading `0x` prefix.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut result = String::new();
    result.push_str("0x");
    for byte in bytes.iter() {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}
