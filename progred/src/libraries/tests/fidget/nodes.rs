//! Field nodes as values, for tests to build fields from.

use super::super::vocabulary;
use gid::{CellId, Value};

pub fn node(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::isa::record(kind, fields)
}

pub fn axis(axis: CellId) -> Value {
    node(vocabulary::kind::AXIS, [(vocabulary::AXIS, axis.into())])
}

pub fn unary(kind: CellId, operand: Value) -> Value {
    node(kind, [(vocabulary::OPERAND, operand)])
}

pub fn binary(kind: CellId, left: Value, right: Value) -> Value {
    node(kind, [(vocabulary::LEFT, left), (vocabulary::RIGHT, right)])
}
