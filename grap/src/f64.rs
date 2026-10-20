//! The f64 number convention, privileged so native numbers stay
//! unboxed through calls, containers, and environments. Privileged
//! knowledge is an accelerator only: the convention must remain
//! expressible as an ordinary external library, which would evaluate
//! correctly and merely lose the fast paths.

use gid::{CellId, Record, Value};

pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);
/// Why a value isn't an f64, carrying it.
pub const NOT_F64: CellId = CellId::from_u128(0x4aabc44d46a2402dbb9ef64e377632db);

pub fn value(number: f64) -> Value {
    Value::record([(F64, encode(number))])
}

pub fn read(value: &Value) -> Option<f64> {
    read_record(value.as_record()?)
}

pub fn read_record(fields: &Record) -> Option<f64> {
    fields.get(&F64).and_then(Value::as_blob).and_then(decode)
}

/// `number` as a count or an index: whole, not negative, and small
/// enough to convert exactly.
pub fn to_usize(number: f64) -> Option<usize> {
    (number >= 0.0 && number.fract() == 0.0 && number < usize::MAX as f64)
        .then_some(number as usize)
}

/// The `f64` field holding `number`.
pub(crate) fn encode(number: f64) -> Value {
    Value::from(number.to_le_bytes().to_vec())
}

/// The number an `f64` field's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<f64> {
    <[u8; 8]>::try_from(bytes).ok().map(f64::from_le_bytes)
}
