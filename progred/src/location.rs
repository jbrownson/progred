//! Where stored data lives. A location starts at the document's root or at a
//! cell's definition and descends by ordinary steps from there. Following a
//! cell starts a new location at its definition, however the cell was reached,
//! so whether something can be edited depends on where it's stored, not on
//! the route to it. A computed value isn't stored, so it has no location of
//! its own, but a cell inside one still starts one.

use crate::sources::Sources;
use gid::{CellId, Resolution, Step};
use std::borrow::Cow;

/// Where a location starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// The document's root value.
    Root,
    /// The definition `resolution` gives `cell`.
    Cell {
        cell: CellId,
        resolution: Resolution,
    },
}

/// A start and the steps from it. The steps can still follow cells, as a
/// route a projection's conject names may; [`Location::stored`] restarts the
/// location at the last cell it follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    pub start: Start,
    pub path: Cow<'a, [Step]>,
}

impl<'a> Location<'a> {
    pub fn root(path: impl Into<Cow<'a, [Step]>>) -> Self {
        Self {
            start: Start::Root,
            path: path.into(),
        }
    }

    pub fn in_cell(cell: CellId, resolution: Resolution, path: impl Into<Cow<'a, [Step]>>) -> Self {
        Self {
            start: Start::Cell { cell, resolution },
            path: path.into(),
        }
    }

    pub fn into_owned(self) -> Location<'static> {
        Location {
            start: self.start,
            path: Cow::Owned(self.path.into_owned()),
        }
    }

    /// The location `steps` below this one.
    pub fn descendant(&self, steps: &[Step]) -> Location<'static> {
        Location {
            start: self.start,
            path: self.path.iter().chain(steps).cloned().collect(),
        }
    }

    /// The same location from where its data is stored: the definition its
    /// last Follow enters, or else its own start. None when the route to that
    /// Follow no longer reaches a cell.
    pub fn stored(&self, sources: &Sources) -> Option<Location<'_>> {
        match last_follow(&self.path) {
            Some((before, source, after)) => sources
                .read_from(self.start, before)?
                .as_cell()
                .map(|cell| Location::in_cell(cell, source, after)),
            None => Some(Location {
                start: self.start,
                path: Cow::Borrowed(&self.path),
            }),
        }
    }
}

/// Whether writes can land where `start` stores its data: the root, or one of
/// the document's own definitions. A library's definitions are read-only.
pub fn owned(start: Start) -> bool {
    matches!(
        start,
        Start::Root
            | Start::Cell {
                resolution: Resolution::Document,
                ..
            }
    )
}

/// Whether a write at `location` can land: where it's stored must be the
/// document's own.
pub fn writable(sources: &Sources, location: &Location) -> bool {
    location
        .stored(sources)
        .is_some_and(|stored| owned(stored.start))
}

/// The path's last Follow, between the steps before and after it.
fn last_follow(path: &[Step]) -> Option<(&[Step], Resolution, &[Step])> {
    let (before, rest) = path.split_at(
        path.iter()
            .rposition(|step| matches!(step, Step::Follow(_)))?,
    );
    match rest.split_first()? {
        (Step::Follow(source), after) => Some((before, *source, after)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
