use super::id::Id;
use super::path::Path;

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
    pub leaf_edit_text: Option<String>,
}

impl Selection {
    pub fn edge(path: Path) -> Self {
        Self { target: SelectionTarget::Edge(path), placeholder: PlaceholderState::default(), leaf_edit_text: None }
    }

    pub fn insert_root(index: usize) -> Self {
        Self { target: SelectionTarget::InsertRoot(index), placeholder: PlaceholderState::default(), leaf_edit_text: None }
    }

    pub fn graph_edge(entity: Id, label: Id) -> Self {
        Self { target: SelectionTarget::GraphEdge { entity, label }, placeholder: PlaceholderState::default(), leaf_edit_text: None }
    }

    pub fn graph_root(id: Id) -> Self {
        Self { target: SelectionTarget::GraphRoot(id), placeholder: PlaceholderState::default(), leaf_edit_text: None }
    }

    pub fn edge_path(&self) -> Option<&Path> {
        match &self.target {
            SelectionTarget::Edge(p) => Some(p),
            _ => None,
        }
    }
}
