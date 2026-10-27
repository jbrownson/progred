use crate::{Position, Value, sorted_entries};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// An ordered sequence whose positions, saved with the document, give
/// its elements stable identities. Like records, lists are contiguous and
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

    pub fn iter(&self) -> std::slice::Iter<'_, (Position, Value)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &Position> + ExactSizeIterator {
        self.0.iter().map(|(position, _)| position)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.0.iter().map(|(_, value)| value)
    }

    /// Sets the element at `position`, or with `None` drops it, and returns
    /// what was there.
    pub fn set(&mut self, position: &Position, value: impl Into<Option<Value>>) -> Option<Value> {
        let found = self.0.binary_search_by(|(at, _)| at.cmp(position));
        match (found, value.into()) {
            (Ok(index), Some(value)) => Some(std::mem::replace(
                &mut Arc::make_mut(&mut self.0)[index].1,
                value,
            )),
            (Err(index), Some(value)) => {
                Arc::make_mut(&mut self.0).insert(index, (position.clone(), value));
                None
            }
            (Ok(index), None) => Some(Arc::make_mut(&mut self.0).remove(index).1),
            (Err(_), None) => None,
        }
    }

    /// A copy with the element at `position` set, or with `None` dropped.
    pub fn with(&self, position: &Position, value: impl Into<Option<Value>>) -> Self {
        let mut next = self.clone();
        next.set(position, value);
        next
    }
}

impl FromIterator<(Position, Value)> for List {
    fn from_iter<T: IntoIterator<Item = (Position, Value)>>(iter: T) -> Self {
        Self(Arc::new(sorted_entries(iter)))
    }
}

/// Bare values get evenly spread positions, in order.
impl FromIterator<Value> for List {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        let elements: Vec<Value> = iter.into_iter().collect();
        crate::position::spread(elements.len())
            .into_iter()
            .zip(elements)
            .collect()
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

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.len() == other.len() && self.values().zip(other.values()).all(|(x, y)| x == y))
    }
}

impl Eq for List {}

impl fmt::Display for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (index, element) in self.values().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            element.fmt(f)?;
        }
        write!(f, "]")
    }
}

/// Spelled as its values in order. Positions aren't spelled: reading mints
/// fresh ones in the same order.
impl Serialize for List {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.values())
    }
}

impl<'de> Deserialize<'de> for List {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Vec::<Value>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

impl Hash for List {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for element in self.values() {
            element.hash(state);
        }
    }
}

#[cfg(test)]
#[path = "tests/list.rs"]
mod tests;
