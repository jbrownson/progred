//! GID values and cells: every shape — atom, list, record — is a pure
//! structural value compared by content; identity is a cell, a minted
//! 128-bit id whose current value lives in the `Cells` table. Values are
//! finite trees; the graph lives in the links. One canonical spelling
//! per value, owned by the constructors. See `docs/gid.md`.

use crate::blob::Blob;
use crate::cell_id::CellId;
use crate::{List, Record};
use std::fmt;

/// A value: anything sayable — pure structure, no identity of its
/// own. Cycles are unrepresentable here; they exist only by a cell's
/// value linking back through `Value::Cell`. Positions are element
/// identity, saved with the document, but lists compare without them:
/// two occurrences of `[2, 3]` are the same value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Cell(CellId),
    Blob(Blob),
    List(List),
    Record(Record),
}

impl Value {
    /// Builds a list, minting evenly spread positions.
    pub fn list(elements: impl IntoIterator<Item = Value>) -> Value {
        Value::List(elements.into_iter().collect())
    }

    pub fn record(fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
        Value::Record(fields.into_iter().collect())
    }

    pub fn as_cell(&self) -> Option<CellId> {
        match self {
            Value::Cell(cell) => Some(*cell),
            _ => None,
        }
    }

    pub fn as_blob(&self) -> Option<&[u8]> {
        match self {
            Value::Blob(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&List> {
        match self {
            Value::List(elements) => Some(elements),
            _ => None,
        }
    }

    pub fn as_record(&self) -> Option<&Record> {
        match self {
            Value::Record(fields) => Some(fields),
            _ => None,
        }
    }
}

impl From<CellId> for Value {
    fn from(cell: CellId) -> Self {
        Value::Cell(cell)
    }
}
impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Value::Blob(bytes.into())
    }
}

impl From<Blob> for Value {
    fn from(bytes: Blob) -> Self {
        Value::Blob(bytes)
    }
}

impl From<List> for Value {
    fn from(elements: List) -> Self {
        Value::List(elements)
    }
}

impl From<Record> for Value {
    fn from(fields: Record) -> Self {
        Value::Record(fields)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Cell(cell) => cell.fmt(f),
            Value::Blob(bytes) => bytes.fmt(f),
            Value::List(elements) => elements.fmt(f),
            Value::Record(fields) => fields.fmt(f),
        }
    }
}

#[cfg(test)]
#[path = "tests/value.rs"]
mod tests;
