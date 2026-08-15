//! Pointer hover: the placement claim, air hysteresis, and the value
//! a hover refers to for secondary marks.

use crate::completion::{completion_entries, EntryAction};
use crate::document::Path;
use crate::selection::Selection;
use crate::sources::Sources;
use progred_graph::{Step, Value};
use vello::kurbo::{Point, Rect};

/// Settled placement's internal pointer hit test. Later claims replace
/// earlier ones, matching placement order: descendants and overlays win.
pub trait HasHover<T> {
    fn pointer(&self) -> Option<Point>;
    fn claim_hover(&mut self, claim: T);
}

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

/// One placement report for the current pointer input: what the
/// claim under it means for the hover state.
#[derive(Clone, Debug, PartialEq)]
pub enum HoverClaim {
    /// The pointer names this claim outright; `None` is an occluder
    /// naming nothing.
    Direct(Option<Hovering>),
    /// Unclaimed air, anywhere on the plane — the resolver's backstop
    /// for every pixel no claim took. Within a little gap's reach of
    /// the current hover's footprint it HOLDS — crossing a separator
    /// or the leading between rows never flickers — and beyond that
    /// reach it clears, so open space keeps no distant focus.
    Air,
}

/// What a claim does to the current hover: `Some(next)` replaces it,
/// `None` keeps it. `reach` is the little-gap radius air holds
/// across.
pub fn resolve_hover(
    claim: HoverClaim,
    current: Option<&Hovering>,
    point: Point,
    reach: f64,
) -> Option<Option<Hovering>> {
    match claim {
        HoverClaim::Direct(hovering) => Some(hovering),
        HoverClaim::Air => {
            let held =
                current.is_some_and(|current| current.rect.inflate(reach, reach).contains(point));
            if held { None } else { Some(None) }
        }
    }
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
            .filter(|value| {
                !matches!(value, Value::Record(_)) || progred_text::read(value).is_some()
            })
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
