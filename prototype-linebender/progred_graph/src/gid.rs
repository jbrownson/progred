use crate::value::{Atom, NodeId, Value};
use im::HashMap;

/// The entity-reading interface: what a stored or computed entity
/// answers. `MutGid` is the stored implementor; computed entities are
/// the anticipated other.
pub trait Gid {
    fn edges(&self, entity: NodeId) -> Option<&HashMap<Atom, Value>>;

    fn get(&self, entity: NodeId, key: &Atom) -> Option<&Value> {
        self.edges(entity)?.get(key)
    }
}

impl<T: Gid + ?Sized> Gid for &T {
    fn edges(&self, entity: NodeId) -> Option<&HashMap<Atom, Value>> {
        (**self).edges(entity)
    }
}
