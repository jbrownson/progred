use super::gid::Gid;
use super::id::Id;
use super::path::Path;
use crate::document::Document;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceholderState {
    pub text: String,
    pub selected_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionTarget {
    Edge(Path),
    InsertRoot(usize),
    GraphEdge { entity: Id, label: Id },
    GraphRoot(Id),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub target: SelectionTarget,
    pub placeholder: PlaceholderState,
}

impl Selection {
    pub fn edge(path: Path) -> Self {
        Self { target: SelectionTarget::Edge(path), placeholder: PlaceholderState::default() }
    }

    pub fn insert_root(index: usize) -> Self {
        Self { target: SelectionTarget::InsertRoot(index), placeholder: PlaceholderState::default() }
    }

    pub fn graph_edge(entity: Id, label: Id) -> Self {
        Self { target: SelectionTarget::GraphEdge { entity, label }, placeholder: PlaceholderState::default() }
    }

    pub fn graph_root(id: Id) -> Self {
        Self { target: SelectionTarget::GraphRoot(id), placeholder: PlaceholderState::default() }
    }

    pub fn edge_path(&self) -> Option<&Path> {
        match &self.target {
            SelectionTarget::Edge(p) => Some(p),
            _ => None,
        }
    }

    pub fn selected_node_id(&self, doc: &Document) -> Option<Id> {
        match &self.target {
            SelectionTarget::Edge(path) => doc.node(path),
            SelectionTarget::GraphEdge { entity, label } => doc.gid.edges(entity).and_then(|e| e.get(label)).cloned(),
            SelectionTarget::GraphRoot(id) => Some(id.clone()),
            SelectionTarget::InsertRoot(_) => None,
        }
    }

    pub fn placeholder_visible(&self, doc: &Document) -> bool {
        match &self.target {
            SelectionTarget::InsertRoot(_) => true,
            SelectionTarget::Edge(path) => doc.node(path).is_none(),
            SelectionTarget::GraphEdge { .. } | SelectionTarget::GraphRoot(_) => false,
        }
    }
}
