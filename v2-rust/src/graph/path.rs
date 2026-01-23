use super::id::Id;
use super::mutgid::MutGid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub root_slot: Id,
    pub edges: Vec<Id>,
}

impl Path {
    pub fn root(root_slot: Id) -> Self {
        Self {
            root_slot,
            edges: Vec::new(),
        }
    }

    pub fn child(&self, label: Id) -> Self {
        let mut edges = self.edges.clone();
        edges.push(label);
        Self {
            root_slot: self.root_slot.clone(),
            edges,
        }
    }

    pub fn pop(&self) -> Option<(Path, Id)> {
        if self.edges.is_empty() {
            None
        } else {
            let parent = Path {
                root_slot: self.root_slot.clone(),
                edges: self.edges[..self.edges.len() - 1].to_vec(),
            };
            let label = self.edges[self.edges.len() - 1].clone();
            Some((parent, label))
        }
    }

    pub fn is_root(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn node<'a>(&self, gid: &'a MutGid, root_node: Option<&'a Id>) -> Option<&'a Id> {
        let mut current = root_node?;
        for label in &self.edges {
            if !matches!(current, Id::Uuid(_)) {
                return None;
            }
            current = gid.get(current, label)?;
        }
        Some(current)
    }
}
