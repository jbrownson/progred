//! Per-path editor state as data: one UI trie belongs to each workspace
//! view. Each path holds one open record under convention keys, so
//! independent concerns compose at a node without owning it. The runtime
//! uses typed paths; the path library exposes their GID representation.

use gid::{CellId, Step, Value};
use std::collections::HashMap;
use std::rc::Rc;

/// Fold override: one of two named states, absent meaning the default
/// (collapsed inside a cycle).
pub use progred_libraries::site::vocabulary::{EXPANDED, FOLD, FOLDED};

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

pub fn collapsed(annotations: &Annotations, path: &[Step], in_cycle: bool) -> bool {
    match annotations.field(path, FOLD).and_then(Value::as_cell) {
        Some(state) if state == FOLDED => true,
        Some(state) if state == EXPANDED => false,
        _ => in_cycle,
    }
}

/// Stays sparse: an override matching the default clears instead of
/// storing.
pub fn set_collapsed(annotations: &mut Annotations, path: &[Step], default: bool, next: bool) {
    let state = (next != default).then(|| Value::Cell(if next { FOLDED } else { EXPANDED }));
    annotations.set_field(path, FOLD, state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_compose_at_a_path_and_prune_when_cleared() {
        let other = CellId::from_u128(7);
        let mut annotations = Annotations::default();
        let path = [Step::Follow(gid::Resolution::Document)];
        set_collapsed(&mut annotations, &path, false, true);
        annotations.set_field(&path, other, Some(Value::from(vec![9u8])));
        assert!(collapsed(&annotations, &path, false));
        assert!(annotations.field(&path, other).is_some());

        set_collapsed(&mut annotations, &path, false, false);
        assert!(!collapsed(&annotations, &path, false));
        assert!(annotations.field(&path, other).is_some());

        annotations.set_field(&path, other, None);
        assert!(annotations.at(&path).is_none());
    }

    #[test]
    fn absent_overrides_fall_to_the_cycle_default() {
        let annotations = Annotations::default();
        assert!(collapsed(
            &annotations,
            &[Step::Follow(gid::Resolution::Document)],
            true
        ));
        assert!(!collapsed(
            &annotations,
            &[Step::Follow(gid::Resolution::Document)],
            false
        ));
    }
}
