//! The f32 number convention, held natively like f64's: four little-endian
//! bytes under the f32 tag. Privileged knowledge is an accelerator only: the
//! convention must remain expressible as an ordinary external library.

use gid::{CellId, Record, Value};

pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
/// Why a value isn't an f32, carrying it.
pub const NOT_F32: CellId = CellId::from_u128(0x6a124272d8df415ab944020cd1fd352b);

pub fn value(number: f32) -> Value {
    Value::record([(F32, encode(number))])
}

pub fn read(value: &Value) -> Option<f32> {
    read_record(value.as_record()?)
}

pub fn read_record(fields: &Record) -> Option<f32> {
    fields.get(&F32).and_then(Value::as_blob).and_then(decode)
}

/// The `f32` field holding `number`.
pub(crate) fn encode(number: f32) -> Value {
    Value::from(number.to_le_bytes().to_vec())
}

/// The number a `f32` field's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<f32> {
    <[u8; 4]>::try_from(bytes).ok().map(f32::from_le_bytes)
}
