//! Text: literals of the text kind, `{isa: text, bytes}`, their bytes UTF-8.
//! Privileged like f64's, so text stays unboxed through calls, containers,
//! and environments, and readers borrow it without decoding; it remains an
//! ordinary convention an external library could own.

use gid::{CellId, Value};

pub const TEXT: CellId = CellId::from_u128(0x332529b8ea83a7ba10fd7f6d942e5016);

pub fn value(text: impl Into<String>) -> Value {
    crate::literal::value(TEXT, encode(text))
}

pub fn read(value: &Value) -> Option<&str> {
    crate::literal::bytes(value, TEXT).and_then(decode)
}

/// The bytes of a text literal holding `text`.
pub(crate) fn encode(text: impl Into<String>) -> Vec<u8> {
    text.into().into_bytes()
}

/// The text a text literal's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes).ok()
}
