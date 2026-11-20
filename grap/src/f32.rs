//! f32 numbers: literals of the f32 kind, four little-endian bytes, held
//! natively like f64's. Privileged knowledge is an accelerator only: the
//! convention must remain expressible as an ordinary external library.

use gid::{CellId, Value};

pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
/// Why a value isn't an f32, carrying it.
pub const NOT_F32: CellId = CellId::from_u128(0x6a124272d8df415ab944020cd1fd352b);

pub fn value(number: f32) -> Value {
    crate::literal::value(F32, encode(number))
}

pub fn read(value: &Value) -> Option<f32> {
    crate::literal::bytes(value, F32).and_then(decode)
}

/// The bytes of a f32 literal holding `number`.
pub(crate) fn encode(number: f32) -> Vec<u8> {
    number.to_le_bytes().to_vec()
}

/// The number a f32 literal's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<f32> {
    <[u8; 4]>::try_from(bytes).ok().map(f32::from_le_bytes)
}
