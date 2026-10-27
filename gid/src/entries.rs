use crate::Value;
use std::sync::Arc;

/// Values under sorted, unrepeated keys, kept contiguous for cheap reads and
/// iteration. Clones share their storage, while edits copy only when another
/// value still owns the same entries. Records and lists are entries under
/// cell identities and positions.
#[derive(Debug, Clone)]
pub struct Entries<K>(pub(crate) Arc<Vec<(K, Value)>>);

impl<K> Default for Entries<K> {
    fn default() -> Self {
        Self(Arc::default())
    }
}

impl<K: Ord + Clone> Entries<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &K) -> Option<&Value> {
        self.0
            .binary_search_by(|(at, _)| at.cmp(key))
            .ok()
            .map(|index| &self.0[index].1)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (K, Value)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.0.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.0.iter().map(|(_, value)| value)
    }

    /// Sets the value under `key`, or with `None` drops it, and returns what
    /// was there.
    pub fn set(&mut self, key: K, value: impl Into<Option<Value>>) -> Option<Value> {
        let found = self.0.binary_search_by(|(at, _)| at.cmp(&key));
        match (found, value.into()) {
            (Ok(index), Some(value)) => Some(std::mem::replace(
                &mut Arc::make_mut(&mut self.0)[index].1,
                value,
            )),
            (Err(index), Some(value)) => {
                Arc::make_mut(&mut self.0).insert(index, (key, value));
                None
            }
            (Ok(index), None) => Some(Arc::make_mut(&mut self.0).remove(index).1),
            (Err(_), None) => None,
        }
    }

    /// A copy with the value under `key` set, or with `None` dropped.
    pub fn with(&self, key: K, value: impl Into<Option<Value>>) -> Self {
        let mut next = self.clone();
        next.set(key, value);
        next
    }
}

/// Entries replace any already under their keys.
impl<K: Ord + Clone> Extend<(K, Value)> for Entries<K> {
    fn extend<T: IntoIterator<Item = (K, Value)>>(&mut self, entries: T) {
        for (key, value) in entries {
            self.set(key, value);
        }
    }
}

impl<K: Ord> FromIterator<(K, Value)> for Entries<K> {
    fn from_iter<T: IntoIterator<Item = (K, Value)>>(iter: T) -> Self {
        Self(Arc::new(sorted_entries(iter)))
    }
}

impl<K: Clone> IntoIterator for Entries<K> {
    type Item = (K, Value);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Arc::try_unwrap(self.0)
            .unwrap_or_else(|shared| (*shared).clone())
            .into_iter()
    }
}

impl<'a, K> IntoIterator for &'a Entries<K> {
    type Item = &'a (K, Value);
    type IntoIter = std::slice::Iter<'a, (K, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Entries sorted by key; a repeated key keeps its last value. Entries
/// already in strictly increasing key order stay as they are.
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

#[cfg(test)]
#[path = "tests/entries.rs"]
mod tests;
