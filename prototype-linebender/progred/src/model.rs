//! The editor's durable document state: document, selection, view,
//! and history.

use crate::history;
use crate::selection;
use crate::workspace;
use gid::Document;

/// The View menu's frame inputs.
#[derive(Clone, Copy, Default)]
pub(crate) struct ViewFlags {
    /// Overlay leaf rectangles and the pointer hysteresis geometry.
    pub debug_geometry: bool,
}

pub(crate) struct Model {
    pub doc: Document,
    pub selection: Option<selection::Selection>,
    pub history: history::History,
    pub view: ViewFlags,
    /// Session-owned views over the document. Their layout, roots,
    /// projections, and scroll positions are not GID.
    pub workspace: workspace::Workspace,
}
