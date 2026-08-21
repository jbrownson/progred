//! The editor's durable document state: document, selection, view,
//! and history.

use crate::annotations;
use crate::history;
use crate::selection;
use gid::Document;

/// The View menu's frame inputs.
#[derive(Clone, Copy, Default)]
pub(crate) struct ViewFlags {
    /// The one Raw bit: convention layers derive from it — names
    /// answer bare identities. Lists stay lists; kind is data.
    pub raw: bool,
}

pub(crate) struct Model {
    pub doc: Document,
    pub selection: Option<selection::Selection>,
    pub annotations: annotations::Annotations,
    pub history: history::History,
    pub view: ViewFlags,
    /// Document scroll offsets in logical pixels, so the position
    /// survives moving between monitor scales. May exceed the
    /// current maximum after a resize: placement clamps effectively,
    /// so a transient shrink-and-grow restores the position;
    /// scrolling collapses it to the clamped reality. Both axes ride
    /// the same gesture; scroll BARS are a later affordance.
    pub scroll: f64,
    pub scroll_x: f64,
}
