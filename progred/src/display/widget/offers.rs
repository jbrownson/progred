use crate::display::Face;
use gid::CellId;
use std::{ops::Range, rc::Rc};

pub struct Entry<C> {
    pub display: String,
    pub detail: Option<String>,
    pub matches: Vec<Range<usize>>,
    pub face: Face,
    pub source: Option<CellId>,
    pub activate: Rc<dyn Fn(&mut C)>,
}

impl<C> Clone for Entry<C> {
    fn clone(&self) -> Self {
        Self {
            display: self.display.clone(),
            detail: self.detail.clone(),
            matches: self.matches.clone(),
            face: self.face,
            source: self.source,
            activate: self.activate.clone(),
        }
    }
}

/// Retained in the placed frame for attribution to the exact visible offers.
pub struct Offers<C> {
    pub entries: Vec<Entry<C>>,
}
