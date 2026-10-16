//! GID values and cells: every shape — atom, list, record — is a pure
//! structural value compared by content; identity is a cell, a minted
//! 128-bit id whose current value lives in the `Cells` table. Values are
//! finite trees; the graph lives in the links. One canonical spelling
//! per value, owned by the constructors. See `docs/gid.md`.

use crate::blob::Blob;
use crate::cell_id::CellId;
use crate::position::Position;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A small immutable map keyed by durable cell identity. Records are
/// kept sorted and contiguous for cheap reads and iteration; clones
/// share their storage, while edits copy only when another value still
/// owns the same record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record(Arc<Vec<(CellId, Value)>>);

impl Record {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &CellId) -> Option<&Value> {
        self.0
            .binary_search_by_key(key, |(field, _)| *field)
            .ok()
            .map(|index| &self.0[index].1)
    }

    pub fn contains_key(&self, key: &CellId) -> bool {
        self.0
            .binary_search_by_key(key, |(field, _)| *field)
            .is_ok()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (CellId, Value)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &CellId> + ExactSizeIterator {
        self.0.iter().map(|(field, _)| field)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.0.iter().map(|(_, value)| value)
    }

    pub fn insert(&mut self, key: CellId, value: Value) -> Option<Value> {
        let fields = Arc::make_mut(&mut self.0);
        match fields.binary_search_by_key(&key, |(field, _)| *field) {
            Ok(index) => Some(std::mem::replace(&mut fields[index].1, value)),
            Err(index) => {
                fields.insert(index, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &CellId) -> Option<Value> {
        let index = self.0.binary_search_by_key(key, |(field, _)| *field).ok()?;
        Some(Arc::make_mut(&mut self.0).remove(index).1)
    }

    pub fn update(&self, key: CellId, value: Value) -> Self {
        let mut next = self.clone();
        next.insert(key, value);
        next
    }

    pub fn without(&self, key: &CellId) -> Self {
        let mut next = self.clone();
        next.remove(key);
        next
    }
}

impl FromIterator<(CellId, Value)> for Record {
    fn from_iter<T: IntoIterator<Item = (CellId, Value)>>(iter: T) -> Self {
        let mut fields: Vec<_> = iter.into_iter().collect();
        fields.sort_by_key(|(field, _)| *field);
        let mut unique = Vec::with_capacity(fields.len());
        for (field, value) in fields {
            if let Some((last, stored)) = unique.last_mut()
                && *last == field
            {
                *stored = value;
            } else {
                unique.push((field, value));
            }
        }
        Self(Arc::new(unique))
    }
}

impl IntoIterator for Record {
    type Item = (CellId, Value);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Arc::try_unwrap(self.0)
            .unwrap_or_else(|shared| (*shared).clone())
            .into_iter()
    }
}

impl<'a> IntoIterator for &'a Record {
    type Item = &'a (CellId, Value);
    type IntoIter = std::slice::Iter<'a, (CellId, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// An ordered sequence whose session-local positions give projected
/// elements stable identities. Like records, lists are contiguous and
/// copy-on-write; positions affect navigation but not value equality.
#[derive(Debug, Clone, Default)]
pub struct List(Arc<Vec<(Position, Value)>>);

impl List {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &Position) -> Option<&Value> {
        self.0
            .binary_search_by(|(position, _)| position.cmp(key))
            .ok()
            .map(|index| &self.0[index].1)
    }

    pub fn contains_key(&self, key: &Position) -> bool {
        self.0
            .binary_search_by(|(position, _)| position.cmp(key))
            .is_ok()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (Position, Value)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &Position> + ExactSizeIterator {
        self.0.iter().map(|(position, _)| position)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.0.iter().map(|(_, value)| value)
    }

    pub fn insert(&mut self, key: Position, value: Value) -> Option<Value> {
        let elements = Arc::make_mut(&mut self.0);
        match elements.binary_search_by(|(position, _)| position.cmp(&key)) {
            Ok(index) => Some(std::mem::replace(&mut elements[index].1, value)),
            Err(index) => {
                elements.insert(index, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &Position) -> Option<Value> {
        let index = self
            .0
            .binary_search_by(|(position, _)| position.cmp(key))
            .ok()?;
        Some(Arc::make_mut(&mut self.0).remove(index).1)
    }

    pub fn update(&self, key: Position, value: Value) -> Self {
        let mut next = self.clone();
        next.insert(key, value);
        next
    }

    pub fn without(&self, key: &Position) -> Self {
        let mut next = self.clone();
        next.remove(key);
        next
    }
}

impl FromIterator<(Position, Value)> for List {
    fn from_iter<T: IntoIterator<Item = (Position, Value)>>(iter: T) -> Self {
        let mut elements: Vec<_> = iter.into_iter().collect();
        elements.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut unique = Vec::with_capacity(elements.len());
        for (position, value) in elements {
            if let Some((last, stored)) = unique.last_mut()
                && *last == position
            {
                *stored = value;
            } else {
                unique.push((position, value));
            }
        }
        Self(Arc::new(unique))
    }
}

impl IntoIterator for List {
    type Item = (Position, Value);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Arc::try_unwrap(self.0)
            .unwrap_or_else(|shared| (*shared).clone())
            .into_iter()
    }
}

impl<'a> IntoIterator for &'a List {
    type Item = &'a (Position, Value);
    type IntoIter = std::slice::Iter<'a, (Position, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// A value: anything sayable — pure structure, no identity of its
/// own. Cycles are unrepresentable here; they exist only by a cell's
/// value linking back through `Value::Cell`. Positions are session-only
/// element identity — minted at load and insert, stripped at save —
/// and the hand-written Eq/Hash below IGNORE them: two occurrences of
/// `[2, 3]` are the same value.
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
        let elements: Vec<Value> = elements.into_iter().collect();
        Value::List(
            crate::position::spread(elements.len())
                .into_iter()
                .zip(elements)
                .collect(),
        )
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

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Cell(a), Value::Cell(b)) => a == b,
            (Value::Blob(a), Value::Blob(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                Arc::ptr_eq(&a.0, &b.0)
                    || (a.len() == b.len() && a.values().zip(b.values()).all(|(x, y)| x == y))
            }
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
                elements.len().hash(state);
                for element in elements.values() {
                    element.hash(state);
                }
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

pub fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Strict reads: lowercase pairs only, so every blob has exactly one
/// spelled form.
fn hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let digit = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(format!(
            "blob hex must be lowercase hex, got {:?}",
            c as char
        )),
    };
    if !s.len().is_multiple_of(2) {
        return Err("blob hex must have even length".to_string());
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| Ok(digit(pair[0])? << 4 | digit(pair[1])?))
        .collect()
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

/// The source a followed cell resolves through: the document or a loaded
/// library, named by stable identity rather than load order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    Document,
    Library(CellId),
}

/// A projection path step: into a record field, into a list element,
/// or through a link to one of the cell's resolved values. A step that no
/// longer resolves is the stale-path class the editor already
/// tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Key(CellId),
    Element(Position),
    Follow(Resolution),
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
mod tests;
