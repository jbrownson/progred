//! u64 numbers: literals of the u64 kind, eight little-endian bytes, held
//! natively like f64's. Privileged knowledge is an accelerator only: the
//! convention must remain expressible as an ordinary external library.

use gid::{CellId, Value};

pub const U64: CellId = CellId::from_u128(0xfc77282178c2fa31b80aed2b8b6f7888);
/// Why a value isn't a u64, carrying it.
pub const NOT_U64: CellId = CellId::from_u128(0xae77a805a7ce41dbac03b292d2685918);

pub fn value(number: u64) -> Value {
    crate::literal::value(U64, encode(number))
}

pub fn read(value: &Value) -> Option<u64> {
    crate::literal::bytes(value, U64).and_then(decode)
}

/// The bytes of a u64 literal holding `number`.
pub(crate) fn encode(number: u64) -> Vec<u8> {
    number.to_le_bytes().to_vec()
}

/// The number a u64 literal's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<u64> {
    <[u8; 8]>::try_from(bytes).ok().map(u64::from_le_bytes)
}
