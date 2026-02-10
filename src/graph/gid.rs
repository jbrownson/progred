use super::id::Id;
use im::HashMap;

pub trait Gid {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>>;

    fn get(&self, entity: &Id, label: &Id) -> Option<&Id> {
        self.edges(entity)?.get(label)
    }
}
