use crate::document::Document;
use crate::graph::{EdgeState, Gid, Id, MutGid, Path, PlaceholderState, RootSlot, Selection, SpanningTree};
use crate::ui::graph_view::GraphViewState;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Editor {
    pub doc: Document,
    pub tree: SpanningTree,
    pub selection: Option<Selection>,
    pub file_path: Option<PathBuf>,
    pub graph_view: GraphViewState,
    pub(crate) cached_orphans: Option<(MutGid, Vec<RootSlot>, HashSet<Id>)>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            tree: SpanningTree::empty(),
            selection: None,
            file_path: None,
            graph_view: GraphViewState::new(),
            cached_orphans: None,
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

    pub fn placeholder_visible(&self) -> bool {
        match self.selection.as_ref() {
            Some(Selection::InsertRoot(..)) => true,
            Some(Selection::Edge(path, EdgeState::Cursor(_))) => self.doc.node(path).is_none(),
            _ => false,
        }
    }

    pub fn is_editing_leaf(&self) -> bool {
        matches!(self.selection, Some(Selection::Edge(_, EdgeState::EditingLeaf(_))))
    }

    pub fn orphan_roots(&self) -> &HashSet<Id> {
        static EMPTY: std::sync::LazyLock<HashSet<Id>> = std::sync::LazyLock::new(HashSet::new);
        match &self.cached_orphans {
            Some((gid, roots, orphans)) if self.doc.gid.ptr_eq(gid) && &self.doc.roots == roots => orphans,
            _ => &EMPTY,
        }
    }

    pub fn refresh_orphan_cache(&mut self) {
        if !self.cached_orphans.as_ref()
            .is_some_and(|(gid, roots, _)| self.doc.gid.ptr_eq(gid) && &self.doc.roots == roots)
        {
            self.cached_orphans = Some((self.doc.gid.clone(), self.doc.roots.clone(), self.doc.orphan_roots()));
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
            Some(Selection::Edge(_, EdgeState::Cursor(ref mut ps))) => *ps = state,
            Some(Selection::InsertRoot(_, ref mut ps)) => *ps = state,
            _ => {}
        }
    }

    pub fn set_graph_view(&mut self, state: GraphViewState) {
        self.editor.graph_view = state;
    }

    pub fn start_leaf_edit(&mut self, text: String) {
        if let Some(Selection::Edge(_, ref mut edge_state)) = self.editor.selection {
            *edge_state = EdgeState::EditingLeaf(text);
        }
    }

    pub fn stop_leaf_edit(&mut self) -> Option<String> {
        if let Some(Selection::Edge(_, ref mut edge_state)) = self.editor.selection {
            if let EdgeState::EditingLeaf(text) = edge_state {
                let final_text = text.clone();
                *edge_state = EdgeState::Cursor(PlaceholderState::default());
                return Some(final_text);
            }
        }
        None
    }

    pub fn update_leaf_edit_text(&mut self, text: String) {
        if let Some(Selection::Edge(_, EdgeState::EditingLeaf(ref mut current))) = self.editor.selection {
            *current = text;
        }
    }
}
