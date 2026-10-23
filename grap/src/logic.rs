//! The boolean convention, privileged like f64's so comparisons answer
//! natively and stay unboxed through calls and environments. Privileged
//! knowledge is an accelerator only: the convention must remain expressible
//! as an ordinary external library.

use gid::{CellId, Record, Value};

pub const BOOL: CellId = CellId::from_u128(0xb2d13b64336a9e808133d0c7d5e4981e);
pub const TRUE: CellId = CellId::from_u128(0x04831f9b704935059231eb111770e62e);
pub const FALSE: CellId = CellId::from_u128(0x8fcc0a2e5f26c72efd9e917a28919801);
/// Why a value isn't a boolean, carrying it.
pub const NOT_BOOL: CellId = CellId::from_u128(0x0f168ae21a023bdd711339e3aa949f9a);

pub fn value(value: bool) -> Value {
    Value::record([(BOOL, encode(value))])
}

pub fn read(value: &Value) -> Option<bool> {
    read_record(value.as_record()?)
}

pub fn read_record(fields: &Record) -> Option<bool> {
    decode(fields.get(&BOOL)?)
}

/// The `bool` field holding `value`.
pub(crate) fn encode(value: bool) -> Value {
    Value::from(if value { TRUE } else { FALSE })
}

/// The boolean a `bool` field holds.
pub(crate) fn decode(field: &Value) -> Option<bool> {
    match field.as_cell()? {
        TRUE => Some(true),
        FALSE => Some(false),
        _ => None,
    }
}
