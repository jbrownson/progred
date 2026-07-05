use progred_graph::{Gid, Id};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathRoot {
    Root,
    Orphan(Id),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    pub root: PathRoot,
    pub edges: Vec<Id>,
}

impl Path {
    pub fn root() -> Self {
        Self {
            root: PathRoot::Root,
            edges: Vec::new(),
        }
    }

    pub fn orphan(id: Id) -> Self {
        Self {
            root: PathRoot::Orphan(id),
            edges: Vec::new(),
        }
    }

    pub fn child(&self, label: Id) -> Self {
        Self {
            root: self.root.clone(),
            edges: self.edges.iter().cloned().chain([label]).collect(),
        }
    }

    pub fn pop(&self) -> Option<(Path, Id)> {
        let (label, parent_edges) = self.edges.split_last()?;
        Some((
            Path { root: self.root.clone(), edges: parent_edges.to_vec() },
            label.clone(),
        ))
    }

    pub fn node(&self, gid: &impl Gid, root: Option<&Id>) -> Option<Id> {
        let start = match &self.root {
            PathRoot::Root => root?.clone(),
            PathRoot::Orphan(id) => id.clone(),
        };
        self.edges.iter().try_fold(start, |current, label| {
            match &current {
                Id::Uuid(_) => gid.get(&current, label).cloned(),
                _ => None,
            }
        })
    }
}
