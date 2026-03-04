use progred_graph::Id;
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
    ListElement {
        path: Path,
        cons_id: Id,
        edge_state: EdgeState,
    },
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

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Edge(p, _) => Some(p),
            Self::ListElement { path, .. } => Some(path),
            _ => None,
        }
    }

    pub fn edge_state_mut(&mut self) -> Option<&mut EdgeState> {
        match self {
            Self::Edge(_, es) => Some(es),
            Self::ListElement { edge_state, .. } => Some(edge_state),
            _ => None,
        }
    }
}
