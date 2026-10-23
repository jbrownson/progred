//! Paths as ordinary GID data, interpreted in a caller-supplied root
//! and resolution context. List positions are saved with the document, so
//! a path through a list survives a reload.

pub use ::grap::path::{read, read_step, value};

pub mod vocabulary {
    pub use ::grap::path::vocabulary::*;
}

#[cfg(test)]
mod tests;
