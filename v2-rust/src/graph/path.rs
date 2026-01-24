use super::gid::Gid;
use super::id::Id;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct RootSlot(Rc<Id>);

impl RootSlot {
    pub fn new(node: Id) -> Self {
        RootSlot(Rc::new(node))
    }

    pub fn node(&self) -> &Id {
        &self.0
    }
}

impl PartialEq for RootSlot {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for RootSlot {}

impl Hash for RootSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    pub root: RootSlot,
    pub edges: Vec<Id>,
}

impl Path {
    pub fn new(root: RootSlot) -> Self {
        Self {
            root,
            edges: Vec::new(),
        }
    }

    pub fn child(&self, label: Id) -> Self {
        let mut edges = self.edges.clone();
        edges.push(label);
        Self {
            root: self.root.clone(),
            edges,
        }
    }

    pub fn pop(&self) -> Option<(Path, Id)> {
        if self.edges.is_empty() {
            None
        } else {
            let parent = Path {
                root: self.root.clone(),
                edges: self.edges[..self.edges.len() - 1].to_vec(),
            };
            let label = self.edges[self.edges.len() - 1].clone();
            Some((parent, label))
        }
    }

    pub fn is_root(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn node<'a>(&'a self, gid: &'a impl Gid) -> Option<&'a Id> {
        let mut current = self.root.node();
        for label in &self.edges {
            if !matches!(current, Id::Uuid(_)) {
                return None;
            }
            current = gid.get(current, label)?;
        }
        Some(current)
    }
}
