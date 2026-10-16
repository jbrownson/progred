//! The GID identity table: each cell id maps directly to its current
//! structural value. A referenced id absent from the table is a bare
//! cell. Cells are the only mutable state; values are persistent, so
//! clones retain structural sharing.

use crate::{CellId, Value};
use imbl::HashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

    /// Makes the cell bare. Removing an already-bare cell is a no-op,
    /// preserving `ptr_eq` as an honest changed-state signal.
    pub fn clear_value(&mut self, cell: CellId) {
        if self.data.contains_key(&cell) {
            self.data = self.data.without(&cell);
        }
    }

    pub fn remove(&mut self, cell: CellId) {
        self.clear_value(cell);
    }

    pub fn cells(&self) -> impl Iterator<Item = &CellId> {
        self.data.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CellId, &Value)> {
        self.data.iter()
    }

    /// Left-biased per cell: existing values are kept and only cells
    /// absent here are added. Not how definitions resolve; see `Host::resolve`.
    pub fn merge(&mut self, other: Cells) {
        for (cell, value) in other.data {
            if !self.data.contains_key(&cell) {
                self.data.insert(cell, value);
            }
        }
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.data.ptr_eq(&other.data)
    }
}

impl Serialize for Cells {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let proxy: std::collections::BTreeMap<CellId, Value> = self
            .data
            .iter()
            .map(|(cell, value)| (*cell, value.clone()))
            .collect();
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Cells {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let proxy = std::collections::HashMap::<CellId, Value>::deserialize(deserializer)?;
        Ok(Cells {
            data: proxy.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests;
