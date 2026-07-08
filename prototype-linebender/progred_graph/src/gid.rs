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

// StackedGid (a Gid-implementing document-over-library merge) lived
// here through 2026-07-08; retired for the editor's explicit
// `Sources` — a combined view that masquerades as one graph erases
// the provenance the editor then has to bolt back on.
