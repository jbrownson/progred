//! The GID identity table: each cell id maps directly to its current
//! structural value. A referenced id absent from the table is a bare
//! cell. Cells are the only mutable state; values are persistent, so
//! clones retain structural sharing.

use crate::{CellId, Value};
use imbl::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cells {
    data: HashMap<CellId, Value>,
}

impl Cells {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self, cell: CellId) -> Option<&Value> {
        self.data.get(&cell)
    }

    pub fn set_value(&mut self, cell: CellId, value: Value) {
        self.data.insert(cell, value);
    }

    /// Makes the cell bare.
    pub fn clear_value(&mut self, cell: CellId) {
        self.data = self.data.without(&cell);
    }

    pub fn cells(&self) -> impl Iterator<Item = &CellId> {
        self.data.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CellId, &Value)> {
        self.data.iter()
    }
}

#[cfg(test)]
#[path = "tests/cells.rs"]
mod tests;
