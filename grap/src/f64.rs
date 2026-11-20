//! f64 numbers: literals of the f64 kind, `{isa: f64, bytes}`, eight
//! little-endian bytes, privileged so native numbers stay unboxed through
//! calls, containers, and environments. Privileged
//! knowledge is an accelerator only: the convention must remain
//! expressible as an ordinary external library, which would evaluate
//! correctly and merely lose the fast paths.

use gid::{CellId, Value};

pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);
/// Why a value isn't an f64, carrying it.
pub const NOT_F64: CellId = CellId::from_u128(0x4aabc44d46a2402dbb9ef64e377632db);

pub fn value(number: f64) -> Value {
    crate::literal::value(F64, encode(number))
}

pub fn read(value: &Value) -> Option<f64> {
    crate::literal::bytes(value, F64).and_then(decode)
}

/// `number` as a count or an index: whole, not negative, and small
/// enough to convert exactly.
pub fn to_usize(number: f64) -> Option<usize> {
    (number >= 0.0 && number.fract() == 0.0 && number < usize::MAX as f64)
        .then_some(number as usize)
}

/// The bytes of a f64 literal holding `number`.
pub(crate) fn encode(number: f64) -> Vec<u8> {
    number.to_le_bytes().to_vec()
}

/// The number a f64 literal's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<f64> {
    <[u8; 8]>::try_from(bytes).ok().map(f64::from_le_bytes)
}
