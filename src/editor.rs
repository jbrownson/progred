use crate::document::Document;
use crate::graph::{Gid, Id, Path, PlaceholderState, RootSlot, Selection, SpanningTree};
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

pub struct EditorWriter<'a> {
    editor: &'a mut Editor,
}

impl<'a> EditorWriter<'a> {
    pub fn new(editor: &'a mut Editor) -> Self {
        Self { editor }
    }

    pub fn select(&mut self, selection: Option<Selection>) {
        self.editor.selection = selection;
    }

    pub fn set_number_text(&mut self, text: Option<String>) {
        if let Some(Selection::Edge(_, ref mut es)) = self.editor.selection {
            es.number_text = text;
        }
    }

    pub fn set_edge(&mut self, path: &Path, value: Id) {
        self.editor.doc.set_edge(path, value);
    }

    pub fn set_collapsed(&mut self, path: &Path, collapsed: bool) {
        self.editor.tree.set_collapsed(path, collapsed);
    }

    pub fn insert_root(&mut self, index: usize, value: Id) {
        self.editor.doc.roots.insert(index, RootSlot::new(value));
    }

    pub fn set_placeholder_state(&mut self, state: PlaceholderState) {
        match self.editor.selection {
            Some(Selection::Edge(_, ref mut es)) => es.placeholder = state,
            Some(Selection::InsertRoot(_, ref mut ps)) => *ps = state,
            _ => {}
        }
    }

    pub fn set_graph_view(&mut self, state: GraphViewState) {
        self.editor.graph_view = state;
    }

}
