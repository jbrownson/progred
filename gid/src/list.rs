use crate::{Position, Value, sorted_entries};
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
        Self(Arc::new(sorted_entries(iter)))
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
