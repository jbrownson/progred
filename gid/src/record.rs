use crate::{CellId, Entries};
use std::fmt;
use std::hash::{Hash, Hasher};

/// A small immutable map keyed by durable cell identity.
pub type Record = Entries<CellId>;

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

#[cfg(test)]
#[path = "tests/record.rs"]
mod tests;
