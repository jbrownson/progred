//! Per-path editor state as data: each workspace view holds one tree of it
//! along the paths. Each path holds one open record under convention keys, so
//! independent concerns compose at a node without owning it. The runtime
//! uses typed paths; the editor library's path module exposes their GID
//! representation.

use crate::occurrence::Step;
use gid::{CellId, Value};
use std::collections::HashMap;
use std::rc::Rc;

/// Fold override: one of two named states, absent meaning the default
/// (collapsed inside a cycle). The editor library names these.
pub const FOLD: CellId = CellId::from_u128(0x3fa8d15e60b7c2941d8ea05b47f2c6d3);
pub const FOLDED: CellId = CellId::from_u128(0x84c07f3b9ad2561e02c6b4d81f7a39e5);
pub const EXPANDED: CellId = CellId::from_u128(0x1d5b0c47e8f6a923d7405c9128b3fae6);

/// Each path's record, held as a tree along the paths: a projection carries
/// the node for where it draws and reaches a child's by its one step, rather
/// than looking up each occurrence's whole path.
#[derive(Clone, Default)]
pub struct Annotations {
    root: Rc<Node>,
}

/// The record at one path and the nodes below it.
#[derive(Clone, Default)]
pub struct Node {
    value: Option<Value>,
    children: HashMap<Step, Rc<Node>>,
}

impl Node {
    /// The record here.
    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    /// The node `step` below this one, when anything is annotated there or
    /// below it.
    pub fn child(&self, step: &Step) -> Option<&Node> {
        self.children.get(step).map(|child| &**child)
    }

    fn is_empty(&self) -> bool {
        self.value.is_none() && self.children.is_empty()
    }

    /// Every record at or below this node, with its path from here.
    fn entries<'a>(&'a self, path: &mut Vec<Step>, entries: &mut Vec<(Vec<Step>, &'a Value)>) {
        if let Some(value) = &self.value {
            entries.push((path.clone(), value));
        }
        for (step, child) in &self.children {
            path.push(step.clone());
            child.entries(path, entries);
            path.pop();
        }
    }
}

/// Replaces the record at `path` below `node`, pruning nodes left empty.
fn set_in(node: &mut Rc<Node>, path: &[Step], value: Option<Value>) {
    let node = Rc::make_mut(node);
    match path.split_first() {
        None => node.value = value,
        Some((step, rest)) => {
            let child = node.children.entry(step.clone()).or_default();
            set_in(child, rest, value);
            if child.is_empty() {
                node.children.remove(step);
            }
        }
    }
}

impl Annotations {
    /// The node at `path`, when anything is annotated there or below it.
    pub fn node_at(&self, path: &[Step]) -> Option<&Node> {
        path.iter()
            .try_fold(&*self.root, |node, step| node.child(step))
    }

    /// The whole record at a path — what a projection at that path
    /// will receive.
    pub fn at(&self, path: &[Step]) -> Option<&Value> {
        self.node_at(path)?.value()
    }

    /// Replace or clear the whole value at a path. Empty records prune.
    pub fn set(&mut self, path: &[Step], value: Option<Value>) {
        let value =
            value.filter(|value| !matches!(value, Value::Record(fields) if fields.is_empty()));
        if value.is_some() || self.at(path).is_some() {
            set_in(&mut self.root, path, value);
        }
    }

    /// Set or clear one convention field. Empty node records prune,
    /// so the store stays sparse.
    pub fn set_field(&mut self, path: &[Step], key: CellId, value: Option<Value>) {
        let mut fields = self
            .at(path)
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
            .entries()
            .into_iter()
            .filter(|(_, value)| {
                value
                    .as_record()
                    .and_then(|fields| fields.get(&key))
                    .is_some()
            })
            .map(|(path, _)| path)
            .collect();
        for path in paths {
            self.set_field(&path, key, None);
        }
        for (path, value) in saved.entries() {
            if let Some(value) = value.as_record().and_then(|fields| fields.get(&key)) {
                self.set_field(&path, key, Some(value.clone()));
            }
        }
    }

    /// Every record, with its path.
    fn entries(&self) -> Vec<(Vec<Step>, &Value)> {
        let mut entries = Vec::new();
        self.root.entries(&mut Vec::new(), &mut entries);
        entries
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
pub fn set_folded(annotations: &mut Annotations, path: &[Step], default: bool, next: bool) {
    let state = (next != default).then_some(Value::Cell(if next { FOLDED } else { EXPANDED }));
    annotations.set_field(path, FOLD, state);
}

#[cfg(test)]
#[path = "tests/annotations.rs"]
mod tests;
