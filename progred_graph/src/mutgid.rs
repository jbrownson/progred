use crate::gid::Gid;
use crate::id::Id;
use im::HashMap;
use std::collections::HashMap as StdHashMap;
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

    pub fn to_json(&self) -> StdHashMap<String, StdHashMap<String, serde_json::Value>> {
        self.data
            .iter()
            .map(|(entity_uuid, edges)| {
                let edge_obj = edges
                    .iter()
                    .filter_map(|(label, value)| match label {
                        Id::Uuid(label_uuid) => serde_json::to_value(value)
                            .ok()
                            .map(|json| (label_uuid.to_string(), json)),
                        _ => None,
                    })
                    .collect();
                (entity_uuid.to_string(), edge_obj)
            })
            .collect()
    }

    pub fn from_json(
        json: StdHashMap<String, StdHashMap<String, serde_json::Value>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut gid = MutGid::new();
        for (entity_str, edges) in json {
            let entity = Uuid::parse_str(&entity_str)?;
            for (label_str, value_json) in edges {
                let label = Id::Uuid(Uuid::parse_str(&label_str)?);
                let id: Id = serde_json::from_value(value_json)?;
                gid.set(entity, label, id);
            }
        }
        Ok(gid)
    }
}

impl Default for MutGid {
    fn default() -> Self {
        Self::new()
    }
}
