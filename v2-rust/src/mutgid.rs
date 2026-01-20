use crate::id::{GuidId, Id};
use im::HashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap as StdHashMap;

#[derive(Debug, Clone)]
pub struct MutGid {
    data: HashMap<GuidId, HashMap<GuidId, Id>>,
}

impl MutGid {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn get(&self, entity: &GuidId, label: &GuidId) -> Option<&Id> {
        self.data.get(entity)?.get(label)
    }

    pub fn edges(&self, entity: &GuidId) -> Option<&HashMap<GuidId, Id>> {
        self.data.get(entity)
    }

    pub fn set(&mut self, entity: GuidId, label: GuidId, value: Id) {
        let edges = match self.data.get(&entity) {
            Some(e) => e.update(label, value),
            None => HashMap::unit(label, value),
        };
        self.data.insert(entity, edges);
    }

    pub fn delete(&mut self, entity: &GuidId, label: &GuidId) {
        if let Some(edges) = self.data.get(entity) {
            let new_edges = edges.without(label);
            if new_edges.is_empty() {
                self.data = self.data.without(entity);
            } else {
                self.data.insert(entity.clone(), new_edges);
            }
        }
    }

    pub fn has(&self, entity: &GuidId) -> bool {
        self.data.contains_key(entity)
    }

    pub fn entities(&self) -> impl Iterator<Item = &GuidId> {
        self.data.keys()
    }

    pub fn to_json(&self) -> StdHashMap<String, StdHashMap<String, serde_json::Value>> {
        let mut result = StdHashMap::new();
        for (entity, edges) in self.data.iter() {
            let mut edge_obj = StdHashMap::new();
            for (label, value) in edges.iter() {
                if let Ok(json) = serde_json::to_value(value) {
                    edge_obj.insert(label.guid.clone(), json);
                }
            }
            result.insert(entity.guid.clone(), edge_obj);
        }
        result
    }

    pub fn from_json(
        json: StdHashMap<String, StdHashMap<String, serde_json::Value>>,
    ) -> Result<Self, serde_json::Error> {
        let mut gid = MutGid::new();
        for (entity_guid, edges) in json {
            let entity = GuidId::new(entity_guid);
            for (label_guid, value_json) in edges {
                let label = GuidId::new(label_guid);
                let id: Id = serde_json::from_value(value_json)?;
                gid.set(entity.clone(), label, id);
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
