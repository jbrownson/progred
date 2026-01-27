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

    pub fn node<'a>(&'a self, gid: &'a impl Gid) -> Option<&'a Id> {
        self.edges.iter().try_fold(self.root.node(), |current, label| {
            match current {
                Id::Uuid(_) => gid.get(current, label),
                _ => None,
            }
        })
    }
}
