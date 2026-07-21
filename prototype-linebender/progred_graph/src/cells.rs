//! The identity table: each cell id maps to what is said about that
//! identity — a NAME (presentation of the identity itself, the
//! editor-layer metadata values cannot hold), a current VALUE, or
//! both. The empty entry is unrepresentable by the sum: a cell with
//! neither is simply absent — fully bare, indistinct from never
//! having been minted, which is the honest state. Cells are the only
//! mutable state; values are persistent, so clones are O(1)
//! structural sharing.

use crate::value::{CellId, Value};
use im::HashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Named(String),
    Valued(Value),
    Both(String, Value),
}

impl Cell {
    pub fn name(&self) -> Option<&str> {
        match self {
            Cell::Named(name) | Cell::Both(name, _) => Some(name),
            Cell::Valued(_) => None,
        }
    }

    pub fn value(&self) -> Option<&Value> {
        match self {
            Cell::Valued(value) | Cell::Both(_, value) => Some(value),
            Cell::Named(_) => None,
        }
    }

    fn of(name: Option<String>, value: Option<Value>) -> Option<Cell> {
        match (name, value) {
            (Some(name), Some(value)) => Some(Cell::Both(name, value)),
            (Some(name), None) => Some(Cell::Named(name)),
            (None, Some(value)) => Some(Cell::Valued(value)),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Cells {
    data: HashMap<CellId, Cell>,
}

impl Cells {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entry(&self, cell: CellId) -> Option<&Cell> {
        self.data.get(&cell)
    }

    pub fn name(&self, cell: CellId) -> Option<&str> {
        self.entry(cell)?.name()
    }

    pub fn value(&self, cell: CellId) -> Option<&Value> {
        self.entry(cell)?.value()
    }

    /// Replaces what the entry holds, dropping it when nothing
    /// remains — the no-empty-entry invariant lives here, and so
    /// does its name-level twin: the empty string spells no name and
    /// must be normalized away before storage (`set_name` does; a
    /// future writer that forgets trips the assert). A no-op update
    /// leaves the map untouched, so `ptr_eq` stays an honest
    /// did-anything-change signal.
    fn update(&mut self, cell: CellId, entry: Option<Cell>) {
        assert!(
            entry.as_ref().and_then(|entry| entry.name()) != Some(""),
            "the empty string spells no name; normalize before storing"
        );
        match entry {
            Some(entry) => {
                self.data.insert(cell, entry);
            }
            None => {
                if self.data.contains_key(&cell) {
                    self.data = self.data.without(&cell);
                }
            }
        }
    }

    pub fn set_value(&mut self, cell: CellId, value: Value) {
        let name = self.name(cell).map(str::to_owned);
        self.update(cell, Cell::of(name, Some(value)));
    }

    /// The value goes; the name, if any, stays — an un-valued named
    /// cell is the red link.
    pub fn clear_value(&mut self, cell: CellId) {
        let name = self.name(cell).map(str::to_owned);
        self.update(cell, Cell::of(name, None));
    }

    /// Names the identity, or un-names it — the empty string is the
    /// canonical spelling of namelessness, normalized away here, so
    /// `Named("")` and `Both("", …)` are unrepresentable. The value,
    /// if any, stays.
    pub fn set_name(&mut self, cell: CellId, name: &str) {
        let name = (!name.is_empty()).then(|| name.to_string());
        let value = self.value(cell).cloned();
        self.update(cell, Cell::of(name, value));
    }

    /// Removes everything said about the identity — the graph view's
    /// full detach.
    pub fn remove(&mut self, cell: CellId) {
        self.update(cell, None);
    }

    pub fn cells(&self) -> impl Iterator<Item = &CellId> {
        self.data.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CellId, &Cell)> {
        self.data.iter()
    }

    /// Left-biased per entry: an existing cell's statement wins, new
    /// cells arrive. Libraries compose upstream through this — they
    /// are read-only, so composition is merging.
    pub fn merge(&mut self, other: Cells) {
        for (cell, entry) in other.data {
            if !self.data.contains_key(&cell) {
                self.data.insert(cell, entry);
            }
        }
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.data.ptr_eq(&other.data)
    }
}

/// The file form: `{"name": …, "value": …}` with absent halves
/// omitted; the empty object refuses, as the sum has no state for
/// it, and an empty name refuses too — namelessness is spelled by
/// omission (strict reads: parsable means canonical).
#[derive(Serialize, Deserialize)]
struct CellRepr {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    value: Option<Value>,
}

impl Serialize for Cells {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let proxy: std::collections::BTreeMap<CellId, CellRepr> = self
            .data
            .iter()
            .map(|(cell, entry)| {
                (
                    *cell,
                    CellRepr {
                        name: entry.name().map(str::to_owned),
                        value: entry.value().cloned(),
                    },
                )
            })
            .collect();
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Cells {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let proxy: std::collections::HashMap<CellId, CellRepr> =
            std::collections::HashMap::deserialize(deserializer)?;
        let data = proxy
            .into_iter()
            .map(|(cell, repr)| {
                if repr.name.as_deref() == Some("") {
                    return Err(serde::de::Error::custom(
                        "an empty name is spelled by omission",
                    ));
                }
                Cell::of(repr.name, repr.value)
                    .map(|entry| (cell, entry))
                    .ok_or_else(|| {
                        serde::de::Error::custom("a cell entry needs a name or a value")
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Cells { data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Label, new_cell_id};

    #[test]
    fn names_and_values_are_orthogonal() {
        let mut cells = Cells::new();
        let cell = new_cell_id();
        assert!(cells.entry(cell).is_none());

        // A named bare cell: the red link.
        cells.set_name(cell, "roof");
        assert_eq!(cells.name(cell), Some("roof"));
        assert!(cells.value(cell).is_none());

        // The name survives the value arriving, changing kind, and
        // leaving — it names the identity, not the value.
        cells.set_value(cell, Value::record([(Label::from("x"), Value::from("1"))]));
        assert_eq!(cells.name(cell), Some("roof"));
        cells.set_value(cell, Value::list([Value::from("a")]));
        assert_eq!(cells.name(cell), Some("roof"));
        assert_eq!(cells.value(cell), Some(&Value::list([Value::from("a")])));
        cells.clear_value(cell);
        assert_eq!(cells.name(cell), Some("roof"));

        // The empty string spells no name: un-naming the valueless
        // cell drops the entry whole — the empty entry is
        // unrepresentable — and a valued cell just loses its name.
        cells.set_name(cell, "");
        assert!(cells.entry(cell).is_none());
        cells.set_value(cell, Value::from("v"));
        cells.set_name(cell, "");
        assert_eq!(cells.name(cell), None);
        assert!(cells.value(cell).is_some());
        cells.clear_value(cell);

        // And a value-only cell emptied by clear_value drops too.
        cells.set_value(cell, Value::from("v"));
        cells.clear_value(cell);
        assert!(cells.entry(cell).is_none());
    }

    #[test]
    #[should_panic(expected = "normalize before storing")]
    fn storing_an_empty_name_trips_the_invariant() {
        // Unreachable through the public API — set_name normalizes,
        // the deserializer refuses — so the tripwire is exercised
        // through the private funnel directly.
        let mut cells = Cells::new();
        cells.update(
            new_cell_id(),
            Cell::of(Some(String::new()), Some(Value::from("v"))),
        );
    }

    #[test]
    fn noop_updates_keep_ptr_eq_honest() {
        let mut cells = Cells::new();
        cells.set_value(new_cell_id(), Value::from("x"));
        let before = cells.clone();
        cells.remove(new_cell_id());
        cells.set_name(new_cell_id(), "");
        assert!(cells.ptr_eq(&before));
    }

    #[test]
    fn merge_is_left_biased_per_entry() {
        let shared = new_cell_id();
        let fresh = new_cell_id();
        let mut mine = Cells::new();
        mine.set_value(shared, Value::from("mine"));
        let mut other = Cells::new();
        other.set_name(shared, "theirs");
        other.set_name(fresh, "new");

        mine.merge(other);
        // The whole entry is the statement: no name/value crossing.
        assert_eq!(mine.value(shared), Some(&Value::from("mine")));
        assert_eq!(mine.name(shared), None);
        assert_eq!(mine.name(fresh), Some("new"));
    }

    #[test]
    fn the_table_round_trips_and_refuses_empty_entries() {
        let mut cells = Cells::new();
        let named = new_cell_id();
        let valued = new_cell_id();
        let both = new_cell_id();
        cells.set_name(named, "stroke");
        cells.set_value(valued, Value::from(vec![0x66, 0x33, 0x99]));
        cells.set_name(both, "roof");
        cells.set_value(both, Value::record([(Label::from("k"), Value::from(named))]));

        let json = serde_json::to_string(&cells).unwrap();
        let loaded: Cells = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.entry(named), cells.entry(named));
        assert_eq!(loaded.entry(valued), cells.entry(valued));
        assert_eq!(loaded.entry(both), cells.entry(both));
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
        // A name-only entry spells without a value key at all.
        assert!(json.contains(r#"{"name":"stroke"}"#));

        let empty = format!(r#"{{"{}": {{}}}}"#, new_cell_id());
        assert!(serde_json::from_str::<Cells>(&empty).is_err());
        // An empty name is spelled by omission; the spelled form
        // refuses.
        let blank = format!(r#"{{"{}": {{"name": ""}}}}"#, new_cell_id());
        assert!(serde_json::from_str::<Cells>(&blank).is_err());
    }
}
