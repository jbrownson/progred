//! Per-path editor state as data: one UI trie belongs to each workspace
//! view. Each path holds one open record under convention keys, so
//! independent concerns compose at a node without owning it. The runtime
//! uses typed paths; the path library exposes their GID representation.

use gid::{CellId, Step, Value};
use std::collections::HashMap;
use std::rc::Rc;

/// Fold override: one of two named states, absent meaning the default
/// (collapsed inside a cycle). The site library names these.
pub const FOLD: CellId = CellId::from_u128(0x3fa8d15e60b7c2941d8ea05b47f2c6d3);
pub const FOLDED: CellId = CellId::from_u128(0x84c07f3b9ad2561e02c6b4d81f7a39e5);
pub const EXPANDED: CellId = CellId::from_u128(0x1d5b0c47e8f6a923d7405c9128b3fae6);

#[derive(Clone, Default)]
pub struct Annotations {
    values: Rc<HashMap<Vec<Step>, Value>>,
}

impl Annotations {
    /// The whole record at a path — what a projection at that path
    /// will receive.
    pub fn at(&self, path: &[Step]) -> Option<&Value> {
        self.values.get(path)
    }

    /// One convention field at a path.
    #[cfg(test)]
    pub fn field(&self, path: &[Step], key: CellId) -> Option<&Value> {
        self.at(path)?.as_record()?.get(&key)
    }

    /// Replace or clear the whole value at a path. Empty records prune.
    pub fn set(&mut self, path: &[Step], value: Option<Value>) {
        match value {
            Some(Value::Record(fields)) if fields.is_empty() => {
                Rc::make_mut(&mut self.values).remove(path);
            }
            Some(value) => {
                Rc::make_mut(&mut self.values).insert(path.to_vec(), value);
            }
            None => {
                Rc::make_mut(&mut self.values).remove(path);
            }
        }
    }

    /// Set or clear one convention field. Empty node records prune,
    /// so the store stays sparse.
    pub fn set_field(&mut self, path: &[Step], key: CellId, value: Option<Value>) {
        let mut fields = self
            .values
            .get(path)
            .and_then(Value::as_record)
            .cloned()
            .unwrap_or_default();
        match value {
            Some(value) => {
                fields.insert(key, value);
            }
            None => {
                fields.remove(&key);
            }
        }
        self.set(path, (!fields.is_empty()).then_some(Value::Record(fields)));
    }

    pub fn restore_field(&mut self, key: CellId, saved: &Self) {
        let paths: Vec<_> = self
            .values
            .iter()
            .filter(|(_, value)| {
                value
                    .as_record()
                    .is_some_and(|fields| fields.contains_key(&key))
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in paths {
            self.set_field(&path, key, None);
        }
        for (path, value) in saved.values.iter() {
            if let Some(value) = value.as_record().and_then(|fields| fields.get(&key)) {
                self.set_field(path, key, Some(value.clone()));
            }
        }
    }
}

/// Whether a path's `state` says it's folded, or `default` where it
/// says nothing.
pub fn folded(state: Option<&Value>, default: bool) -> bool {
    match state
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&FOLD))
        .and_then(Value::as_cell)
    {
        Some(state) if state == FOLDED => true,
        Some(state) if state == EXPANDED => false,
        _ => default,
    }
}

/// Stays sparse: an override matching the default clears instead of
/// storing.
pub fn set_collapsed(annotations: &mut Annotations, path: &[Step], default: bool, next: bool) {
    let state = (next != default).then_some(Value::Cell(if next { FOLDED } else { EXPANDED }));
    annotations.set_field(path, FOLD, state);
}

#[cfg(test)]
mod tests;
