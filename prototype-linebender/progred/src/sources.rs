//! The reading context: a document read over its library.
//! Mutation takes the document exclusively and gates on cell
//! authority; presentation and resolution read through both sides;
//! provenance is simply which side answered. Fallback is per CELL
//! ENTRY, never per part: an entry — name, value, or both — is one
//! authority's whole statement about the identity, and a part-level
//! merge would invent a semantic join that belongs above the data
//! model if anywhere. Multiple libraries compose UPSTREAM into the
//! one library table (they are read-only, so composition is
//! merging); `Sources` stays two-sided.

use crate::raw::Document;
use progred_graph::{Cell, CellId, Cells, Step, Value};

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub library: &'a Cells,
}

impl<'a> Sources<'a> {
    /// What is said about the cell: the document's entry, else the
    /// library's. `None` is a fully bare cell — referenced identity
    /// with nothing said yet.
    pub fn entry(&self, cell: CellId) -> Option<&'a Cell> {
        self.doc
            .cells
            .entry(cell)
            .or_else(|| self.library.entry(cell))
    }

    pub fn value(&self, cell: CellId) -> Option<&'a Value> {
        self.entry(cell)?.value()
    }

    pub fn name(&self, cell: CellId) -> Option<&'a str> {
        self.entry(cell)?.name()
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path`, following each step from the root: Follow
    /// looks the link's cell up (both sides, so navigation reaches
    /// what presentation shows), Key into a record, Element into a
    /// list. A Name step never resolves — names are identity
    /// metadata, not values; the editor's name arms read them
    /// directly. Writes gate separately, on cell authority.
    pub fn resolve(&self, path: &[Step]) -> Option<&'a Value> {
        path.iter().try_fold(self.root()?, |value, step| match step {
            Step::Follow => self.value(value.as_cell()?),
            Step::Key(label) => value.as_record()?.get(label),
            Step::Element(position) => value.as_list()?.get(position),
            Step::Name => None,
        })
    }

    /// Every cell either side has an entry for; shadowed duplicates
    /// are the caller's to fold.
    pub fn cells(&self) -> impl Iterator<Item = &'a CellId> {
        self.doc.cells.cells().chain(self.library.cells())
    }

    /// Whether the library is the authority for this cell: it has the
    /// entry and the document does not. External facts render on
    /// their own ground and decline authoring; a document that takes
    /// the cell over (a fork — copy/paste's job) is the authority
    /// again. Fully bare cells are nobody's statement and stay
    /// writable.
    pub fn external(&self, cell: CellId) -> bool {
        self.doc.cells.entry(cell).is_none() && self.library.entry(cell).is_some()
    }

    /// Whether writes may target the cell: the document is the
    /// authority, or nobody is (bare and fresh cells).
    pub fn writable(&self, cell: CellId) -> bool {
        !self.external(cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::{Label, new_cell_id};

    fn doc_of(cells: Cells) -> Document {
        Document { root: None, cells }
    }

    #[test]
    fn the_document_shadows_the_library_per_entry() {
        let cell = new_cell_id();
        let mut library = Cells::new();
        library.set_name(cell, "lib-name");
        library.set_value(
            cell,
            Value::record([(Label::from("a"), Value::from("1"))]),
        );

        let doc = doc_of(Cells::new());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(sources.name(cell), Some("lib-name"));
        assert_eq!(
            sources.value(cell).unwrap().as_record().unwrap().get(&Label::from("a")),
            Some(&Value::from("1"))
        );

        // The document's entry is the whole statement: its name-only
        // entry shadows the library's value too — no part-level
        // merge.
        let mut cells = Cells::new();
        cells.set_name(cell, "mine");
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(sources.name(cell), Some("mine"));
        assert_eq!(sources.value(cell), None);
    }

    #[test]
    fn external_means_the_library_answers() {
        let lib_cell = new_cell_id();
        let doc_cell = new_cell_id();
        let bare = new_cell_id();
        let mut library = Cells::new();
        library.set_value(lib_cell, Value::from("lib"));
        let mut cells = Cells::new();
        cells.set_value(doc_cell, Value::from("doc"));

        let doc = doc_of(cells.clone());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(sources.external(lib_cell));
        assert!(!sources.writable(lib_cell));
        assert!(!sources.external(doc_cell));
        // A bare cell is nobody's and stays writable.
        assert!(!sources.external(bare));
        assert!(sources.writable(bare));

        // A fork — the document taking the cell over — ends the
        // library's authority.
        cells.set_value(lib_cell, Value::from("mine"));
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(!sources.external(lib_cell));
        assert!(sources.writable(lib_cell));
    }

    #[test]
    fn resolve_follows_links_keys_and_elements_never_names() {
        let mut cells = Cells::new();
        let root = new_cell_id();
        cells.set_name(root, "scene");
        cells.set_value(
            root,
            Value::record([(
                Label::from("items"),
                Value::list([
                    Value::from("one"),
                    Value::record([(Label::from("x"), Value::from("deep"))]),
                ]),
            )]),
        );
        let doc = Document {
            root: Some(Value::from(root)),
            cells,
        };
        let library = Cells::new();
        let sources = Sources {
            doc: &doc,
            library: &library,
        };

        // The empty path is the link; Follow is the cell's value.
        assert_eq!(sources.resolve(&[]), Some(&Value::from(root)));
        let items = [Step::Follow, Step::Key(Label::from("items"))];
        let positions: Vec<_> = sources
            .resolve(&items)
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let deep = [
            Step::Follow,
            Step::Key(Label::from("items")),
            Step::Element(positions[1].clone()),
            Step::Key(Label::from("x")),
        ];
        assert_eq!(sources.resolve(&deep), Some(&Value::from("deep")));

        // Names are not values: the Name step never resolves; the
        // name reads directly.
        assert_eq!(sources.resolve(&[Step::Name]), None);
        assert_eq!(sources.name(root), Some("scene"));

        // Follow on a fully bare cell, and a dangling element, are
        // the stale-path class: None, not a panic.
        let bare_doc = Document {
            root: Some(Value::from(new_cell_id())),
            cells: Cells::new(),
        };
        let bare_sources = Sources {
            doc: &bare_doc,
            library: &library,
        };
        assert!(bare_sources.resolve(&[]).is_some());
        assert_eq!(bare_sources.resolve(&[Step::Follow]), None);
        let gone = progred_graph::position::between(Some(&positions[1]), None).unwrap();
        assert_eq!(
            sources.resolve(&[
                Step::Follow,
                Step::Key(Label::from("items")),
                Step::Element(gone)
            ]),
            None
        );
    }
}
