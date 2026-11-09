use crate::{hex_string, read_hex};
use std::{fmt, ops::Deref, sync::Arc};

/// Bytes with value semantics: clones share storage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Blob(Arc<Vec<u8>>);

impl From<Vec<u8>> for Blob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(Arc::new(bytes))
    }
}

impl Deref for Blob {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl Blob {
    /// The blob that `0x` and hex digit pairs spell, in either case, as
    /// Display writes it.
    pub fn read(text: &str) -> Option<Self> {
        text.strip_prefix("0x").and_then(read_hex).map(Self::from)
    }
}

impl fmt::Display for Blob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex_string(self))
    }
}

#[cfg(test)]
#[path = "tests/blob.rs"]
mod tests;
