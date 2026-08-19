//! Per-path editor state as data: the UI trie. Each path holds one
//! open record under convention keys, so independent concerns compose
//! at a node without owning it. Paths stay Rust addresses — only the
//! VALUES are data; the customization points speak GID, the editor is
//! not bootstrapped into its own graph.

use gid::{CellId, Step, Value};
use std::collections::HashMap;

/// The fold override at a path: one of two NAMED STATES, absent
/// meaning the default (collapsed inside a cycle). Named cells, not a
/// boolean — GID deliberately has no bool; presence covers toggles
/// and named states cover the rest.
pub const FOLD: CellId = CellId::from_u128(0x3fa8d15e60b7c2941d8ea05b47f2c6d3);
pub const FOLDED: CellId = CellId::from_u128(0x84c07f3b9ad2561e02c6b4d81f7a39e5);
pub const EXPANDED: CellId = CellId::from_u128(0x1d5b0c47e8f6a923d7405c9128b3fae6);

#[derive(Default)]
pub struct Annotations {
    values: HashMap<Vec<Step>, Value>,
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
        if fields.is_empty() {
            self.values.remove(path);
        } else {
            self.values.insert(path.to_vec(), Value::Record(fields));
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
    let state =
        (next != default).then(|| Value::Cell(if next { FOLDED } else { EXPANDED }));
    annotations.set_field(path, FOLD, state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_compose_at_a_path_and_prune_when_cleared() {
        let other = CellId::from_u128(7);
        let mut annotations = Annotations::default();
        let path = [Step::Follow];
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
        assert!(collapsed(&annotations, &[Step::Follow], true));
        assert!(!collapsed(&annotations, &[Step::Follow], false));
    }
}
