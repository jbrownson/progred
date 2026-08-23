//! Pointer hover: the tree hover's identity and the value it refers
//! to for secondary marks.

use crate::completion::{EntryAction, completion_entries};
use crate::selection::Selection;
use crate::sources::Sources;
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
    /// A click here opens a pending sibling after the element at
    /// this path — the flat list separator's action.
    Insert(Rc<[Step]>),
    /// A click here commits the completion entry at this index. An
    /// index, not the entry: a hover stores ADDRESSES, never values,
    /// so what it means re-derives from the LIVE entries each frame —
    /// typing under a parked pointer re-answers instead of marking a
    /// snapshot.
    Entry(usize),
}

/// The cell a hover refers to — the hover's `secondary_of`, for
/// marking its other projections. Marks mean IDENTITY: the same
/// cell, shared — never a copy that happens to be equal, so only
/// cell values answer. An `Entry` hover re-derives from the LIVE
/// completion offers of the open pending (recomputed here — the
/// price of never marking a snapshot).
pub fn hover_value(
    sources: &Sources,
    raw: bool,
    selection: Option<&Selection>,
    hover: &Hover,
) -> Option<Value> {
    match hover {
        Hover::Value(path) => sources
            .resolve(path.as_ref())
            .filter(|value| value.as_cell().is_some())
            .cloned(),
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
                EntryAction::Value(value) if value.as_cell().is_some() => Some(value.clone()),
                _ => None,
            }
        }
        Hover::Toggle(_) | Hover::Insert(_) => None,
    }
}
