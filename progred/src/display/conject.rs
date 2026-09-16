//! Interpret a projection-relative address at a document location.
//!
//! A conject is not an inverse projection: several occurrences may resolve to
//! the same source, and a supplied/computed value may have no source at all.

use gid::{Path, Step};
use std::rc::Rc;

#[derive(Clone)]
pub struct Conject(Rc<dyn Fn(&[Step], &[Step]) -> Option<Path>>);

impl Conject {
    pub fn new(f: impl Fn(&[Step], &[Step]) -> Option<Path> + 'static) -> Self {
        Self(Rc::new(f))
    }

    pub fn apply(&self, projection_path: &[Step], document_path: &[Step]) -> Option<Path> {
        (self.0)(projection_path, document_path)
    }

    pub fn descend() -> Self {
        Self::new(|projection, document| Some(document.iter().chain(projection).cloned().collect()))
    }

    pub fn detached() -> Self {
        Self::new(|_, _| None)
    }
}

impl std::fmt::Debug for Conject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Conject(..)")
    }
}
