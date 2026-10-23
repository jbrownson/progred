//! The kind convention: a record's `isa` field names the kind of thing it
//! is, so it stays that kind while it's unfinished and whatever its kind
//! comes to require. Shared like a name: an editor library owns it, and Grap
//! knows it to tell its code from data, a record being a call or a lambda
//! only when its `isa` says so.

use gid::{CellId, Record, Value};

pub const ISA: CellId = CellId::from_u128(0xdbd7e1a38e30425fb1b408b2e446bb1b);

/// The kind `fields` say they are.
pub fn of(fields: &Record) -> Option<CellId> {
    fields.get(&ISA)?.as_cell()
}

/// `fields` saying they're a `kind`.
pub fn record(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record([(ISA, Value::from(kind))].into_iter().chain(fields))
}

/// Runtime `fields`, such as callables, saying they're a `kind`.
pub fn runtime(
    kind: CellId,
    fields: impl IntoIterator<Item = (CellId, crate::RuntimeValue)>,
) -> crate::RuntimeValue {
    crate::RuntimeValue::record([(ISA, Value::from(kind).into())].into_iter().chain(fields))
}
