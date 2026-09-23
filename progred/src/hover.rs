//! Pointer hover: the tree hover's identity and the value it refers
//! to for secondary marks.

use crate::completion::Offers;
use crate::sources::Sources;
#[cfg(test)]
use gid::CellId;
use gid::{Step, Value};
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
    /// A generated widget or drawing linked to its source expression.
    Source(SourceTrace),
    /// A drawing's dynamic call ancestry; the UI chooses the innermost
    /// source that has an available projected occurrence.
    Calls(SourceCalls),
    /// A click here commits the completion entry at this index. An
    /// index, not the entry: a hover stores ADDRESSES, never values,
    /// so what it means re-derives from the LIVE entries each frame —
    /// typing under a parked pointer re-answers instead of marking a
    /// snapshot.
    Entry(usize),
    /// The completion card's one-way widening affordance.
    MoreCompletions,
}

pub use crate::display::widget::source::{Secondary, SourceTrace};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCalls {
    trace: grap::CallTrace,
    input: Option<SourceTrace>,
}

impl SourceCalls {
    pub fn new(trace: grap::CallTrace, input: Option<SourceTrace>) -> Self {
        Self { trace, input }
    }

    pub fn sources(&self) -> impl Iterator<Item = SourceTrace> + '_ {
        self.trace
            .origins()
            .filter_map(|origin| from_grap(origin.clone(), self.input.as_ref()))
    }

    pub fn has_source(&self) -> bool {
        self.trace
            .origins()
            .any(|origin| self.input.is_some() || !matches!(origin, grap::SourceOrigin::Input(_)))
    }

    /// Compare borrowed paths; highlighting must not copy a stack or allocate
    /// source paths for every painted shape.
    pub fn contains(&self, source: &SourceTrace) -> bool {
        self.trace.origins().any(|origin| match (origin, source) {
            (grap::SourceOrigin::Stored(path), SourceTrace::Stored(target)) => {
                path.as_slice() == target.as_ref()
            }
            (
                grap::SourceOrigin::Cell {
                    cell,
                    source: resolution,
                    path,
                },
                SourceTrace::InCell {
                    cell: target,
                    source: target_resolution,
                    path: target_path,
                },
            ) => {
                cell == target
                    && resolution == target_resolution
                    && path.as_slice() == target_path.as_ref()
            }
            (grap::SourceOrigin::Input(suffix), target) => match (&self.input, target) {
                (Some(SourceTrace::Stored(base)), SourceTrace::Stored(path)) => {
                    base.iter().chain(suffix).eq(path.iter())
                }
                (
                    Some(SourceTrace::InCell {
                        cell,
                        source,
                        path: base,
                    }),
                    SourceTrace::InCell {
                        cell: target,
                        source: target_source,
                        path,
                    },
                ) => {
                    cell == target
                        && source == target_source
                        && base.iter().chain(suffix).eq(path.iter())
                }
                _ => false,
            },
            _ => false,
        })
    }
}
impl crate::display::widget::source::PathLookup for Sources<'_> {
    fn value_at(&self, path: &[Step]) -> Option<&Value> {
        self.resolve_path(path)
    }
}
pub(crate) fn from_grap(
    origin: grap::SourceOrigin,
    input: Option<&SourceTrace>,
) -> Option<SourceTrace> {
    match origin {
        grap::SourceOrigin::Stored(path) => Some(SourceTrace::Stored(path.into())),
        grap::SourceOrigin::Input(path) => input.map(|input| input.descendant(&path)),
        grap::SourceOrigin::Cell { cell, source, path } => Some(SourceTrace::InCell {
            cell,
            source,
            path: path.into(),
        }),
    }
}

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
        Hover::Source(source) => Some(Secondary::from_trace(source)),
        Hover::Calls(_) => None, // Needs the available projection; resolved by frame attribution.
        Hover::Entry(index) => completion?.entries.get(*index)?.source.map(Secondary::Cell),
        Hover::Toggle(_) | Hover::MoreCompletions => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, Document, new_cell_id};

    #[test]
    fn detached_drawing_has_no_input_source_but_still_traces_called_cells() {
        let path = vec![Step::Key(new_cell_id())];
        assert_eq!(
            from_grap(grap::SourceOrigin::Input(path.clone()), None),
            None
        );
        let cell = new_cell_id();
        let source = gid::Resolution::Document;
        assert_eq!(
            from_grap(
                grap::SourceOrigin::Cell {
                    cell,
                    source,
                    path: path.clone()
                },
                None
            ),
            Some(SourceTrace::InCell {
                cell,
                source,
                path: path.into()
            })
        );
    }

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
            crate::libraries::Libraries::from_contributions(order.map(|id| {
                (
                    id,
                    crate::libraries::Library::<(), ()>::named(
                        id,
                        "source",
                        crate::libraries::Definitions::from_parts(
                            doc.cells.clone(),
                            grap::ForeignFunctions::default(),
                        ),
                        crate::display::partial(|_| None),
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
                    Some(trace.clone()),
                    crate::hover::from_grap(
                        grap::SourceOrigin::Input(vec![Step::Key(field)]),
                        Some(&input)
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
        let libraries = crate::libraries::Libraries::default();
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
