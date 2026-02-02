use super::gid::Gid;
use super::id::Id;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct RootSlot {
    id: uuid::Uuid,
    pub value: Id,
}

impl RootSlot {
    pub fn new(value: Id) -> Self {
        Self { id: uuid::Uuid::new_v4(), value }
    }
}

impl PartialEq for RootSlot {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq<RootId> for RootSlot {
    fn eq(&self, other: &RootId) -> bool {
        self.id == other.0
    }
}

impl Eq for RootSlot {}

impl Hash for RootSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootId(uuid::Uuid);

impl From<&RootSlot> for RootId {
    fn from(slot: &RootSlot) -> Self {
        RootId(slot.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathRoot {
    Slot(RootId),
    Orphan(Id),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    pub root: PathRoot,
    pub edges: Vec<Id>,
}

impl Path {
    pub fn new(root: &RootSlot) -> Self {
        Self {
            root: PathRoot::Slot(RootId::from(root)),
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

    pub fn node(&self, gid: &impl Gid, roots: &[RootSlot]) -> Option<Id> {
        let start = match &self.root {
            PathRoot::Slot(root_id) => roots.iter().find(|r| **r == *root_id)?.value.clone(),
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
