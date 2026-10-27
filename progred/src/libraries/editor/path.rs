//! Paths as ordinary GID data, interpreted from a caller-supplied root.
//! List positions are saved with the document, so a path through a list
//! survives a reload.

pub use ::grap::path::{read, read_step, value};

pub mod vocabulary {
    pub use ::grap::path::vocabulary::*;
}

#[cfg(test)]
#[path = "tests/path.rs"]
mod tests;
