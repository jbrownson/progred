use crate::gid::Gid;
use crate::id::Id;
use im::HashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MutGid {
    data: HashMap<Uuid, HashMap<Id, Id>>,
}

impl MutGid {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn entities(&self) -> impl Iterator<Item = &Uuid> {
        self.data.keys()
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.data.ptr_eq(&other.data)
    }
}

impl Gid for MutGid {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>> {
        match entity {
            Id::Uuid(uuid) => self.data.get(uuid),
            _ => None,
        }
    }
}

impl MutGid {
    pub fn set(&mut self, entity: Uuid, label: Id, value: Id) {
        let edges = match self.data.get(&entity) {
            Some(e) => e.update(label, value),
            None => HashMap::unit(label, value),
        };
        self.data.insert(entity, edges);
    }

    pub fn merge(&mut self, other: HashMap<Uuid, HashMap<Id, Id>>) {
        for (entity, new_edges) in other {
            let merged = match self.data.get(&entity) {
                Some(existing) => existing.clone().union(new_edges),
                None => new_edges,
            };
            self.data.insert(entity, merged);
        }
    }

    pub fn delete(&mut self, entity: &Uuid, label: &Id) {
        if let Some(edges) = self.data.get(entity) {
            let new_edges = edges.without(label);
            self.data = if new_edges.is_empty() {
                self.data.without(entity)
            } else {
                self.data.update(*entity, new_edges)
            };
        }
    }

    pub fn retain_entities(&mut self, keep: &std::collections::HashSet<Id>) {
        self.data = self.data.iter()
            .filter(|(uuid, _)| keep.contains(&Id::Uuid(**uuid)))
            .map(|(&k, v)| (k, v.clone()))
            .collect();
    }

    pub fn purge(&mut self, id: &Id) {
        if let Id::Uuid(uuid) = id {
            self.data = self.data.without(uuid);
        }
        self.data = self.data.iter()
            .map(|(&entity, edges)| {
                let filtered = edges.iter()
                    .filter(|(_, v)| *v != id)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                (entity, filtered)
            })
            .filter(|(_, edges): &(_, im::HashMap<Id, Id>)| !edges.is_empty())
            .collect();
    }
}

impl Default for MutGid {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for MutGid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let proxy: std::collections::BTreeMap<Uuid, Vec<(Id, Id)>> = self.data
            .iter()
            .map(|(uuid, edges)| {
                let mut pairs: Vec<_> = edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                pairs.sort();
                (*uuid, pairs)
            })
            .collect();
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MutGid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let proxy: std::collections::HashMap<Uuid, Vec<(Id, Id)>> =
            std::collections::HashMap::deserialize(deserializer)?;
        let data = proxy.into_iter()
            .map(|(uuid, edges)| (uuid, edges.into_iter().collect()))
            .collect();
        Ok(MutGid { data })
    }
}
