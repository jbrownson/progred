use crate::{CellId, Entries, Value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A small immutable map keyed by durable cell identity.
pub type Record = Entries<CellId>;

impl Record {
    /// The record spelled by `fields`. A text spelling may list the labels in
    /// any order, but a binary one lists them in increasing order, as they're
    /// written; neither repeats one.
    fn read(mut fields: Vec<(CellId, Value)>, text: bool) -> Result<Self, &'static str> {
        if !fields.is_sorted_by(|(a, _), (b, _)| a < b) {
            if !text {
                return Err("record labels must increase");
            }
            fields.sort_by_key(|(label, _)| *label);
            if fields.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err("repeated label in a record");
            }
        }
        Ok(Self(Arc::new(fields)))
    }
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Record {}

impl Hash for Record {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (index, (label, value)) in self.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{label}: {value}")?;
        }
        write!(f, "}}")
    }
}

/// Spelled as its label-value pairs, in the label order it keeps them in.
impl Serialize for Record {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = deserializer.is_human_readable();
        let fields = Vec::<(CellId, Value)>::deserialize(deserializer)?;
        Record::read(fields, text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "tests/record.rs"]
mod tests;
