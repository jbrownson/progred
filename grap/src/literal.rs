//! Literals: values whose kind reads them from their encoded bytes, `{isa:
//! kind, bytes}`, such as a number, a text, a color or an image. The kind
//! says how the bytes read; other fields beside them, such as a name, are the
//! value's too.

use crate::vocabulary::BYTES;
use gid::{CellId, Record, Value};

/// A literal of `kind` holding `bytes`.
pub fn value(kind: CellId, bytes: Vec<u8>) -> Value {
    Value::record(fields(kind, bytes))
}

/// A literal's own fields: its kind, then its bytes.
pub fn fields(kind: CellId, bytes: Vec<u8>) -> [(CellId, Value); 2] {
    [
        (crate::isa::ISA, Value::from(kind)),
        (BYTES, Value::from(bytes)),
    ]
}

/// The bytes `value` holds as a literal of `kind`.
pub fn bytes(value: &Value, kind: CellId) -> Option<&[u8]> {
    of(value.as_record()?, kind)
}

/// The bytes `fields` hold as a literal of `kind`.
pub fn of(fields: &Record, kind: CellId) -> Option<&[u8]> {
    (crate::isa::of(fields)? == kind).then_some(())?;
    fields.get(&BYTES)?.as_blob()
}
