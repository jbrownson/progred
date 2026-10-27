use crate::location::{Location, Start};
use crate::sources::Sources;
use crate::workspace::View;
use gid::{CellId, Step};
use std::rc::Rc;

/// A structural source location used by execution-linked display.
/// Definition-relative routes survive multiple projections of the same definition;
/// stored routes identify an ordinary document occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceTrace {
    Stored(Rc<[Step]>),
    InCell { cell: CellId, path: Rc<[Step]> },
}

impl SourceTrace {
    /// Where a location is stored, however it was named. One whose route no
    /// longer reaches a cell keeps its own spelling.
    pub fn from_location(sources: &Sources, location: &Location) -> Self {
        let stored = location.stored(sources);
        let location = stored.as_ref().unwrap_or(location);
        let path = Rc::from(location.path.as_ref());
        match location.start {
            Start::Root => Self::Stored(path),
            Start::Cell(cell) => Self::InCell { cell, path },
        }
    }

    pub fn descendant(&self, steps: &[Step]) -> Self {
        let append = |path: &[Step]| {
            path.iter()
                .cloned()
                .chain(steps.iter().cloned())
                .collect::<Rc<[Step]>>()
        };
        match self {
            Self::Stored(path) => Self::Stored(append(path)),
            Self::InCell { cell, path } => Self::InCell {
                cell: *cell,
                path: append(path),
            },
        }
    }
}

/// What makes two projected locations secondary copies. Cell values
/// match wherever that cell is referenced. Other values match only
/// at the same path inside the same nearest enclosing definition, whichever
/// views draw them.
#[derive(Clone, Debug)]
pub enum Secondary {
    Cell(CellId),
    Stored(crate::occurrence::Trail),
    InCell {
        cell: CellId,
        path: crate::occurrence::Trail,
        /// The first step relative to `cell`, immediately after its
        /// `Follow` step in `path`.
        relative_from: usize,
    },
}

impl Secondary {
    /// What an occurrence at `path`, whose data steps from the root, or from
    /// the `enclosing` cell's start, are its location's, shares.
    pub fn from_context(
        path: crate::occurrence::Trail,
        cell: Option<CellId>,
        enclosing: Option<(CellId, usize)>,
    ) -> Self {
        match cell {
            Some(cell) => Self::Cell(cell),
            None => match enclosing {
                Some((cell, relative_from)) => Self::InCell {
                    cell,
                    path,
                    relative_from,
                },
                None => Self::Stored(path),
            },
        }
    }

    /// What `trace`'s data shares, drawn in `view`.
    pub fn from_trace(trace: &SourceTrace, view: &Rc<View>) -> Self {
        let trail = |path: &[Step]| {
            crate::occurrence::Trail::in_view(view.clone(), &crate::occurrence::of(path))
        };
        match trace {
            SourceTrace::Stored(path) => Self::Stored(trail(path)),
            SourceTrace::InCell { cell, path } => Self::InCell {
                cell: *cell,
                path: trail(path),
                relative_from: 0,
            },
        }
    }

    /// What a value other than a reference shares, drawn in `view`: where
    /// it's stored.
    pub fn from_location(sources: &Sources, location: &Location, view: &Rc<View>) -> Self {
        Self::from_trace(&SourceTrace::from_location(sources, location), view)
    }
}

impl PartialEq for Secondary {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Cell(left), Self::Cell(right)) => left == right,
            (Self::Stored(left), Self::Stored(right)) => left.same_after(0, right, 0),
            (
                Self::InCell {
                    cell: left_cell,
                    path: left_path,
                    relative_from: left_from,
                },
                Self::InCell {
                    cell: right_cell,
                    path: right_path,
                    relative_from: right_from,
                },
            ) => {
                left_cell == right_cell && left_path.same_after(*left_from, right_path, *right_from)
            }
            _ => false,
        }
    }
}

impl Eq for Secondary {}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
