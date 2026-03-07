use crate::id::Id;
use im::HashMap;

pub trait Gid {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>>;

    fn get(&self, entity: &Id, label: &Id) -> Option<&Id> {
        self.edges(entity)?.get(label)
    }
}

impl<T: Gid + ?Sized> Gid for &T {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>> {
        (**self).edges(entity)
    }
}

pub struct StackedGid<Top: Gid, Bottom: Gid> {
    top: Top,
    bottom: Bottom,
}

impl<Top: Gid, Bottom: Gid> StackedGid<Top, Bottom> {
    pub fn new(top: Top, bottom: Bottom) -> Self {
        Self { top, bottom }
    }
}

impl<Top: Gid, Bottom: Gid> Gid for StackedGid<Top, Bottom> {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>> {
        self.top.edges(entity).or_else(|| self.bottom.edges(entity))
    }
}
