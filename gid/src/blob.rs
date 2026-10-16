use std::{ops::Deref, sync::Arc};

/// Bytes with value semantics: clones share storage, writes detach on demand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Blob(Arc<Vec<u8>>);

impl Blob {
    pub fn make_mut(&mut self) -> &mut Vec<u8> {
        Arc::make_mut(&mut self.0)
    }
}

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

#[cfg(test)]
mod tests;
