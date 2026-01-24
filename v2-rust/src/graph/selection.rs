use super::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Edge(Path),
    InsertRoot(usize),
}

impl Selection {
    pub fn edge(path: Path) -> Self {
        Selection::Edge(path)
    }

    pub fn insert_root(index: usize) -> Self {
        Selection::InsertRoot(index)
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Selection::Edge(p) => Some(p),
            _ => None,
        }
    }
}
