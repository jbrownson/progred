use super::id::Id;
use super::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceholderState {
    pub text: String,
    pub selected_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeState {
    Cursor(PlaceholderState),
    EditingLeaf(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Edge(Path, EdgeState),
    InsertRoot(usize, PlaceholderState),
    GraphEdge { entity: Id, label: Id },
    GraphNode(Id),
}

impl Selection {
    pub fn edge(path: Path) -> Self {
        Self::Edge(path, EdgeState::Cursor(PlaceholderState::default()))
    }

    pub fn insert_root(index: usize) -> Self {
        Self::InsertRoot(index, PlaceholderState::default())
    }

    pub fn graph_edge(entity: Id, label: Id) -> Self {
        Self::GraphEdge { entity, label }
    }

    pub fn graph_node(id: Id) -> Self {
        Self::GraphNode(id)
    }

    pub fn edge_path(&self) -> Option<&Path> {
        match self {
            Self::Edge(p, _) => Some(p),
            _ => None,
        }
    }
}
