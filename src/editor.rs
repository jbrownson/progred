use crate::document::Document;
use crate::graph::{Gid, Id, Selection, SpanningTree};
use crate::ui::graph_view::GraphViewState;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Editor {
    pub doc: Document,
    pub tree: SpanningTree,
    pub selection: Option<Selection>,
    pub file_path: Option<PathBuf>,
    pub graph_view: GraphViewState,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            tree: SpanningTree::empty(),
            selection: None,
            file_path: None,
            graph_view: GraphViewState::new(),
        }
    }

    pub fn selected_node_id(&self) -> Option<Id> {
        match self.selection.as_ref()? {
            Selection::Edge(path, _) => self.doc.node(path),
            Selection::GraphEdge { entity, label } => self.doc.gid.edges(entity).and_then(|e| e.get(label)).cloned(),
            Selection::GraphNode(id) => Some(id.clone()),
            Selection::InsertRoot(..) => None,
        }
    }
}
