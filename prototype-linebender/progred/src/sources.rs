//! The reading context: a document read over its library. Mutation
//! targets the document and gates on cell authority; presentation and
//! resolution read through both sides. Fallback is per cell value:
//! the document's value wins whole, otherwise the library answers.

use crate::document::Document;
use progred_graph::{CellId, Cells, Step, Value};

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub library: &'a Cells,
}

impl<'a> Sources<'a> {
    pub fn value(&self, cell: CellId) -> Option<&'a Value> {
        self.doc
            .cells
            .value(cell)
            .or_else(|| self.library.value(cell))
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path`, following links and descending through
    /// ordinary record and list structure. Writes gate separately on
    /// the cell owning the path's last Follow.
    pub fn resolve(&self, path: &[Step]) -> Option<&'a Value> {
        path.iter()
            .try_fold(self.root()?, |value, step| match step {
                Step::Follow => self.value(value.as_cell()?),
                Step::Key(label) => value.as_record()?.get(label),
                Step::Element(position) => value.as_list()?.get(position),
            })
    }

    pub fn cells(&self) -> impl Iterator<Item = &'a CellId> {
        self.doc.cells.cells().chain(self.library.cells())
    }

    /// The library is authoritative only when it supplies the value
    /// and the document does not. A bare cell remains writable.
    pub fn external(&self, cell: CellId) -> bool {
        self.doc.cells.value(cell).is_none() && self.library.value(cell).is_some()
    }

    pub fn writable(&self, cell: CellId) -> bool {
        !self.external(cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    fn doc_of(cells: Cells) -> Document {
        Document { root: None, cells }
    }

    #[test]
    fn the_document_shadows_the_library_per_cell() {
        let cell = new_cell_id();
        let mut library = Cells::new();
        library.set_value(
            cell,
            Value::record([
                progred_name::field("lib-name"),
                (
                    crate::test_values::label("a"),
                    crate::test_values::text("1"),
                ),
            ]),
        );

        let doc = doc_of(Cells::new());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(
            sources.value(cell).and_then(progred_name::read),
            Some("lib-name")
        );

        let mut cells = Cells::new();
        cells.set_value(cell, progred_name::record("mine", []));
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert_eq!(
            sources.value(cell).and_then(progred_name::read),
            Some("mine")
        );
        assert_eq!(
            sources
                .value(cell)
                .and_then(Value::as_record)
                .and_then(|fields| fields.get(&crate::test_values::label("a"))),
            None
        );
    }

    #[test]
    fn external_means_the_library_answers() {
        let lib_cell = new_cell_id();
        let doc_cell = new_cell_id();
        let bare = new_cell_id();
        let mut library = Cells::new();
        library.set_value(lib_cell, crate::test_values::text("lib"));
        let mut cells = Cells::new();
        cells.set_value(doc_cell, crate::test_values::text("doc"));

        let doc = doc_of(cells.clone());
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(sources.external(lib_cell));
        assert!(!sources.writable(lib_cell));
        assert!(!sources.external(doc_cell));
        assert!(!sources.external(bare));
        assert!(sources.writable(bare));

        cells.set_value(lib_cell, crate::test_values::text("mine"));
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        assert!(!sources.external(lib_cell));
        assert!(sources.writable(lib_cell));
    }

    #[test]
    fn resolve_follows_links_keys_and_elements() {
        let mut cells = Cells::new();
        let root = new_cell_id();
        cells.set_value(
            root,
            Value::record([
                progred_name::field("scene"),
                (
                    crate::test_values::label("items"),
                    Value::list([
                        crate::test_values::text("one"),
                        Value::record([(
                            crate::test_values::label("x"),
                            crate::test_values::text("deep"),
                        )]),
                    ]),
                ),
            ]),
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

        assert_eq!(sources.resolve(&[]), Some(&Value::from(root)));
        assert_eq!(
            sources.resolve(&[
                Step::Follow,
                Step::Key(progred_name::vocabulary::NAME),
            ]),
            Some(&crate::test_values::text("scene"))
        );
        let items = [Step::Follow, Step::Key(crate::test_values::label("items"))];
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
            Step::Key(crate::test_values::label("items")),
            Step::Element(positions[1].clone()),
            Step::Key(crate::test_values::label("x")),
        ];
        assert_eq!(
            sources.resolve(&deep),
            Some(&crate::test_values::text("deep"))
        );

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
                Step::Key(crate::test_values::label("items")),
                Step::Element(gone),
            ]),
            None
        );
    }
}
