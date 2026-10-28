//! Pointer hover: the tree hover's identity and the value it refers
//! to for secondary marks.

use crate::display::widget::offers::Offers;
use crate::display::widget::source::{Secondary, SourceTrace};
use crate::occurrence::Trail;

/// What the pointer rests on: the address a later editor action will
/// target. Values preview their selection; toggles and popup entries
/// light their own ink. Placement derives it from the current
/// pointer input and settled geometry; only gap hysteresis needs the
/// prior answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Hover {
    /// Activate here selects the value at this path.
    Value(Trail),
    /// Activate here toggles this path's collapse.
    Toggle(Trail),
    /// The label of the field at this path. Activate here selects the field;
    /// pick picks the label, not the field's value.
    Label(Trail),
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
}

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
            (grap::SourceOrigin::Root(path), SourceTrace::Root(target)) => {
                path.as_slice() == target.as_ref()
            }
            (
                grap::SourceOrigin::Cell { cell, path },
                SourceTrace::InCell {
                    cell: target,
                    path: target_path,
                },
            ) => cell == target && path.as_slice() == target_path.as_ref(),
            (grap::SourceOrigin::Input(suffix), target) => match (&self.input, target) {
                (Some(SourceTrace::Root(base)), SourceTrace::Root(path)) => {
                    base.iter().chain(suffix).eq(path.iter())
                }
                (
                    Some(SourceTrace::InCell { cell, path: base }),
                    SourceTrace::InCell { cell: target, path },
                ) => cell == target && base.iter().chain(suffix).eq(path.iter()),
                _ => false,
            },
            _ => false,
        })
    }
}

/// `trace` as Grap records a source, the inverse of [`from_grap`].
pub(crate) fn to_grap(trace: SourceTrace) -> grap::SourceOrigin {
    match trace {
        SourceTrace::Root(path) => grap::SourceOrigin::Root(path.to_vec()),
        SourceTrace::InCell { cell, path } => grap::SourceOrigin::Cell {
            cell,
            path: path.to_vec(),
        },
    }
}

pub(crate) fn from_grap(
    origin: grap::SourceOrigin,
    input: Option<&SourceTrace>,
) -> Option<SourceTrace> {
    match origin {
        grap::SourceOrigin::Root(path) => Some(SourceTrace::Root(path.into())),
        grap::SourceOrigin::Input(path) => input.map(|input| input.descendant(&path)),
        grap::SourceOrigin::Cell { cell, path } => Some(SourceTrace::InCell {
            cell,
            path: path.into(),
        }),
    }
}

/// The secondary target a hover refers to. A `Value` hover's is what its
/// landmark was drawn sharing, which a computed occurrence has without a
/// stored source; a `Source` hover's is its source, drawn in the area the
/// pointer is in; an `Entry` hover reads the exact completion offers
/// emitted by the current frame.
pub(crate) fn hover_secondary<C>(
    landmark: Option<&crate::display::widget::navigation::Landmark<C>>,
    area: Option<&crate::display::widget::view::Area>,
    completion: Option<&Offers<C>>,
    hover: &Hover,
) -> Option<Secondary> {
    match hover {
        Hover::Value(_) => landmark?.secondary.clone(),
        Hover::Source(source) => Some(Secondary::from_trace(source, &area?.view)),
        Hover::Calls(_) => None, // Needs the available projection; resolved by frame attribution.
        Hover::Entry(index) => completion?.entries.get(*index)?.source.map(Secondary::Cell),
        Hover::Toggle(_) | Hover::Label(_) => None,
    }
}

#[cfg(test)]
#[path = "tests/hover.rs"]
mod tests;
