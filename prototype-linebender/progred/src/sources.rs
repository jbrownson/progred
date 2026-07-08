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
use progred_graph::{Gid, Id, MutGid, NodeId};

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub library: &'a MutGid,
}

impl<'a> Sources<'a> {
    pub fn edges(&self, entity: &Id) -> Option<&'a HashMap<Id, Id>> {
        self.doc
            .gid
            .edges(entity)
            .or_else(|| self.library.edges(entity))
    }

    pub fn get(&self, entity: &Id, label: &Id) -> Option<&'a Id> {
        self.edges(entity)?.get(label)
    }

    pub fn root(&self) -> Option<&'a Id> {
        self.doc.root.as_ref()
    }

    /// The value at `path`, following each label from the root —
    /// through both sides, so navigation reaches what presentation
    /// shows. Writes gate separately, on entity authority.
    pub fn resolve(&self, path: &[Id]) -> Option<&'a Id> {
        path.iter()
            .try_fold(self.root()?, |node, label| self.get(node, label))
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
    pub fn external(&self, entity: &Id) -> bool {
        self.doc.gid.edges(entity).is_none() && self.library.edges(entity).is_some()
    }

    /// Whether writes may target the entity: the document is the
    /// authority, or nobody is (fresh nodes).
    pub fn writable(&self, entity: &Id) -> bool {
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
        library.set(entity, Id::from("a"), Id::from(1.0));
        library.set(entity, Id::from("b"), Id::from(2.0));

        let doc = doc_of(MutGid::new());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(
            sources.get(&Id::from(entity), &Id::from("a")),
            Some(&Id::from(1.0))
        );

        // One document edge takes over the whole entity — no
        // edge-level merge, so the library's `b` is gone too.
        let mut gid = MutGid::new();
        gid.set(entity, Id::from("a"), Id::from(9.0));
        let doc = doc_of(gid);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(
            sources.get(&Id::from(entity), &Id::from("a")),
            Some(&Id::from(9.0))
        );
        assert_eq!(sources.get(&Id::from(entity), &Id::from("b")), None);
    }

    #[test]
    fn external_means_the_library_answers() {
        let lib_entity = new_node_id();
        let doc_entity = new_node_id();
        let mut library = MutGid::new();
        library.set(lib_entity, Id::from("a"), Id::from(1.0));
        let mut gid = MutGid::new();
        gid.set(doc_entity, Id::from("a"), Id::from(1.0));

        let doc = doc_of(gid.clone());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(sources.external(&Id::from(lib_entity)));
        assert!(!sources.writable(&Id::from(lib_entity)));
        assert!(!sources.external(&Id::from(doc_entity)));
        // Atoms have no edges anywhere; never external.
        assert!(!sources.external(&Id::from(1.0)));

        // A fork — the document taking the entity over — ends the
        // library's authority.
        gid.set(lib_entity, Id::from("mine"), Id::from(2.0));
        let doc = doc_of(gid);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(!sources.external(&Id::from(lib_entity)));
        assert!(sources.writable(&Id::from(lib_entity)));
    }
}
