use crate::id::Id;
use crate::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceholderState {
    pub text: String,
    pub selected_index: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdgeState {
    pub placeholder: PlaceholderState,
    pub number_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Edge(Path, EdgeState),
    InsertRoot(usize, PlaceholderState),
    InsertList(Path, PlaceholderState),
    GraphEdge { entity: Id, label: Id },
    GraphNode(Id),
}

impl Selection {
    pub fn edge(path: Path) -> Self {
        Self::Edge(path, EdgeState::default())
    }

    pub fn insert_root(index: usize) -> Self {
        Self::InsertRoot(index, PlaceholderState::default())
    }

    pub fn edge_path(&self) -> Option<&Path> {
        match self {
            Self::Edge(p, _) => Some(p),
            _ => None,
        }
    }
}
