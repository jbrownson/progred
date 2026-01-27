use crate::graph::{Id, MutGid, Path, RootSlot, Selection, SpanningTree};
use std::path::PathBuf;

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

pub struct Editor {
    pub doc: Document,
    pub tree: SpanningTree,
    pub selection: Option<Selection>,
    pub file_path: Option<PathBuf>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            tree: SpanningTree::empty(),
            selection: None,
            file_path: None,
        }
    }
}
