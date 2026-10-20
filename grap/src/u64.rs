//! The u64 number convention, held natively like f64's: eight little-endian
//! bytes under the u64 tag. Privileged knowledge is an accelerator only: the
//! convention must remain expressible as an ordinary external library.

use gid::{CellId, Record, Value};

pub const U64: CellId = CellId::from_u128(0xfc77282178c2fa31b80aed2b8b6f7888);
/// Why a value isn't a u64, carrying it.
pub const NOT_U64: CellId = CellId::from_u128(0xae77a805a7ce41dbac03b292d2685918);

pub fn value(number: u64) -> Value {
    Value::record([(U64, encode(number))])
}

pub fn read(value: &Value) -> Option<u64> {
    read_record(value.as_record()?)
}

pub fn read_record(fields: &Record) -> Option<u64> {
    fields.get(&U64).and_then(Value::as_blob).and_then(decode)
}

/// The `u64` field holding `number`.
pub(crate) fn encode(number: u64) -> Value {
    Value::from(number.to_le_bytes().to_vec())
}

/// The number a `u64` field's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<u64> {
    <[u8; 8]>::try_from(bytes).ok().map(u64::from_le_bytes)
}
