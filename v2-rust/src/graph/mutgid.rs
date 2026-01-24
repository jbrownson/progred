use super::gid::Gid;
use super::id::Id;
use im::HashMap;
use std::collections::HashMap as StdHashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MutGid {
    data: HashMap<Id, HashMap<Id, Id>>,
}

impl MutGid {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

impl Gid for MutGid {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>> {
        self.data.get(entity)
    }
}

impl MutGid {
    pub fn set(&mut self, entity: Id, label: Id, value: Id) {
        let edges = match self.data.get(&entity) {
            Some(e) => e.update(label, value),
            None => HashMap::unit(label, value),
        };
        self.data.insert(entity, edges);
    }

    pub fn delete(&mut self, entity: &Id, label: &Id) {
        if let Some(edges) = self.data.get(entity) {
            let new_edges = edges.without(label);
            if new_edges.is_empty() {
                self.data = self.data.without(entity);
            } else {
                self.data.insert(entity.clone(), new_edges);
            }
        }
    }

    pub fn has(&self, entity: &Id) -> bool {
        self.data.contains_key(entity)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Id> {
        self.data.keys()
    }

    pub fn to_json(&self) -> StdHashMap<String, StdHashMap<String, serde_json::Value>> {
        let mut result = StdHashMap::new();
        for (entity, edges) in self.data.iter() {
            if let Id::Uuid(entity_uuid) = entity {
                let mut edge_obj = StdHashMap::new();
                for (label, value) in edges.iter() {
                    if let Id::Uuid(label_uuid) = label {
                        if let Ok(json) = serde_json::to_value(value) {
                            edge_obj.insert(label_uuid.to_string(), json);
                        }
                    }
                }
                result.insert(entity_uuid.to_string(), edge_obj);
            }
        }
        result
    }

    pub fn from_json(
        json: StdHashMap<String, StdHashMap<String, serde_json::Value>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut gid = MutGid::new();
        for (entity_str, edges) in json {
            let entity = Id::Uuid(Uuid::parse_str(&entity_str)?);
            for (label_str, value_json) in edges {
                let label = Id::Uuid(Uuid::parse_str(&label_str)?);
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
