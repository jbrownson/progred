//! Bytes spelled as hex digit pairs, as blobs and list positions are.

pub fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The bytes that hex digit pairs spell, in either case; [`hex_string`]
/// writes them.
pub fn read_hex(hex: &str) -> Option<Vec<u8>> {
    let digit = |byte: u8| char::from(byte).to_digit(16);
    hex.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => Some((digit(*high)? << 4 | digit(*low)?) as u8),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/hex.rs"]
mod tests;
