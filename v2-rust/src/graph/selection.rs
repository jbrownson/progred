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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub target: SelectionTarget,
    pub placeholder: Option<PlaceholderState>,
}

impl Selection {
    pub fn edge(path: Path) -> Self {
        Self { target: SelectionTarget::Edge(path), placeholder: None }
    }

    pub fn insert_root(index: usize) -> Self {
        Self { target: SelectionTarget::InsertRoot(index), placeholder: None }
    }

    pub fn edge_path(&self) -> Option<&Path> {
        match &self.target {
            SelectionTarget::Edge(p) => Some(p),
            _ => None,
        }
    }

}
