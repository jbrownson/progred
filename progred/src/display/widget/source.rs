use crate::location::{Location, Start};
use crate::sources::Sources;
use gid::{CellId, Resolution, Step};
use std::rc::Rc;

/// A structural source location used by execution-linked display.
/// Definition-relative routes survive multiple projections of the same definition;
/// stored routes identify an ordinary document occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceTrace {
    Stored(Rc<[Step]>),
    InCell {
        cell: CellId,
        source: Resolution,
        path: Rc<[Step]>,
    },
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
            Start::Cell { cell, source } => Self::InCell { cell, source, path },
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
            Self::InCell { cell, source, path } => Self::InCell {
                cell: *cell,
                source: *source,
                path: append(path),
            },
        }
    }
}

/// What makes two projected locations secondary copies. Cell values
/// match wherever that cell is referenced. Other values match only
/// at the same path inside the same nearest enclosing definition.
#[derive(Clone, Debug)]
pub enum Secondary {
    Cell(CellId),
    Stored(Rc<[Step]>),
    InCell {
        cell: CellId,
        source: Resolution,
        path: Rc<[Step]>,
        /// The first step relative to `cell`, immediately after its
        /// `Follow` step in `path`.
        relative_from: usize,
    },
}

impl Secondary {
    pub fn from_context(
        path: Rc<[Step]>,
        cell: Option<CellId>,
        enclosing: Option<(CellId, Resolution, usize)>,
    ) -> Self {
        match cell {
            Some(cell) => Self::Cell(cell),
            None => match enclosing {
                Some((cell, source, relative_from)) => Self::InCell {
                    cell,
                    source,
                    path,
                    relative_from,
                },
                None => Self::Stored(path),
            },
        }
    }

    pub fn from_trace(trace: &SourceTrace) -> Self {
        match trace {
            SourceTrace::Stored(path) => Self::Stored(path.clone()),
            SourceTrace::InCell { cell, source, path } => Self::InCell {
                cell: *cell,
                source: *source,
                path: path.clone(),
                relative_from: 0,
            },
        }
    }

    /// What a value other than a reference shares: where it's stored.
    pub fn from_location(sources: &Sources, location: &Location) -> Self {
        Self::from_trace(&SourceTrace::from_location(sources, location))
    }
}

impl PartialEq for Secondary {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Cell(left), Self::Cell(right)) => left == right,
            (Self::Stored(left), Self::Stored(right)) => left == right,
            (
                Self::InCell {
                    cell: left_cell,
                    source: left_source,
                    path: left_path,
                    relative_from: left_from,
                },
                Self::InCell {
                    cell: right_cell,
                    source: right_source,
                    path: right_path,
                    relative_from: right_from,
                },
            ) => {
                left_cell == right_cell
                    && left_source == right_source
                    && left_path[*left_from..] == right_path[*right_from..]
            }
            _ => false,
        }
    }
}

impl Eq for Secondary {}
