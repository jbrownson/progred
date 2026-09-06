//! Pointer hover: the tree hover's identity and the value it refers
//! to for secondary marks.

use crate::completion::Offers;
use crate::sources::Sources;
use gid::{CellId, Resolution, Step, Value};
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
    /// The completion card's one-way widening affordance.
    MoreCompletions,
}

/// A structural source location used by execution-linked display.
/// Definition-relative routes survive multiple projections of the same definition;
/// stored routes identify an ordinary document occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceTrace {
    Stored(Rc<[Step]>),
    InCell {
        cell: CellId,
        source: Resolution,
        path: Rc<[Step]>,
    },
}

fn enclosing_definition(sources: &Sources, path: &[Step]) -> Option<(CellId, Resolution, usize)> {
    path.iter()
        .enumerate()
        .rev()
        .find_map(|(index, step)| match step {
            Step::Follow(source) => Some((index, *source)),
            _ => None,
        })
        .and_then(|(index, source)| {
            sources
                .resolve_path(&path[..index])
                .and_then(Value::as_cell)
                .map(|cell| (cell, source, index + 1))
        })
}

impl SourceTrace {
    pub(crate) fn from_path(sources: &Sources, path: Rc<[Step]>) -> Self {
        enclosing_definition(sources, &path)
            .map(|(cell, source, relative_from)| Self::InCell {
                cell,
                source,
                path: Rc::from(&path[relative_from..]),
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
            Self::InCell { cell, source, path } => Self::InCell {
                cell: *cell,
                source: *source,
                path: append(path),
            },
        }
    }

    pub(crate) fn from_grap(origin: grap::SourceOrigin, input: &Self) -> Self {
        match origin {
            grap::SourceOrigin::Input(path) => input.descendant(&path),
            grap::SourceOrigin::Cell { cell, source, path } => Self::InCell {
                cell,
                source,
                path: path.into(),
            },
        }
    }
}

/// What makes two projected locations secondary copies. Cell values
/// match wherever that cell is referenced. Other values match only
/// at the same path inside the same nearest enclosing definition.
#[derive(Clone, Debug)]
pub(crate) enum Secondary {
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
    pub(crate) fn from_context(
        path: Rc<[Step]>,
        value: &Value,
        enclosing: Option<(CellId, Resolution, usize)>,
    ) -> Self {
        match value.as_cell() {
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

    pub(crate) fn from_trace(trace: &SourceTrace) -> Self {
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

    pub(crate) fn from_path(sources: &Sources, path: Rc<[Step]>, value: &Value) -> Self {
        let enclosing = enclosing_definition(sources, &path);
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

/// The secondary target a hover refers to. An `Entry` hover reads the
/// exact completion offers emitted by the current frame.
pub(crate) fn hover_secondary<C>(
    sources: &Sources,
    completion: Option<&Offers<C>>,
    hover: &Hover,
) -> Option<Secondary> {
    match hover {
        Hover::Value(path) => sources
            .resolve_path(path)
            .map(|value| Secondary::from_path(sources, path.clone(), value)),
        Hover::Drawing(source) => Some(Secondary::from_trace(source)),
        Hover::Entry(index) => completion?.entries.get(*index)?.source.map(Secondary::Cell),
        Hover::Toggle(_) | Hover::Insert(_) | Hover::MoreCompletions => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, Document, new_cell_id};

    fn secondary(sources: &Sources, path: Vec<Step>) -> Option<Secondary> {
        let path: Rc<[Step]> = Rc::from(path);
        sources
            .resolve_path(&path)
            .map(|value| Secondary::from_path(sources, path.clone(), value))
    }

    #[test]
    fn definitions_have_distinct_source_traces_and_secondary_marks() {
        let cell = new_cell_id();
        let field = new_cell_id();
        let library_ids = [new_cell_id(), new_cell_id()];
        let mut cells = Cells::new();
        cells.set_value(cell, Value::record([(field, Value::from(vec![1]))]));
        let root = Value::list([Value::from(cell), Value::from(cell)]);
        let positions: Vec<_> = root.as_list().unwrap().keys().cloned().collect();
        let doc = Document {
            root: Some(root),
            cells,
        };
        let libraries = |order: [CellId; 2]| {
            progred_libraries::Libraries::from_contributions(order.map(|id| {
                (
                    id,
                    progred_libraries::Library::<(), ()>::named(
                        id,
                        "source",
                        progred_libraries::Definitions::from_parts(
                            doc.cells.clone(),
                            grap::ForeignFunctions::default(),
                        ),
                        progred_display::partial(|_| None),
                    ),
                )
            }))
            .0
        };
        let path = |position: usize, resolution| {
            vec![
                Step::Element(positions[position].clone()),
                Step::Follow(resolution),
                Step::Key(field),
            ]
        };
        let definitions = [
            gid::Resolution::Document,
            gid::Resolution::Library(library_ids[0]),
            gid::Resolution::Library(library_ids[1]),
        ];
        let first = libraries(library_ids);
        let reordered = libraries([library_ids[1], library_ids[0]]);
        let original = Sources {
            doc: &doc,
            libraries: &first,
        };
        for libraries in [&first, &reordered] {
            let sources = Sources {
                doc: &doc,
                libraries,
            };
            for left in definitions {
                let left_path = path(0, left);
                let trace = SourceTrace::from_path(&sources, left_path.clone().into());
                let input = SourceTrace::from_path(&sources, Rc::from(&left_path[..2]));
                assert_eq!(
                    trace,
                    SourceTrace::from_grap(
                        grap::SourceOrigin::Input(vec![Step::Key(field)]),
                        &input
                    )
                );
                assert_eq!(
                    Some(Secondary::from_trace(&trace)),
                    secondary(&sources, left_path.clone())
                );
                assert_eq!(
                    trace,
                    SourceTrace::from_path(&original, left_path.clone().into())
                );
                for right in definitions {
                    let right_path = path(1, right);
                    assert_eq!(
                        sources.resolve_path(&left_path),
                        sources.resolve_path(&right_path)
                    );
                    assert_eq!(
                        trace == SourceTrace::from_path(&sources, right_path.clone().into()),
                        left == right,
                    );
                    assert_eq!(
                        secondary(&sources, left_path.clone()) == secondary(&sources, right_path),
                        left == right,
                    );
                }
            }
            assert_eq!(
                secondary(&sources, vec![Step::Element(positions[0].clone())]),
                secondary(&sources, vec![Step::Element(positions[1].clone())]),
            );
        }
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
