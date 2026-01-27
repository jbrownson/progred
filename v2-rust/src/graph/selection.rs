use super::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Edge(Path),
    InsertRoot(usize),
}
