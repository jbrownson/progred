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
            self.data = if new_edges.is_empty() {
                self.data.without(entity)
            } else {
                self.data.update(entity.clone(), new_edges)
            };
        }
    }

    pub fn to_json(&self) -> StdHashMap<String, StdHashMap<String, serde_json::Value>> {
        self.data
            .iter()
            .filter_map(|(entity, edges)| {
                let Id::Uuid(entity_uuid) = entity else { return None };
                let edge_obj = edges
                    .iter()
                    .filter_map(|(label, value)| {
                        let Id::Uuid(label_uuid) = label else { return None };
                        serde_json::to_value(value)
                            .ok()
                            .map(|json| (label_uuid.to_string(), json))
                    })
                    .collect();
                Some((entity_uuid.to_string(), edge_obj))
            })
            .collect()
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
