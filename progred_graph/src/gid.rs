use crate::id::Id;
use im::HashMap;

pub trait Gid {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>>;

    fn get(&self, entity: &Id, label: &Id) -> Option<&Id> {
        self.edges(entity)?.get(label)
    }
}

pub struct StackedGid<'a, Top: Gid, Bottom: Gid> {
    top: &'a Top,
    bottom: &'a Bottom,
}

impl<'a, Top: Gid, Bottom: Gid> StackedGid<'a, Top, Bottom> {
    pub fn new(top: &'a Top, bottom: &'a Bottom) -> Self {
        Self { top, bottom }
    }
}

impl<Top: Gid, Bottom: Gid> Gid for StackedGid<'_, Top, Bottom> {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>> {
        self.top.edges(entity).or_else(|| self.bottom.edges(entity))
    }
}
