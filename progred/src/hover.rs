//! Pointer hover: the tree hover's identity and the value it refers
//! to for secondary marks.

use crate::completion::{EntryAction, completion_entries};
use crate::selection::Selection;
use crate::sources::Sources;
use gid::{CellId, Step, Value};
use std::rc::Rc;

/// What the pointer rests on: the address a later editor action will
/// target. Values preview their selection; toggles and popup entries
/// light their own ink. Placement derives it from the current
/// pointer input and settled geometry; only gap hysteresis needs the
/// prior answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Hover {
    /// Activate here selects the value at this path.
    Value(Rc<[Step]>),
    /// Activate here toggles this path's collapse.
    Toggle(Rc<[Step]>),
    /// A click here opens a pending sibling after the element at
    /// this path — the flat list separator's action.
    Insert(Rc<[Step]>),
    /// A painted Grap operation linked back to the expression that
    /// emitted it.
    Drawing(SourceTrace),
    /// A click here commits the completion entry at this index. An
    /// index, not the entry: a hover stores ADDRESSES, never values,
    /// so what it means re-derives from the LIVE entries each frame —
    /// typing under a parked pointer re-answers instead of marking a
    /// snapshot.
    Entry(usize),
}

/// A structural source location used by execution-linked display.
/// Cell-relative routes survive multiple projections of the same cell;
/// stored routes identify an ordinary document occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceTrace {
    Stored(Rc<[Step]>),
    InCell { cell: CellId, path: Rc<[Step]> },
}

impl SourceTrace {
    pub(crate) fn from_path(sources: &Sources, path: Rc<[Step]>) -> Self {
        path.iter()
            .rposition(|step| matches!(step, Step::Follow(_)))
            .and_then(|follow| {
                sources
                    .resolve_path(&path[..follow])
                    .and_then(Value::as_cell)
                    .map(|cell| Self::InCell {
                        cell,
                        path: Rc::from(&path[follow + 1..]),
                    })
            })
            .unwrap_or(Self::Stored(path))
    }

    pub(crate) fn descendant(&self, steps: &[Step]) -> Self {
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

    pub(crate) fn from_grap(origin: grap::SourceOrigin, input: &Self) -> Self {
        match origin {
            grap::SourceOrigin::Input(path) => input.descendant(&path),
            grap::SourceOrigin::Cell { cell, path } => Self::InCell {
                cell,
                path: path.into(),
            },
        }
    }
}

/// What makes two projected locations secondary copies. Cell values
/// match wherever that cell is referenced. Other values match only
/// at the same path inside the same nearest enclosing cell.
#[derive(Clone, Debug)]
pub(crate) enum Secondary {
    Cell(CellId),
    Stored(Rc<[Step]>),
    InCell {
        cell: CellId,
        path: Rc<[Step]>,
        /// The first step relative to `cell`, immediately after its
        /// `Follow` step in `path`.
        relative_from: usize,
    },
}

impl Secondary {
    pub(crate) fn from_context(
        path: Rc<[Step]>,
        value: &Value,
        enclosing: Option<(CellId, usize)>,
    ) -> Option<Self> {
        match value.as_cell() {
            Some(cell) => Some(Self::Cell(cell)),
            None => Some(match enclosing {
                Some((cell, relative_from)) => Self::InCell {
                    cell,
                    path,
                    relative_from,
                },
                None => Self::Stored(path),
            }),
        }
    }

    pub(crate) fn from_trace(trace: &SourceTrace) -> Self {
        match trace {
            SourceTrace::Stored(path) => Self::Stored(path.clone()),
            SourceTrace::InCell { cell, path } => Self::InCell {
                cell: *cell,
                path: path.clone(),
                relative_from: 0,
            },
        }
    }

    pub(crate) fn from_path(sources: &Sources, path: Rc<[Step]>, value: &Value) -> Option<Self> {
        let enclosing = path
            .iter()
            .rposition(|step| matches!(step, Step::Follow(_)))
            .and_then(|follow| {
                sources
                    .resolve_path(&path[..follow])
                    .and_then(Value::as_cell)
                    .map(|cell| (cell, follow + 1))
            });
        Self::from_context(path, value, enclosing)
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
                    path: left_path,
                    relative_from: left_from,
                },
                Self::InCell {
                    cell: right_cell,
                    path: right_path,
                    relative_from: right_from,
                },
            ) => left_cell == right_cell && left_path[*left_from..] == right_path[*right_from..],
            _ => false,
        }
    }
}

impl Eq for Secondary {}

/// The secondary target a hover refers to. An `Entry` hover
/// re-derives from the live completion offers of the open pending
/// (recomputed here — the price of never marking a snapshot).
pub(crate) fn hover_secondary(
    sources: &Sources,
    raw: bool,
    selection: Option<&Selection>,
    hover: &Hover,
) -> Option<Secondary> {
    match hover {
        Hover::Value(path) => {
            Secondary::from_path(sources, path.clone(), sources.resolve_path(path)?)
        }
        Hover::Drawing(source) => Some(Secondary::from_trace(source)),
        Hover::Entry(index) => {
            let current = selection?;
            let labels = match current.stage() {
                crate::selection::Stage::Pending => false,
                crate::selection::Stage::Label => true,
                crate::selection::Stage::Edge => return None,
            };
            let query = current.edit()?;
            let entries = completion_entries(sources, raw, labels, query.text());
            match &entries.get(*index)?.action {
                EntryAction::Value(value) => value.as_cell().map(Secondary::Cell),
                _ => None,
            }
        }
        Hover::Toggle(_) | Hover::Insert(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, Document, new_cell_id};

    fn secondary(sources: &Sources, path: Vec<Step>) -> Option<Secondary> {
        let path: Rc<[Step]> = Rc::from(path);
        Secondary::from_path(sources, path.clone(), sources.resolve_path(path.as_ref())?)
    }

    #[test]
    fn nested_values_match_by_cell_and_relative_path() {
        let shared = new_cell_id();
        let other = new_cell_id();
        let outer = crate::test_values::label("outer");
        let inner = crate::test_values::label("inner");
        let peer = crate::test_values::label("peer");
        let contents = Value::record([
            (
                outer,
                Value::record([(inner, crate::test_values::text("asdf"))]),
            ),
            (peer, crate::test_values::text("asdf")),
        ]);
        let mut cells = Cells::new();
        cells.set_value(shared, contents.clone());
        cells.set_value(other, contents);
        let root = Value::list([Value::from(shared), Value::from(shared), Value::from(other)]);
        let positions: Vec<_> = root.as_list().expect("root list").keys().cloned().collect();
        let doc = Document {
            root: Some(root),
            cells,
        };
        let libraries = progred_libraries::Libraries::default();
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let nested = |position| {
            vec![
                Step::Element(position),
                Step::Follow(gid::Resolution::Document),
                Step::Key(outer),
                Step::Key(inner),
            ]
        };

        assert_eq!(
            secondary(&sources, nested(positions[0].clone())),
            secondary(&sources, nested(positions[1].clone()))
        );
        assert_ne!(
            secondary(&sources, nested(positions[0].clone())),
            secondary(
                &sources,
                vec![
                    Step::Element(positions[1].clone()),
                    Step::Follow(gid::Resolution::Document),
                    Step::Key(peer),
                ],
            )
        );
        assert_ne!(
            secondary(&sources, nested(positions[0].clone())),
            secondary(&sources, nested(positions[2].clone()))
        );
        assert_eq!(
            secondary(&sources, vec![Step::Element(positions[0].clone())]),
            secondary(&sources, vec![Step::Element(positions[1].clone())])
        );
    }
}
