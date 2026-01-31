use crate::graph::{Gid, Id, MutGid, Path, PlaceholderState, RootSlot, Selection, SelectionTarget, SpanningTree};
use crate::ui::graph_view::GraphViewState;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct Semantics {
    pub name_field: Option<Id>,
    pub isa_field: Option<Id>,
}

#[derive(Clone)]
pub struct Document {
    pub gid: MutGid,
    pub roots: Vec<RootSlot>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            gid: MutGid::new(),
            roots: Vec::new(),
        }
    }

    pub fn node(&self, path: &Path) -> Option<Id> {
        path.node(&self.gid, &self.roots)
    }

    pub fn delete(&mut self, target: &SelectionTarget) {
        match target {
            SelectionTarget::Edge(path) => self.delete_path(path),
            SelectionTarget::GraphEdge { entity, label } => self.gid.delete(entity, label),
            SelectionTarget::GraphRoot(id) => self.roots.retain(|r| &r.value != id),
            SelectionTarget::InsertRoot(_) => {}
        }
    }

    fn delete_path(&mut self, path: &Path) {
        match path.pop() {
            None => {
                if let Some(idx) = self.roots.iter().position(|r| *r == path.root) {
                    self.roots.remove(idx);
                }
            }
            Some((parent_path, label)) => {
                if let Some(parent_node @ Id::Uuid(_)) = self.node(&parent_path) {
                    self.gid.delete(&parent_node, &label);
                }
            }
        }
    }

    pub fn set_edge(&mut self, path: &Path, value: Id) {
        match path.pop() {
            Some((parent_path, label)) => {
                if let Some(parent_node @ Id::Uuid(_)) = self.node(&parent_path) {
                    self.gid.set(parent_node, label, value);
                }
            }
            None => {
                if let Some(root) = self.roots.iter_mut().find(|r| **r == path.root) {
                    root.value = value;
                }
            }
        }
    }

    pub fn orphan_roots(&self) -> Vec<Id> {
        let all_nodes: HashSet<Id> = self.gid.all_nodes().cloned().collect();
        let orphans = all_nodes.difference(
            &reachable_from(&self.gid, self.roots.iter().map(|r| r.value.clone()), &all_nodes)
        ).cloned().collect();
        let sources = sources_within(&self.gid, &orphans);
        let cycle_rep = cycle_representative(&self.gid, &orphans, &sources);

        let mut result: Vec<Id> = sources.into_iter().chain(cycle_rep).collect();
        result.sort();
        result
    }

    pub fn to_json(&self) -> serde_json::Value {
        let root_ids: Vec<_> = self.roots.iter().map(|r| &r.value).collect();
        serde_json::json!({
            "graph": self.gid.to_json(),
            "roots": root_ids,
        })
    }

    pub fn from_json(contents: &str) -> Option<Self> {
        let json_doc: serde_json::Value = serde_json::from_str(contents).ok()?;
        let graph_data = serde_json::from_value(json_doc.get("graph")?.clone()).ok()?;
        let gid = MutGid::from_json(graph_data).ok()?;
        let root_ids: Vec<Id> = serde_json::from_value(json_doc.get("roots")?.clone()).ok()?;
        Some(Self { gid, roots: root_ids.into_iter().map(RootSlot::new).collect() })
    }
}

fn reachable_from(gid: &impl Gid, starts: impl Iterator<Item = Id>, within: &HashSet<Id>) -> HashSet<Id> {
    let mut reachable = HashSet::new();
    let mut queue: VecDeque<Id> = starts.collect();
    while let Some(id) = queue.pop_front() {
        if within.contains(&id) && reachable.insert(id.clone()) {
            if let Some(edges) = gid.edges(&id) {
                for (label, value) in edges.iter() {
                    queue.push_back(label.clone());
                    queue.push_back(value.clone());
                }
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
                edges.iter().filter_map(|(_, v)| set.contains(v).then(|| v.clone()))
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

#[derive(Clone)]
pub struct Editor {
    pub doc: Document,
    pub tree: SpanningTree,
    pub selection: Option<Selection>,
    pub file_path: Option<PathBuf>,
    pub graph_view: GraphViewState,
    pub editing_leaf: bool,
    pub semantics: Semantics,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            tree: SpanningTree::empty(),
            selection: None,
            file_path: None,
            graph_view: GraphViewState::new(),
            editing_leaf: false,
            semantics: Semantics::default(),
        }
    }

    pub fn name_of(&self, id: &Id) -> Option<String> {
        self.semantics.name_field.as_ref()
            .and_then(|name_field| self.doc.gid.get(id, name_field))
            .and_then(|value| match value {
                Id::String(s) => Some(s.clone()),
                _ => None,
            })
    }

    pub fn display_label(&self, id: &Id) -> Option<String> {
        let isa_name = self.semantics.isa_field.as_ref()
            .and_then(|isa_field| self.doc.gid.get(id, isa_field))
            .and_then(|isa_id| self.name_of(isa_id));

        match (isa_name, self.name_of(id)) {
            (Some(isa), Some(n)) => Some(format!("{isa} \"{n}\"")),
            (Some(isa), None) => Some(isa),
            (None, Some(n)) => Some(format!("\"{n}\"")),
            (None, None) => None,
        }
    }
}

pub struct EditorWriter<'a> {
    editor: &'a mut Editor,
}

impl<'a> EditorWriter<'a> {
    pub fn new(editor: &'a mut Editor) -> Self {
        Self { editor }
    }

    pub fn select(&mut self, selection: Option<Selection>) {
        self.editor.selection = selection;
    }

    pub fn set_edge(&mut self, path: &Path, value: Id) {
        self.editor.doc.set_edge(path, value);
    }

    pub fn set_collapsed(&mut self, path: &Path, collapsed: bool) {
        self.editor.tree = self.editor.tree.set_collapsed_at_path(path, collapsed);
    }

    pub fn insert_root(&mut self, index: usize, value: Id) {
        self.editor.doc.roots.insert(index, RootSlot::new(value));
    }

    pub fn set_placeholder_state(&mut self, state: PlaceholderState) {
        if let Some(ref mut sel) = self.editor.selection {
            sel.placeholder = state;
        }
    }

    pub fn set_graph_view(&mut self, state: GraphViewState) {
        self.editor.graph_view = state;
    }

    pub fn set_editing_leaf(&mut self, editing: bool) {
        self.editor.editing_leaf = editing;
        if !editing {
            if let Some(ref mut sel) = self.editor.selection {
                sel.leaf_edit_text = None;
            }
        }
    }

    pub fn set_leaf_edit_text(&mut self, text: Option<String>) {
        if let Some(ref mut sel) = self.editor.selection {
            sel.leaf_edit_text = text;
        }
    }

    pub fn set_name_field(&mut self, field: Option<Id>) {
        self.editor.semantics.name_field = field;
    }

    pub fn set_isa_field(&mut self, field: Option<Id>) {
        self.editor.semantics.isa_field = field;
    }
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
            gid.set(uuid(entity), uuid(label), uuid(value));
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
        // A -[L]-> B -[L]-> C
        let gid = make_gid(&[(1, 100, 2), (2, 100, 3)]);
        let within: HashSet<Id> = [1, 2, 3, 100].into_iter().map(uuid).collect();
        let result = reachable_from(&gid, std::iter::once(uuid(1)), &within);
        assert!(result.contains(&uuid(1)));
        assert!(result.contains(&uuid(2)));
        assert!(result.contains(&uuid(3)));
        assert!(result.contains(&uuid(100))); // labels are also reachable
    }

    #[test]
    fn reachable_from_respects_within() {
        // A -[L]-> B, but B not in within
        let gid = make_gid(&[(1, 100, 2)]);
        let within: HashSet<Id> = [1, 100].into_iter().map(uuid).collect();
        let result = reachable_from(&gid, std::iter::once(uuid(1)), &within);
        assert!(result.contains(&uuid(1)));
        assert!(!result.contains(&uuid(2))); // not in within
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
        // A -[L]-> B: A is source, B has incoming
        let gid = make_gid(&[(1, 100, 2)]);
        let set: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources = sources_within(&gid, &set);
        assert!(sources.contains(&uuid(1)));
        assert!(!sources.contains(&uuid(2)));
    }

    #[test]
    fn sources_within_cycle_no_sources() {
        // A -[L]-> B -[L]-> A: pure cycle, no sources
        let gid = make_gid(&[(1, 100, 2), (2, 100, 1)]);
        let set: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources = sources_within(&gid, &set);
        assert!(sources.is_empty());
    }

    #[test]
    fn cycle_representative_no_cycle() {
        // A -[L]-> B: no cycle, A is source
        let gid = make_gid(&[(1, 100, 2)]);
        let orphans: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources = vec![uuid(1)];
        let rep = cycle_representative(&gid, &orphans, &sources);
        assert!(rep.is_none());
    }

    #[test]
    fn cycle_representative_picks_min() {
        // Pure cycle A <-> B, no sources, should pick min UUID
        let gid = make_gid(&[(1, 100, 2), (2, 100, 1)]);
        let orphans: HashSet<Id> = [1, 2].into_iter().map(uuid).collect();
        let sources: Vec<Id> = vec![];
        let rep = cycle_representative(&gid, &orphans, &sources);
        assert_eq!(rep, Some(uuid(1)));
    }

    #[test]
    fn orphan_roots_no_orphans() {
        let gid = make_gid(&[(1, 100, 2)]);
        let doc = Document { gid, roots: vec![RootSlot::new(uuid(1))] };
        assert!(doc.orphan_roots().is_empty());
    }

    #[test]
    fn orphan_roots_single_orphan() {
        let mut gid = make_gid(&[(1, 100, 2)]);
        gid.set(uuid(3), uuid(100), uuid(4)); // orphan island
        let doc = Document { gid, roots: vec![RootSlot::new(uuid(1))] };
        let orphans = doc.orphan_roots();
        assert!(orphans.contains(&uuid(3)));
    }

    #[test]
    fn orphan_roots_cycle() {
        // Root: 1. Orphan cycle: 2 <-> 3
        let mut gid = make_gid(&[(2, 100, 3), (3, 100, 2)]);
        gid.set(uuid(1), uuid(100), uuid(1)); // self-loop so 1 is an entity
        let doc = Document { gid, roots: vec![RootSlot::new(uuid(1))] };
        let orphans = doc.orphan_roots();
        // Should pick min UUID from cycle as representative
        assert_eq!(orphans.len(), 1);
        assert!(orphans.contains(&uuid(2)));
    }
}
