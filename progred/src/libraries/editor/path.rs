//! Paths as ordinary GID data, interpreted in a caller-supplied root
//! and resolution context. List positions retain their session lifetime.

pub use ::grap::path::{read, read_step, value};

pub mod vocabulary {
    pub use ::grap::path::vocabulary::*;
}

#[cfg(test)]
mod tests;
