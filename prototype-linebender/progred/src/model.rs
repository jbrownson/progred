//! The editor's durable application state: the document, selection,
//! view flags, and the library they are read against.

use crate::document::Document;
use crate::graph_view;
use crate::history;
use crate::selection;
use crate::sources;
use progred_graph::Value;

/// The View menu's frame inputs: which panes and layers this frame
/// shows.
#[derive(Clone, Copy, Default)]
pub(crate) struct ViewFlags {
    pub graph: bool,
    /// The one Raw bit: convention layers derive from it — names
    /// answer bare identities. Lists stay lists; kind is data.
    pub raw: bool,
}

/// The app's one selection: the tree's edge or pending, or the
/// graph's node. A single slot, so selecting in either pane
/// inherently clears the other — there is nothing to synchronize.
// One instance lives in the model; the variants' size gap is moot.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Selected {
    Tree(selection::Selection),
    Graph(graph_view::GraphSelection),
}

pub(crate) struct Model {
    pub doc: Document,
    pub selection: Option<Selected>,
    pub collapse: selection::Collapse,
    /// The built-in library, read under every document; never
    /// written, never saved.
    pub library: progred_graph::Cells,
    /// Rust implementations registered by Grap libraries; editor
    /// configuration rather than document state.
    pub foreign: grap::ForeignFunctions,
    pub graph: graph_view::GraphView,
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

impl Model {
    /// The reading context: this document over the editor's library.
    pub fn sources(&self) -> sources::Sources<'_> {
        sources::Sources {
            doc: &self.doc,
            library: &self.library,
        }
    }

    pub fn tree_selection(&self) -> Option<&selection::Selection> {
        match &self.selection {
            Some(Selected::Tree(selection)) => Some(selection),
            _ => None,
        }
    }

    pub fn tree_selection_mut(&mut self) -> Option<&mut selection::Selection> {
        match &mut self.selection {
            Some(Selected::Tree(selection)) => Some(selection),
            _ => None,
        }
    }

    pub fn graph_selection(&self) -> Option<&graph_view::GraphSelection> {
        match &self.selection {
            Some(Selected::Graph(selection)) => Some(selection),
            _ => None,
        }
    }

    /// The graph-selected node's value, for the tree's secondary
    /// marks. Inline records are structure, not identity, so a
    /// record root's node mirrors no mark.
    pub fn graph_node(&self) -> Option<Value> {
        match self.graph_selection() {
            Some(graph_view::GraphSelection::Node(node)) => graph_view::node_value(&self.doc, node)
                .filter(|value| !matches!(value, Value::Record(_))),
            _ => None,
        }
    }
}
