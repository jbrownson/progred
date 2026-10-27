//! GID values and cells: every shape — atom, list, record — is a pure
//! structural value compared by content; identity is a cell, a minted
//! 128-bit id whose current value lives in the `Cells` table. Values are
//! finite trees; the graph lives in the links. One canonical spelling
//! per value, owned by the constructors. See `docs/gid.md`.

use crate::blob::{Blob, hex_bytes, hex_string};
use crate::cell_id::CellId;
use crate::{List, Record};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};

/// A value: anything sayable — pure structure, no identity of its
/// own. Cycles are unrepresentable here; they exist only by a cell's
/// value linking back through `Value::Cell`. Positions are element
/// identity, saved with the document, and the hand-written Eq/Hash
/// below IGNORE them: two occurrences of `[2, 3]` are the same value.
#[derive(Debug, Clone)]
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

    /// `patch` laid over this value: where both are records, these fields
    /// with `patch`'s added or replacing theirs; otherwise `patch`.
    pub fn overlaid(&self, patch: Value) -> Value {
        match (self.as_record(), patch.as_record()) {
            (Some(fields), Some(patched)) => {
                Value::Record(fields.iter().chain(patched).cloned().collect())
            }
            _ => patch,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Cell(a), Value::Cell(b)) => a == b,
            (Value::Blob(a), Value::Blob(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Record(a), Value::Record(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Cell(cell) => {
                0_u8.hash(state);
                cell.hash(state);
            }
            Value::Blob(bytes) => {
                1_u8.hash(state);
                bytes.hash(state);
            }
            Value::List(elements) => {
                2_u8.hash(state);
                elements.hash(state);
            }
            Value::Record(fields) => {
                3_u8.hash(state);
                fields.len().hash(state);
                for (label, value) in fields {
                    label.hash(state);
                    value.hash(state);
                }
            }
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
            Value::Cell(cell) => write!(f, "{cell}"),
            Value::Blob(bytes) => write!(f, "0x{}", hex_string(bytes)),
            Value::List(elements) => {
                write!(f, "[")?;
                for (index, element) in elements.values().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    element.fmt(f)?;
                }
                write!(f, "]")
            }
            Value::Record(fields) => {
                write!(f, "{{")?;
                for (index, (label, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{label}: {value}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueRepr {
    Cell(CellId),
    Blob(String),
    List(Vec<Value>),
    Record(Vec<(CellId, Value)>),
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self {
            Value::Cell(cell) => ValueRepr::Cell(*cell),
            Value::Blob(bytes) => ValueRepr::Blob(hex_string(bytes)),
            Value::List(elements) => ValueRepr::List(elements.values().cloned().collect()),
            // Record storage is sorted by label, so the file's pair
            // order is canonical without an explicit sort.
            Value::Record(fields) => {
                ValueRepr::Record(fields.iter().map(|(k, v)| (*k, v.clone())).collect())
            }
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ValueRepr::deserialize(deserializer)? {
            ValueRepr::Cell(cell) => Ok(Value::from(cell)),
            ValueRepr::Blob(hex) => hex_bytes(&hex)
                .map(Value::from)
                .map_err(serde::de::Error::custom),
            ValueRepr::List(elements) => Ok(Value::list(elements)),
            ValueRepr::Record(fields) => Ok(Value::record(fields)),
        }
    }
}

#[cfg(test)]
#[path = "tests/value.rs"]
mod tests;
