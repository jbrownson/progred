use crate::gid::Gid;
use crate::value::{Atom, NodeId, Value};
use im::HashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The entity table: maps are the only entities — identity-bearing,
/// mutable in place, shareable, cycle-capable. Keys are atoms by
/// type; values are anything sayable. See `docs/model.md`.
#[derive(Debug, Clone)]
pub struct MutGid {
    data: HashMap<NodeId, HashMap<Atom, Value>>,
}

impl MutGid {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn entities(&self) -> impl Iterator<Item = &NodeId> {
        self.data.keys()
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.data.ptr_eq(&other.data)
    }
}

impl Gid for MutGid {
    fn edges(&self, entity: NodeId) -> Option<&HashMap<Atom, Value>> {
        self.data.get(&entity)
    }
}

impl MutGid {
    pub fn set(&mut self, entity: NodeId, key: Atom, value: Value) {
        let edges = match self.data.get(&entity) {
            Some(edges) => edges.update(key, value),
            None => HashMap::unit(key, value),
        };
        self.data.insert(entity, edges);
    }

    pub fn merge(&mut self, other: MutGid) {
        for (entity, new_edges) in other.data {
            let merged = match self.data.get(&entity) {
                Some(existing) => existing.clone().union(new_edges),
                None => new_edges,
            };
            self.data.insert(entity, merged);
        }
    }

    /// Absent keys leave the map untouched, so `ptr_eq` stays an
    /// honest did-anything-change signal. An emptied entity is
    /// dropped: absence reads as the empty map.
    pub fn delete(&mut self, entity: NodeId, key: &Atom) {
        if let Some(edges) = self.data.get(&entity)
            && edges.contains_key(key)
        {
            let edges = edges.without(key);
            self.data = if edges.is_empty() {
                self.data.without(&entity)
            } else {
                self.data.update(entity, edges)
            };
        }
    }
}

impl Default for MutGid {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for MutGid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let proxy: std::collections::BTreeMap<NodeId, Vec<(Atom, Value)>> = self
            .data
            .iter()
            .map(|(entity, edges)| {
                let mut pairs: Vec<_> =
                    edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                pairs.sort();
                (*entity, pairs)
            })
            .collect();
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MutGid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let proxy: std::collections::HashMap<NodeId, Vec<(Atom, Value)>> =
            std::collections::HashMap::deserialize(deserializer)?;
        let data = proxy
            .into_iter()
            // An empty entity normalizes to absence, as `delete`
            // leaves it.
            .filter(|(_, edges)| !edges.is_empty())
            .map(|(entity, edges)| (entity, edges.into_iter().collect()))
            .collect();
        Ok(MutGid { data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::new_node_id;

    #[test]
    fn edges_set_get_delete_and_normalize() {
        let mut gid = MutGid::new();
        let entity = new_node_id();
        gid.set(entity, Atom::from("x"), Value::from(1.0));
        gid.set(entity, Atom::from("items"), Value::list([Value::from(2.0)]));
        assert_eq!(
            gid.get(entity, &Atom::from("x")),
            Some(&Value::from(1.0))
        );
        assert_eq!(
            gid.get(entity, &Atom::from("items")),
            Some(&Value::list([Value::from(2.0)]))
        );

        gid.delete(entity, &Atom::from("items"));
        gid.delete(entity, &Atom::from("x"));
        // The emptied entity is nothing at all.
        assert!(gid.edges(entity).is_none());
    }

    #[test]
    fn files_are_maps_of_atom_keyed_pairs_with_lists_inline() {
        let mut gid = MutGid::new();
        let entity = new_node_id();
        gid.set(
            entity,
            Atom::from("dash"),
            Value::list([Value::from(2.0), Value::from(3.0)]),
        );
        gid.set(entity, Atom::from(1.5), Value::from("numeric key"));

        let json = serde_json::to_string(&gid).unwrap();
        let loaded: MutGid = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.get(entity, &Atom::from("dash")),
            Some(&Value::list([Value::from(2.0), Value::from(3.0)]))
        );
        // The round trip is a fixed point even though minted
        // positions differ: only the order is the data.
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
        // A list in key position refuses to parse — by grammar, not
        // by gate.
        let bad = format!(r#"{{"{entity}": [[{{"list": []}}, {{"number": 1.0}}]]}}"#);
        assert!(serde_json::from_str::<MutGid>(&bad).is_err());
    }
}
