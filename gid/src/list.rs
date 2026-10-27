use crate::{Entries, Position, Value};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// An ordered sequence whose positions, saved with the document, give its
/// elements stable identities. Positions affect navigation but not value
/// equality.
pub type List = Entries<Position>;

/// Bare values get evenly spread positions, in order.
impl FromIterator<Value> for List {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        let elements: Vec<Value> = iter.into_iter().collect();
        crate::position::spread(elements.len())
            .into_iter()
            .zip(elements)
            .collect()
    }
}

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.len() == other.len() && self.values().zip(other.values()).all(|(x, y)| x == y))
    }
}

impl Eq for List {}

impl Hash for List {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for element in self.values() {
            element.hash(state);
        }
    }
}

impl fmt::Display for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (index, element) in self.values().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            element.fmt(f)?;
        }
        write!(f, "]")
    }
}

#[cfg(test)]
#[path = "tests/list.rs"]
mod tests;
