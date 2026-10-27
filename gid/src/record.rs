use crate::{CellId, Value};
use std::fmt;
use std::sync::Arc;

/// A small immutable map keyed by durable cell identity. Records are
/// kept sorted and contiguous for cheap reads and iteration; clones
/// share their storage, while edits copy only when another value still
/// owns the same record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
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

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (index, (label, value)) in self.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{label}: {value}")?;
        }
        write!(f, "}}")
    }
}

/// Fields replace any already under their labels.
impl Extend<(CellId, Value)> for Record {
    fn extend<T: IntoIterator<Item = (CellId, Value)>>(&mut self, fields: T) {
        for (label, value) in fields {
            self.insert(label, value);
        }
    }
}

impl FromIterator<(CellId, Value)> for Record {
    fn from_iter<T: IntoIterator<Item = (CellId, Value)>>(iter: T) -> Self {
        Self(Arc::new(sorted_entries(iter)))
    }
}

/// Entries sorted by key; a repeated key keeps its last value. Entries
/// already in strictly increasing key order stay as they are. Inline, since
/// Grap builds every runtime record through it.
#[inline]
pub fn sorted_entries<K: Ord, V>(entries: impl IntoIterator<Item = (K, V)>) -> Vec<(K, V)> {
    let mut entries: Vec<_> = entries.into_iter().collect();
    if !entries.is_sorted_by(|(a, _), (b, _)| a < b) {
        entries.reverse();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        entries.dedup_by(|(a, _), (b, _)| a == b);
    }
    entries
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

#[cfg(test)]
#[path = "tests/record.rs"]
mod tests;
