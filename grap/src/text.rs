//! The UTF-8 text convention: a record whose `utf8` field holds the text's
//! bytes. Privileged like f64's, so text stays unboxed through calls,
//! containers, and environments, and readers borrow it without decoding;
//! it remains an ordinary convention an external library could own.

use gid::{CellId, Record, Value};

pub const UTF8: CellId = CellId::from_u128(0x332529b8ea83a7ba10fd7f6d942e5016);

pub fn value(text: impl Into<String>) -> Value {
    Value::record([(UTF8, encode(text))])
}

pub fn read(value: &Value) -> Option<&str> {
    read_record(value.as_record()?)
}

pub fn read_record(fields: &Record) -> Option<&str> {
    fields.get(&UTF8)?.as_blob().and_then(decode)
}

/// The `utf8` field holding `text`.
pub(crate) fn encode(text: impl Into<String>) -> Value {
    Value::from(text.into().into_bytes())
}

/// The text a `utf8` field's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes).ok()
}
