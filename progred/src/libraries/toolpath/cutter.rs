//! Cutters as document values, and their pictures; the geometry is `cam`'s.
pub use cam::cutter::*;

mod value;
mod view;
pub use value::{ToolValue, vocabulary};
pub(super) use value::{functions, names};
pub(super) use view::{color, display};

#[cfg(test)]
#[path = "tests/cutter/mod.rs"]
mod tests;
