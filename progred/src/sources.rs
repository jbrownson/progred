//! The reading context: a document followed by an ordered set of
//! libraries. Resolution exposes every contributed definition with
//! its source. A stored Follow step names the stable source of the
//! value definition it crosses into.

use gid::{CellId, Document, Resolution, Step, Value};
use progred_libraries::{Libraries, name};

pub type DefinitionSource = Resolution;

#[derive(Clone, Copy)]
pub struct LocatedValue<'a> {
    pub source: DefinitionSource,
    pub value: &'a Value,
}

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub libraries: &'a Libraries,
}

impl<'a> Sources<'a> {
    pub fn value(&self, cell: CellId, resolution: &Resolution) -> Option<&'a Value> {
        self.values(cell)
            .find(|value| &value.source == resolution)
            .map(|value| value.value)
    }

    pub fn names(&self, cell: CellId) -> impl Iterator<Item = &'a str> {
        self.values(cell)
            .filter_map(|value| name::read(value.value))
    }

    pub fn display_names(&self, cell: CellId) -> Option<String> {
        let names: Vec<_> = self.names(cell).collect();
        (!names.is_empty()).then(|| names.join(" / "))
    }

    pub fn library_name(&self, library: CellId) -> Option<&'a str> {
        self.libraries.metadata(library).and_then(name::read)
    }

    pub fn contributors(&self, cell: CellId) -> impl Iterator<Item = Resolution> + '_ {
        self.doc
            .cells
            .value(cell)
            .map(|_| Resolution::Document)
            .into_iter()
            .chain(self.libraries.contributors(cell).map(Resolution::Library))
    }

    pub fn values(&self, cell: CellId) -> impl Iterator<Item = LocatedValue<'a>> {
        self.doc
            .cells
            .value(cell)
            .map(|value| LocatedValue {
                source: Resolution::Document,
                value,
            })
            .into_iter()
            .chain(self.libraries.values(cell).map(|value| LocatedValue {
                source: Resolution::Library(value.library),
                value: value.value,
            }))
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path`, following links and descending through
    /// ordinary record and list structure. Writes gate separately on
    /// the cell owning the path's last Follow.
    pub fn resolve_path(&self, path: &[Step]) -> Option<&'a Value> {
        path.iter()
            .try_fold(self.root()?, |value, step| match step {
                Step::Follow(resolution) => self.value(value.as_cell()?, resolution),
                Step::Key(label) => value.as_record()?.get(label),
                Step::Element(position) => value.as_list()?.get(position),
            })
    }

    pub fn cells(&self) -> impl Iterator<Item = CellId> + '_ {
        let mut cells: Vec<_> = self
            .doc
            .cells
            .cells()
            .copied()
            .chain(self.libraries.cell_ids())
            .collect();
        cells.sort_unstable();
        cells.dedup();
        cells.into_iter()
    }

    /// The library is authoritative only when it supplies the value
    /// and the document does not. A bare cell remains writable.
    pub fn external(&self, cell: CellId) -> bool {
        self.doc.cells.value(cell).is_none() && self.libraries.values(cell).next().is_some()
    }

    pub fn writable(&self, _cell: CellId, resolution: &Resolution) -> bool {
        matches!(resolution, Resolution::Document)
    }
}

impl grap::Host for Sources<'_> {
    fn values(&self, cell: CellId) -> Vec<(Resolution, Value)> {
        self.values(cell)
            .map(|value| (value.source, value.value.clone()))
            .collect()
    }

    fn candidates(&self, cell: CellId) -> Vec<(Resolution, grap::CallCandidate)> {
        self.doc
            .cells
            .value(cell)
            .cloned()
            .map(|value| (Resolution::Document, grap::CallCandidate::Value(value)))
            .into_iter()
            .chain(self.libraries.call_candidates(cell))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, new_cell_id};

    fn libraries(id: CellId, cells: Cells) -> Libraries {
        Libraries::from_contributions([(
            id,
            progred_libraries::Library::<(), ()>::named(
                "test",
                progred_libraries::Definitions::from_parts(
                    cells,
                    grap::ForeignFunctions::default(),
                ),
                vec![],
            ),
        )])
        .0
    }

    fn doc_of(cells: Cells) -> Document {
        Document { root: None, cells }
    }

    #[test]
    fn grap_origins_follow_the_definition_that_executes_after_fallthrough() {
        let function = new_cell_id();
        let probe = new_cell_id();
        let result = new_cell_id();
        let library_ids = [new_cell_id(), new_cell_id()];
        let definition =
            |value| grap::lambda([], grap::call(Value::from(probe), [(result, value)]));
        let mut cells = Cells::new();
        cells.set_value(function, definition(progred_libraries::absent::decline()));
        let doc = doc_of(cells);
        for order in [library_ids, [library_ids[1], library_ids[0]]] {
            let libraries = Libraries::from_contributions(order.map(|id| {
                let mut cells = Cells::new();
                cells.set_value(function, definition(Value::from(id)));
                (
                    id,
                    progred_libraries::Library::<(), ()>::named(
                        "source",
                        progred_libraries::Definitions::from_parts(
                            cells,
                            grap::ForeignFunctions::default(),
                        ),
                        vec![],
                    ),
                )
            }))
            .0;
            let sources = Sources {
                doc: &doc,
                libraries: &libraries,
            };
            for host_apply in [false, true] {
                let origins = std::cell::RefCell::new(Vec::new());
                let functions = [probe];
                let observe = |_, context: &mut grap::Context<'_>, call, _: &grap::Environment| {
                    origins
                        .borrow_mut()
                        .push(context.source_origin(call).unwrap());
                    Ok(context.value(context.field(call, result).unwrap()).clone())
                };
                let overlay = grap::ForeignOverlay::new(&functions, &observe);
                let evaluation = if host_apply {
                    grap::apply_scoped(&Value::from(function), [], &sources, &overlay, 100)
                } else {
                    grap::evaluate_scoped(
                        &grap::call(Value::from(function), []),
                        &sources,
                        &overlay,
                        100,
                    )
                };
                assert_eq!(evaluation.result, Value::from(order[0]));
                assert_eq!(
                    origins.into_inner(),
                    [Resolution::Document, Resolution::Library(order[0])].map(|source| {
                        grap::SourceOrigin::Cell {
                            cell: function,
                            source,
                            path: vec![Step::Key(grap::vocabulary::BODY)],
                        }
                    })
                );
            }
        }
    }

    #[test]
    fn values_are_resolved_by_their_stable_sources() {
        let cell = new_cell_id();
        let library_id = new_cell_id();
        let mut library_cells = Cells::new();
        library_cells.set_value(
            cell,
            Value::record([
                name::field("lib-name"),
                (
                    crate::test_values::label("a"),
                    crate::test_values::text("1"),
                ),
            ]),
        );
        let libraries = libraries(library_id, library_cells);

        let doc = doc_of(Cells::new());
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        assert_eq!(
            sources
                .value(cell, &Resolution::Library(library_id))
                .and_then(name::read),
            Some("lib-name")
        );

        let mut cells = Cells::new();
        cells.set_value(cell, name::record("mine", []));
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        assert_eq!(
            sources
                .value(cell, &Resolution::Document)
                .and_then(name::read),
            Some("mine")
        );
        assert_eq!(
            sources.names(cell).collect::<Vec<_>>(),
            ["mine", "lib-name"]
        );
        assert_eq!(
            sources
                .value(cell, &Resolution::Document)
                .and_then(Value::as_record)
                .and_then(|fields| fields.get(&crate::test_values::label("a"))),
            None
        );
    }

    #[test]
    fn plural_lookup_keeps_document_first_and_names_library_origins() {
        let cell = new_cell_id();
        let library_id = new_cell_id();
        let mut document_cells = Cells::new();
        document_cells.set_value(cell, name::record("document", []));
        let mut library_cells = Cells::new();
        library_cells.set_value(cell, name::record("library", []));
        let libraries = libraries(library_id, library_cells);
        let doc = doc_of(document_cells);
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };

        assert_eq!(
            sources
                .values(cell)
                .map(|definition| definition.source)
                .collect::<Vec<_>>(),
            [
                DefinitionSource::Document,
                DefinitionSource::Library(library_id)
            ]
        );
        assert_eq!(sources.library_name(library_id), Some("test"));
    }

    #[test]
    fn follow_names_a_library_source_not_its_load_order() {
        let cell = new_cell_id();
        let left_id = new_cell_id();
        let right_id = new_cell_id();
        let library = |name| {
            let mut cells = Cells::new();
            cells.set_value(cell, crate::test_values::text(name));
            progred_libraries::Library::<(), ()>::named(
                name,
                progred_libraries::Definitions::from_parts(
                    cells,
                    grap::ForeignFunctions::default(),
                ),
                vec![],
            )
        };
        let doc = Document {
            root: Some(Value::from(cell)),
            cells: Cells::new(),
        };
        let left_then_right = Libraries::from_contributions([
            (left_id, library("left")),
            (right_id, library("right")),
        ])
        .0;
        let right_then_left = Libraries::from_contributions([
            (right_id, library("right")),
            (left_id, library("left")),
        ])
        .0;
        let path = [Step::Follow(Resolution::Library(right_id))];

        assert_eq!(
            Sources {
                doc: &doc,
                libraries: &left_then_right,
            }
            .resolve_path(&path)
            .and_then(progred_libraries::text::read),
            Some("right")
        );
        assert_eq!(
            Sources {
                doc: &doc,
                libraries: &right_then_left,
            }
            .resolve_path(&path)
            .and_then(progred_libraries::text::read),
            Some("right")
        );
    }

    #[test]
    fn external_means_the_library_answers() {
        let lib_cell = new_cell_id();
        let doc_cell = new_cell_id();
        let bare = new_cell_id();
        let mut library_cells = Cells::new();
        library_cells.set_value(lib_cell, crate::test_values::text("lib"));
        let libraries = libraries(new_cell_id(), library_cells);
        let mut cells = Cells::new();
        cells.set_value(doc_cell, crate::test_values::text("doc"));

        let doc = doc_of(cells.clone());
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        assert!(sources.external(lib_cell));
        assert!(!sources.writable(
            lib_cell,
            &Resolution::Library(libraries.iter().next().unwrap().0)
        ));
        assert!(!sources.external(doc_cell));
        assert!(!sources.external(bare));
        assert!(sources.writable(bare, &Resolution::Document));

        cells.set_value(lib_cell, crate::test_values::text("mine"));
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        assert!(!sources.external(lib_cell));
        assert!(sources.writable(lib_cell, &Resolution::Document));
        assert!(!sources.writable(
            lib_cell,
            &Resolution::Library(libraries.iter().next().unwrap().0)
        ));
    }

    #[test]
    fn resolve_follows_links_keys_and_elements() {
        let mut cells = Cells::new();
        let root = new_cell_id();
        cells.set_value(
            root,
            Value::record([
                name::field("scene"),
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
        let libraries = Libraries::default();
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };

        assert_eq!(sources.resolve_path(&[]), Some(&Value::from(root)));
        assert_eq!(
            sources.resolve_path(&[
                Step::Follow(Resolution::Document),
                Step::Key(name::vocabulary::NAME),
            ]),
            Some(&crate::test_values::text("scene"))
        );
        let items = [
            Step::Follow(Resolution::Document),
            Step::Key(crate::test_values::label("items")),
        ];
        let positions: Vec<_> = sources
            .resolve_path(&items)
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let deep = [
            Step::Follow(Resolution::Document),
            Step::Key(crate::test_values::label("items")),
            Step::Element(positions[1].clone()),
            Step::Key(crate::test_values::label("x")),
        ];
        assert_eq!(
            sources.resolve_path(&deep),
            Some(&crate::test_values::text("deep"))
        );

        let bare_doc = Document {
            root: Some(Value::from(new_cell_id())),
            cells: Cells::new(),
        };
        let bare_sources = Sources {
            doc: &bare_doc,
            libraries: &libraries,
        };
        assert!(bare_sources.resolve_path(&[]).is_some());
        assert_eq!(
            bare_sources.resolve_path(&[Step::Follow(Resolution::Document)]),
            None
        );
        let gone = gid::position::between(Some(&positions[1]), None).unwrap();
        assert_eq!(
            sources.resolve_path(&[
                Step::Follow(Resolution::Document),
                Step::Key(crate::test_values::label("items")),
                Step::Element(gone),
            ]),
            None
        );
    }
}
