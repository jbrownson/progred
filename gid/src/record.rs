use crate::{CellId, Value, sorted_entries};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

    pub fn iter(&self) -> std::slice::Iter<'_, (CellId, Value)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &CellId> + ExactSizeIterator {
        self.0.iter().map(|(field, _)| field)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.0.iter().map(|(_, value)| value)
    }

    /// Sets the field under `label`, or with `None` drops it, and returns what
    /// was there.
    pub fn set(&mut self, label: CellId, value: impl Into<Option<Value>>) -> Option<Value> {
        let found = self.0.binary_search_by_key(&label, |(field, _)| *field);
        match (found, value.into()) {
            (Ok(index), Some(value)) => Some(std::mem::replace(
                &mut Arc::make_mut(&mut self.0)[index].1,
                value,
            )),
            (Err(index), Some(value)) => {
                Arc::make_mut(&mut self.0).insert(index, (label, value));
                None
            }
            (Ok(index), None) => Some(Arc::make_mut(&mut self.0).remove(index).1),
            (Err(_), None) => None,
        }
    }

    /// A copy with the field under `label` set, or with `None` dropped.
    pub fn with(&self, label: CellId, value: impl Into<Option<Value>>) -> Self {
        let mut next = self.clone();
        next.set(label, value);
        next
    }

    /// The record spelled by `fields`. A text spelling may list the labels in
    /// any order, but a binary one lists them in increasing order, as they're
    /// written; neither repeats one.
    fn read(mut fields: Vec<(CellId, Value)>, text: bool) -> Result<Self, &'static str> {
        if !fields.is_sorted_by(|(a, _), (b, _)| a < b) {
            if !text {
                return Err("record labels must increase");
            }
            fields.sort_by_key(|(label, _)| *label);
            if fields.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err("repeated label in a record");
            }
        }
        Ok(Self(Arc::new(fields)))
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

/// Spelled as its label-value pairs, in the label order it keeps them in.
impl Serialize for Record {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = deserializer.is_human_readable();
        let fields = Vec::<(CellId, Value)>::deserialize(deserializer)?;
        Record::read(fields, text).map_err(serde::de::Error::custom)
    }
}

/// Fields replace any already under their labels.
impl Extend<(CellId, Value)> for Record {
    fn extend<T: IntoIterator<Item = (CellId, Value)>>(&mut self, fields: T) {
        for (label, value) in fields {
            self.set(label, value);
        }
    }
}

impl FromIterator<(CellId, Value)> for Record {
    fn from_iter<T: IntoIterator<Item = (CellId, Value)>>(iter: T) -> Self {
        Self(Arc::new(sorted_entries(iter)))
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

#[cfg(test)]
#[path = "tests/record.rs"]
mod tests;
