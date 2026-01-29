use crate::graph::{Id, MutGid, Path, PlaceholderState, RootSlot, Selection, SpanningTree};
use crate::ui::graph_view::GraphViewState;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Document {
    pub gid: MutGid,
    pub roots: Vec<RootSlot>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            gid: MutGid::new(),
            roots: Vec::new(),
        }
    }

    pub fn delete_path(&mut self, path: &Path) {
        match path.pop() {
            None => {
                if let Some(idx) = self.roots.iter().position(|r| r == &path.root) {
                    self.roots.remove(idx);
                }
            }
            Some((parent_path, label)) => {
                if let Some(parent_node @ Id::Uuid(_)) = parent_path.node(&self.gid).cloned() {
                    self.gid.delete(&parent_node, &label);
                }
            }
        }
    }

    pub fn set_edge(&mut self, path: &Path, value: Id) {
        match path.pop() {
            Some((parent_path, label)) => {
                if let Some(parent_node @ Id::Uuid(_)) = parent_path.node(&self.gid).cloned() {
                    self.gid.set(parent_node, label, value);
                }
            }
            None => {
                if let Some(idx) = self.roots.iter().position(|r| r == &path.root) {
                    self.roots[idx] = RootSlot::new(value);
                }
            }
        }
    }
}

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
        self.editor.tree = self.editor.tree.set_collapsed_at_path(path, collapsed);
    }

    pub fn insert_root(&mut self, index: usize, root: RootSlot) {
        self.editor.doc.roots.insert(index, root);
    }

    pub fn placeholder_state(&mut self) -> Option<&mut PlaceholderState> {
        self.editor.selection.as_mut().map(|s| &mut s.placeholder)
    }

    pub fn graph_view(&mut self) -> &mut GraphViewState {
        &mut self.editor.graph_view
    }
}
