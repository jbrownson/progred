//! The reading context: a document read over its library.
//! Deliberately NOT a `Gid` — the two-ness is the point. Mutation
//! takes the document exclusively and gates on entity authority;
//! presentation and resolution read through both sides; provenance
//! is simply which side answered. Fallback is per ENTITY, never per
//! edge: an entity's edge-set is one authority's statement, and an
//! edge-level merge would invent a semantic join (inheritance) that
//! belongs above the data model if anywhere. Multiple libraries
//! compose UPSTREAM into the one library gid (they are read-only, so
//! composition is merging); `Sources` stays two-sided.

use crate::raw::Document;
use im::HashMap;
use progred_graph::{Atom, Gid, MutGid, NodeId, Step, Value};

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub library: &'a MutGid,
}

impl<'a> Sources<'a> {
    pub fn edges(&self, entity: NodeId) -> Option<&'a HashMap<Atom, Value>> {
        self.doc
            .gid
            .edges(entity)
            .or_else(|| self.library.edges(entity))
    }

    pub fn get(&self, entity: NodeId, key: &Atom) -> Option<&'a Value> {
        self.edges(entity)?.get(key)
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path`, following each step from the root — key
    /// steps through entities (both sides, so navigation reaches
    /// what presentation shows), element steps into list values.
    /// Writes gate separately, on entity authority.
    pub fn resolve(&self, path: &[Step]) -> Option<&'a Value> {
        path.iter().try_fold(self.root()?, |value, step| match step {
            Step::Key(key) => self.get(value.as_node()?, key),
            Step::Element(position) => value.as_list()?.get(position),
        })
    }

    /// Every entity either side describes; shadowed duplicates are
    /// the caller's to fold.
    pub fn entities(&self) -> impl Iterator<Item = &'a NodeId> {
        self.doc.gid.entities().chain(self.library.entities())
    }

    /// Whether the library is the authority for this entity: it
    /// describes it and the document does not. External facts render
    /// on their own ground and decline authoring; a document that
    /// takes the entity over (a fork — copy/paste's job) is the
    /// authority again.
    pub fn external(&self, entity: NodeId) -> bool {
        self.doc.gid.edges(entity).is_none() && self.library.edges(entity).is_some()
    }

    /// Whether writes may target the entity: the document is the
    /// authority, or nobody is (fresh nodes).
    pub fn writable(&self, entity: NodeId) -> bool {
        !self.external(entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_node_id;

    fn doc_of(gid: MutGid) -> Document {
        Document { root: None, gid }
    }

    #[test]
    fn the_document_shadows_the_library_per_entity() {
        let entity = new_node_id();
        let mut library = MutGid::new();
        library.set(entity, Atom::from("a"), Value::from(1.0));
        library.set(entity, Atom::from("b"), Value::from(2.0));

        let doc = doc_of(MutGid::new());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(sources.get(entity, &Atom::from("a")), Some(&Value::from(1.0)));

        // One document edge takes over the whole entity — no
        // edge-level merge, so the library's `b` is gone too.
        let mut gid = MutGid::new();
        gid.set(entity, Atom::from("a"), Value::from(9.0));
        let doc = doc_of(gid);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(sources.get(entity, &Atom::from("a")), Some(&Value::from(9.0)));
        assert_eq!(sources.get(entity, &Atom::from("b")), None);
    }

    #[test]
    fn external_means_the_library_answers() {
        let lib_entity = new_node_id();
        let doc_entity = new_node_id();
        let mut library = MutGid::new();
        library.set(lib_entity, Atom::from("a"), Value::from(1.0));
        let mut gid = MutGid::new();
        gid.set(doc_entity, Atom::from("a"), Value::from(1.0));

        let doc = doc_of(gid.clone());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(sources.external(lib_entity));
        assert!(!sources.writable(lib_entity));
        assert!(!sources.external(doc_entity));

        // A fork — the document taking the entity over — ends the
        // library's authority.
        gid.set(lib_entity, Atom::from("mine"), Value::from(2.0));
        let doc = doc_of(gid);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(!sources.external(lib_entity));
        assert!(sources.writable(lib_entity));
    }

    #[test]
    fn resolve_walks_keys_and_elements() {
        let mut gid = MutGid::new();
        let root = new_node_id();
        gid.set(
            root,
            Atom::from("items"),
            Value::list([Value::from(1.0), Value::from("two")]),
        );
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let library = MutGid::new();
        let sources = Sources {
            doc: &doc,
            library: &library,
        };

        let items = sources.resolve(&[Step::Key(Atom::from("items"))]).unwrap();
        let positions: Vec<_> = items.as_list().unwrap().keys().cloned().collect();
        let path = vec![
            Step::Key(Atom::from("items")),
            Step::Element(positions[1].clone()),
        ];
        assert_eq!(sources.resolve(&path), Some(&Value::from("two")));
        // A dangling element is the stale-path class: None, not a
        // panic.
        let gone = progred_graph::position::between(Some(&positions[1]), None).unwrap();
        assert_eq!(
            sources.resolve(&[Step::Key(Atom::from("items")), Step::Element(gone)]),
            None
        );
    }
}
