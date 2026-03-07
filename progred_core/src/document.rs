use crate::graph::{Gid, Id, MutGid};
use crate::path::{Path, PathRoot};
use crate::selection::Selection;
use crate::generated::semantics::list;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(rename = "graph")]
    pub gid: MutGid,
    pub root: Option<Id>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            gid: MutGid::new(),
            root: None,
        }
    }

    pub fn node(&self, path: &Path) -> Option<Id> {
        path.node(&self.gid, self.root.as_ref())
    }

    pub fn delete(&mut self, selection: &Selection) {
        match selection {
            Selection::Edge(path, _) => self.delete_edge(path),
            Selection::ListElement { path, cons_id, .. } => {
                self.splice_out_list_element(path, cons_id);
            }
            Selection::GraphEdge { entity: Id::Uuid(uuid), label } => {
                self.gid.delete(uuid, label);
            }
            Selection::GraphEdge { .. } => {}
            Selection::GraphNode(id) => {
                if self.root.as_ref() == Some(id) {
                    self.root = None;
                }
                self.gid.purge(id);
            }
        }
        self.gc();
    }

    fn gc(&mut self) {
        let all_entities: HashSet<Id> = self.gid.entities().map(|u| Id::Uuid(*u)).collect();
        let reachable = reachable_from(&self.gid, self.root.iter().cloned(), &all_entities);
        self.gid.retain_entities(&reachable);
    }

    fn splice_out_list_element(&mut self, head_path: &Path, cons_id: &Id) {
        if let Some((cons_path, _)) = head_path.pop()
            && let Some(tail_value) = self.gid.get(cons_id, &list::Cons::<()>::TAIL).cloned()
        {
            self.set_edge(&cons_path, tail_value);
        }
    }

    fn delete_edge(&mut self, path: &Path) {
        match path.pop() {
            None => {
                if path.root == PathRoot::Root {
                    self.root = None;
                }
            }
            Some((parent_path, label)) => {
                if let Some(Id::Uuid(parent_uuid)) = self.node(&parent_path) {
                    self.gid.delete(&parent_uuid, &label);
                }
            }
        }
    }

    pub fn set_edge(&mut self, path: &Path, value: Id) {
        match path.pop() {
            Some((parent_path, label)) => {
                if let Some(Id::Uuid(parent_uuid)) = self.node(&parent_path) {
                    self.gid.set(parent_uuid, label, value);
                }
            }
            None => {
                if path.root == PathRoot::Root {
                    self.root = Some(value);
                }
            }
        }
    }

    pub fn orphan_roots(&self) -> HashSet<Id> {
        let all_nodes: HashSet<Id> = self.gid.entities().map(|u| Id::Uuid(*u)).collect();
        let orphans = all_nodes.difference(
            &reachable_from(&self.gid, self.root.iter().cloned(), &all_nodes)
        ).cloned().collect();
        let sources = sources_within(&self.gid, &orphans);
        let cycle_rep = cycle_representative(&self.gid, &orphans, &sources);

        sources.into_iter().chain(cycle_rep).collect()
    }
}

fn reachable_from(gid: &impl Gid, starts: impl Iterator<Item = Id>, within: &HashSet<Id>) -> HashSet<Id> {
    let mut reachable = HashSet::new();
    let mut queue: VecDeque<Id> = starts.collect();
    while let Some(id) = queue.pop_front() {
        if within.contains(&id) && reachable.insert(id.clone())
            && let Some(edges) = gid.edges(&id)
        {
            for (label, value) in edges.iter() {
                queue.push_back(label.clone());
                queue.push_back(value.clone());
            }
        }
    }
    reachable
}

fn sources_within(gid: &impl Gid, set: &HashSet<Id>) -> Vec<Id> {
    let has_incoming: HashSet<Id> = set.iter()
        .filter(|n| matches!(n, Id::Uuid(_)))
        .flat_map(|n| {
            gid.edges(n).into_iter().flat_map(|edges| {
                edges.iter().filter(|(_, v)| set.contains(v)).map(|(_, v)| v.clone())
            })
        })
        .collect();

    set.iter().filter(|n| !has_incoming.contains(n)).cloned().collect()
}

fn cycle_representative(gid: &impl Gid, orphans: &HashSet<Id>, sources: &[Id]) -> Option<Id> {
    orphans.difference(&reachable_from(gid, sources.iter().cloned(), orphans))
        .filter(|n| matches!(n, Id::Uuid(_)))
        .min()
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u128) -> Id {
        Id::Uuid(uuid::Uuid::from_u128(n))
    }

    fn make_gid(edges: &[(u128, u128, u128)]) -> MutGid {
        let mut gid = MutGid::new();
        for &(entity, label, value) in edges {
            gid.merge(im::hashmap! {
                uuid::Uuid::from_u128(entity) => im::hashmap! {
                    uuid(label) => uuid(value),
                }
            });
        }
        gid
    }

    #[test]
    fn reachable_from_empty() {
        let gid = MutGid::new();
        let within = HashSet::new();
        let result = reachable_from(&gid, std::iter::empty(), &within);
        assert!(result.is_empty());
    }

    #[test]
    fn reachable_from_chain() {
        let gid = make_gid(&[(1, 100, 2), (2, 100, 3)]);
        let within: HashSet<Id> = [1, 2, 3, 100].into_iter().map(uuid).collect();
        let result = reachable_from(&gid, std::iter::once(uuid(1)), &within);
        assert!(result.contains(&uuid(1)));
        assert!(result.contains(&uuid(2)));
        assert!(result.contains(&uuid(3)));
        assert!(result.contains(&uuid(100)));
    }

    #[test]
    fn reachable_from_respects_within() {
        let gid = make_gid(&[(1, 100, 2)]);
        let within: HashSet<Id> = [1, 100].into_iter().map(uuid).collect();
        let result = reachable_from(&gid, std::iter::once(uuid(1)), &within);
        assert!(result.contains(&uuid(1)));
        assert!(!result.contains(&uuid(2)));
    }

    #[test]
    fn sources_within_single_node() {
        let gid = MutGid::new();
        let set: HashSet<Id> = [1].into_iter().map(uuid).collect();
        let sources = sources_within(&gid, &set);
        assert_eq!(sources, vec![uuid(1)]);
    }

    #[test]
    fn sources_within_chain() {
        let gid = make_gid(&[(1, 100, 2)]);
        let set: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources = sources_within(&gid, &set);
        assert!(sources.contains(&uuid(1)));
        assert!(!sources.contains(&uuid(2)));
    }

    #[test]
    fn sources_within_cycle_no_sources() {
        let gid = make_gid(&[(1, 100, 2), (2, 100, 1)]);
        let set: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources = sources_within(&gid, &set);
        assert!(sources.is_empty());
    }

    #[test]
    fn cycle_representative_no_cycle() {
        let gid = make_gid(&[(1, 100, 2)]);
        let orphans: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources = vec![uuid(1)];
        let rep = cycle_representative(&gid, &orphans, &sources);
        assert!(rep.is_none());
    }

    #[test]
    fn cycle_representative_picks_min() {
        let gid = make_gid(&[(1, 100, 2), (2, 100, 1)]);
        let orphans: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources: Vec<Id> = vec![];
        let rep = cycle_representative(&gid, &orphans, &sources);
        assert_eq!(rep, Some(uuid(1)));
    }

    #[test]
    fn orphan_roots_no_orphans() {
        let gid = make_gid(&[(1, 100, 2)]);
        let doc = Document { gid, root: Some(uuid(1)) };
        assert!(doc.orphan_roots().is_empty());
    }

    #[test]
    fn orphan_roots_single_orphan() {
        let mut gid = make_gid(&[(1, 100, 2)]);
        gid.merge(im::hashmap! { uuid::Uuid::from_u128(3) => im::hashmap! { uuid(100) => uuid(4) } });
        let doc = Document { gid, root: Some(uuid(1)) };
        let orphans = doc.orphan_roots();
        assert!(orphans.contains(&uuid(3)));
    }

    #[test]
    fn orphan_roots_cycle() {
        let mut gid = make_gid(&[(2, 100, 3), (3, 100, 2)]);
        gid.merge(im::hashmap! { uuid::Uuid::from_u128(1) => im::hashmap! { uuid(100) => uuid(1) } });
        let doc = Document { gid, root: Some(uuid(1)) };
        let orphans = doc.orphan_roots();
        assert_eq!(orphans.len(), 1);
        assert!(orphans.contains(&uuid(2)));
    }
}
