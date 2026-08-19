//! Pointer hover: the tree hover's identity, its footprint, and the
//! value a hover refers to for secondary marks.

use crate::completion::{EntryAction, completion_entries};
use crate::selection::Selection;
use crate::sources::Sources;
use gid::{Path, Step, Value};
use progred_libraries::text;
use vello::kurbo::Rect;

/// What the pointer rests on: the claim a plain click at that point
/// would fire. Values preview their selection; labels, toggles, and
/// popup entries light their own ink. Placement derives it from the
/// current pointer input and settled geometry; only gap hysteresis
/// needs the prior answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Hover {
    /// A click here selects the value at this path.
    Value(Path),
    /// A click here re-opens this field's label as its rename.
    Label(Path),
    /// A click here toggles this path's collapse.
    Toggle(Path),
    /// A click here opens a pending sibling after the element at
    /// this path — the flat list separator's click.
    Insert(Path),
    /// A click here commits the completion entry at this index. An
    /// index, not the entry: a hover stores ADDRESSES, never values,
    /// so what it means re-derives from the LIVE entries each frame —
    /// typing under a parked pointer re-answers instead of marking a
    /// snapshot.
    Entry(usize),
}

/// What the pointer rests on plus the footprint it claimed — the
/// identity for drawing, the rect for the little-gap hold.
#[derive(Clone, Debug, PartialEq)]
pub struct Hovering {
    pub hover: Hover,
    pub rect: Rect,
}

/// The value a hover refers to — the hover's `secondary_of`, for
/// marking its other projections. Inline records are structure, not
/// identity: no marks, except for a whole text convention because it
/// projects as one leaf. An `Entry` hover re-derives from the LIVE
/// completion offers of the open pending (recomputed here — the
/// price of never marking a snapshot), so the marks follow the
/// entries as the query is typed.
pub fn hover_value(
    sources: &Sources,
    raw: bool,
    selection: Option<&Selection>,
    hover: &Hover,
) -> Option<Value> {
    match hover {
        Hover::Value(path) => sources
            .resolve(path)
            .filter(|value| !matches!(value, Value::Record(_)) || text::read(value).is_some())
            .cloned(),
        // A dead address answers nothing: the label must still be in
        // the document, or a rename under a parked pointer would keep
        // marking the old spelling's ghost.
        Hover::Label(path) => {
            sources.resolve(path)?;
            match path.last()? {
                Step::Key(key) => Some(Value::Cell(*key)),
                _ => None,
            }
        }
        Hover::Entry(index) => {
            let (query, labels) = match selection? {
                Selection::Pending { query, .. } => (query, false),
                Selection::PendingEdge { query, .. } => (query, true),
                Selection::Edge { .. } => return None,
            };
            let entries = completion_entries(sources, raw, labels, query.text());
            match &entries.get(*index)?.action {
                EntryAction::Value(value) => Some(value.clone()),
                _ => None,
            }
        }
        Hover::Toggle(_) | Hover::Insert(_) => None,
    }
}
