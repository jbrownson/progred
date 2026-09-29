//! The reading context: a document followed by an ordered set of
//! libraries. Ordinary lookup selects the document, then the first
//! library definition. A stored Follow step still names its source.

use crate::libraries::{Libraries, name};
use gid::{CellId, Document, Resolution, Step, Value};

pub type DefinitionSource = Resolution;

#[derive(Clone, Copy)]
pub struct LocatedValue<'a> {
    pub source: DefinitionSource,
    pub value: &'a Value,
}

/// One source's definition of a cell.
#[derive(Clone, Copy)]
enum Contribution<'a> {
    Document(&'a Value),
    Library(&'a grap::Definition),
}

impl<'a> Contribution<'a> {
    fn value(self) -> &'a Value {
        match self {
            Self::Document(value) => value,
            Self::Library(definition) => definition.value(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub doc: &'a Document,
    pub libraries: &'a Libraries,
}

impl crate::display::Env for Sources<'_> {
    fn apply_expression_runtime(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
    ) -> grap::RuntimeValue {
        grap::apply_expression(
            function,
            arguments.iter().cloned(),
            self,
            grap::DEFAULT_FUEL,
        )
        .result
    }
    fn evaluate_runtime(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        _steps: &[Step],
    ) -> grap::RuntimeValue {
        grap::evaluate_runtime_at(
            expression,
            Some(grap::SourceOrigin::Input(vec![])),
            self,
            fuel,
        )
        .result
    }
    fn evaluate_runtime_memo(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[Step],
    ) -> grap::RuntimeValue {
        self.evaluate_runtime(expression, fuel, steps)
    }
    fn apply_runtime_scoped(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation {
        match scope {
            Some(scope) => grap::apply_scoped(
                function,
                arguments.iter().cloned(),
                self,
                scope,
                grap::DEFAULT_FUEL,
            ),
            None => grap::apply(
                function,
                arguments.iter().cloned(),
                self,
                grap::DEFAULT_FUEL,
            ),
        }
    }
    fn apply_runtime_memo(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue {
        grap::apply_expression(
            &function.into(),
            arguments.iter().map(|(k, v)| (*k, v.into())),
            self,
            fuel,
        )
        .result
    }

    fn apply_scoped(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation<gid::Value> {
        match scope {
            Some(scope) => grap::apply_value_scoped(
                function,
                arguments.iter().cloned(),
                self,
                scope,
                grap::DEFAULT_FUEL,
            ),
            None => grap::apply_value(
                function,
                arguments.iter().cloned(),
                self,
                grap::DEFAULT_FUEL,
            ),
        }
    }

    fn evaluate(&self, expression: &Value) -> Value {
        self.evaluate_with_fuel(expression, grap::DEFAULT_FUEL)
    }

    fn evaluate_with_fuel(&self, expression: &Value, fuel: usize) -> Value {
        grap::evaluate_value(expression, self, fuel).result
    }

    fn apply_memo(&self, function: &Value, arguments: &[(CellId, Value)], fuel: usize) -> Value {
        grap::apply_value(function, arguments.iter().cloned(), self, fuel).result
    }

    fn name(&self, cell: CellId) -> Option<&str> {
        Sources::name(self, cell)
    }

    fn resolve(&self, cell: CellId) -> Option<crate::display::ResolvedCell<'_>> {
        self.definition(cell)
    }
}

impl<'a> Sources<'a> {
    /// Every definition of `cell` in lookup order: the document's, then each
    /// library's in load order. Ordinary lookup takes the first.
    fn definitions(&self, cell: CellId) -> impl Iterator<Item = (Resolution, Contribution<'a>)> {
        let libraries = self.libraries;
        self.doc
            .cells
            .value(cell)
            .map(|value| (Resolution::Document, Contribution::Document(value)))
            .into_iter()
            .chain(libraries.definitions(cell).map(|(library, definition)| {
                (
                    Resolution::Library(library),
                    Contribution::Library(definition),
                )
            }))
    }

    pub fn definition(&self, cell: CellId) -> Option<crate::display::ResolvedCell<'a>> {
        self.definitions(cell)
            .next()
            .map(|(source, contribution)| crate::display::ResolvedCell {
                source,
                value: contribution.value(),
                native: matches!(
                    contribution,
                    Contribution::Library(grap::Definition::Foreign(_))
                ),
            })
    }

    pub fn value(&self, cell: CellId, resolution: &Resolution) -> Option<&'a Value> {
        self.values(cell)
            .find(|value| &value.source == resolution)
            .map(|value| value.value)
    }

    pub fn name(&self, cell: CellId) -> Option<&'a str> {
        self.resolve(cell).and_then(|value| name::read(value.value))
    }

    pub fn contributors(&self, cell: CellId) -> impl Iterator<Item = Resolution> + '_ {
        self.definitions(cell).map(|(source, _)| source)
    }

    pub fn values(&self, cell: CellId) -> impl Iterator<Item = LocatedValue<'a>> {
        self.definitions(cell)
            .map(|(source, contribution)| LocatedValue {
                source,
                value: contribution.value(),
            })
    }

    pub fn resolve(&self, cell: CellId) -> Option<LocatedValue<'a>> {
        self.values(cell).next()
    }

    pub fn root(&self) -> Option<&'a Value> {
        self.doc.root.as_ref()
    }

    /// The value at `path`, following links and descending through
    /// ordinary record and list structure. Writes gate separately on
    /// the cell owning the path's last Follow.
    pub fn resolve_path(&self, path: &[Step]) -> Option<&'a Value> {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::SourceLookup);
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
        matches!(
            self.definitions(cell).next(),
            Some((Resolution::Library(_), _))
        )
    }
}

/// Only the document's own definitions are editable; libraries are read-only.
pub fn writable(resolution: &Resolution) -> bool {
    matches!(resolution, Resolution::Document)
}

impl grap::Host for Sources<'_> {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, grap::Definition)> {
        self.definitions(cell).next().map(|(source, contribution)| {
            (
                source,
                match contribution {
                    Contribution::Document(value) => grap::Definition::Value(value.clone()),
                    Contribution::Library(definition) => definition.clone(),
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, new_cell_id};

    fn libraries(id: CellId, cells: Cells) -> Libraries {
        Libraries::from_contributions([(
            id,
            crate::libraries::Library::<(), ()>::named(
                id,
                "test",
                crate::libraries::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
                crate::display::partial(|_| None),
            ),
        )])
        .0
    }

    fn doc_of(cells: Cells) -> Document {
        Document { root: None, cells }
    }

    #[test]
    fn document_definitions_shadow_library_calls_including_non_callable_values() {
        let function = new_cell_id();
        let library = new_cell_id();
        let (libraries, _, _) = Libraries::from_contributions([(
            library,
            crate::libraries::Library::<(), ()>::new(
                crate::libraries::Definitions::from_parts(
                    Cells::new(),
                    grap::ForeignFunctions::default().register(
                        function,
                        grap::ForeignFunction::from_value(|_, _, _| {
                            panic!("a document definition must shadow the library call")
                        }),
                    ),
                ),
                crate::display::partial(|_| None),
            ),
        )]);
        let empty = doc_of(Cells::new());
        let native = Sources {
            doc: &empty,
            libraries: &libraries,
        }
        .definition(function)
        .unwrap();
        assert_eq!(native.source, Resolution::Library(library));
        assert!(native.native);
        assert_eq!(native.value, &Value::record([]));
        let answer = Value::from(b"document".to_vec());
        for (definition, expected) in [
            (grap::lambda([], answer.clone()), answer.clone()),
            (
                answer.clone(),
                grap::absent::with_detail(grap::absent::NOT_CALLABLE, grap::absent::VALUE, answer),
            ),
            (
                grap::lambda([], grap::absent::decline()),
                grap::absent::decline(),
            ),
        ] {
            let mut cells = Cells::new();
            cells.set_value(function, definition);
            let doc = doc_of(cells);
            let sources = Sources {
                doc: &doc,
                libraries: &libraries,
            };
            let resolved = sources.definition(function).unwrap();
            assert_eq!(resolved.source, Resolution::Document);
            assert!(!resolved.native);
            assert_eq!(Some(resolved.value), doc.cells.value(function));
            for result in [
                grap::evaluate_value(&grap::call(function.into(), []), &sources, 100),
                grap::apply_value(&function.into(), [], &sources, 100),
            ] {
                assert!(result.completed);
                assert_eq!(result.result, expected);
            }
        }
    }

    #[test]
    fn grap_origins_follow_the_selected_document_definition_even_when_it_declines() {
        let function = new_cell_id();
        let probe = new_cell_id();
        let result = new_cell_id();
        let library_ids = [new_cell_id(), new_cell_id()];
        let definition =
            |value| grap::lambda([], grap::call(Value::from(probe), [(result, value)]));
        let mut cells = Cells::new();
        cells.set_value(function, definition(crate::libraries::absent::decline()));
        let doc = doc_of(cells);
        for order in [library_ids, [library_ids[1], library_ids[0]]] {
            let libraries = Libraries::from_contributions(order.map(|id| {
                let mut cells = Cells::new();
                cells.set_value(function, definition(Value::from(id)));
                (
                    id,
                    crate::libraries::Library::<(), ()>::named(
                        id,
                        "source",
                        crate::libraries::Definitions::from_parts(
                            cells,
                            grap::ForeignFunctions::default(),
                        ),
                        crate::display::partial(|_| None),
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
                let observe = |_,
                               context: &mut grap::Context<'_>,
                               call: &grap::Expression,
                               _: &grap::Environment| {
                    origins
                        .borrow_mut()
                        .push(context.source_origin(&call).unwrap());
                    Ok(context.value(&context.field(call, result).unwrap()).clone())
                };
                let overlay = grap::ForeignOverlay::from_value(&functions, &observe);
                let evaluation = if host_apply {
                    grap::apply_value_scoped(&Value::from(function), [], &sources, &overlay, 100)
                } else {
                    grap::evaluate_value_scoped(
                        &grap::call(Value::from(function), []),
                        &sources,
                        &overlay,
                        100,
                    )
                };
                assert_eq!(evaluation.result, crate::libraries::absent::decline());
                assert_eq!(
                    origins.into_inner(),
                    [Resolution::Document].map(|source| {
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
        assert_eq!(sources.name(cell), Some("mine"));
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
        assert_eq!(sources.name(library_id), Some("test"));
    }

    #[test]
    fn follow_names_a_library_source_not_its_load_order() {
        let cell = new_cell_id();
        let left_id = new_cell_id();
        let right_id = new_cell_id();
        let library = |id, name| {
            let mut cells = Cells::new();
            cells.set_value(cell, crate::test_values::text(name));
            crate::libraries::Library::<(), ()>::named(
                id,
                name,
                crate::libraries::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
                crate::display::partial(|_| None),
            )
        };
        let doc = Document {
            root: Some(Value::from(cell)),
            cells: Cells::new(),
        };
        let left_then_right = Libraries::from_contributions([
            (left_id, library(left_id, "left")),
            (right_id, library(right_id, "right")),
        ])
        .0;
        let right_then_left = Libraries::from_contributions([
            (right_id, library(right_id, "right")),
            (left_id, library(left_id, "left")),
        ])
        .0;
        let path = [Step::Follow(Resolution::Library(right_id))];

        for (libraries, first) in [(&left_then_right, "left"), (&right_then_left, "right")] {
            let sources = Sources {
                doc: &doc,
                libraries,
            };
            assert_eq!(
                sources
                    .resolve(cell)
                    .and_then(|value| crate::libraries::text::read(value.value)),
                Some(first),
            );
            assert_eq!(
                grap::evaluate_value(&cell.into(), &sources, 100).result,
                crate::test_values::text(first),
            );
        }

        assert_eq!(
            Sources {
                doc: &doc,
                libraries: &left_then_right,
            }
            .resolve_path(&path)
            .and_then(crate::libraries::text::read),
            Some("right")
        );
        assert_eq!(
            Sources {
                doc: &doc,
                libraries: &right_then_left,
            }
            .resolve_path(&path)
            .and_then(crate::libraries::text::read),
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
        assert!(!writable(&Resolution::Library(
            libraries.iter().next().unwrap().0
        )));
        assert!(!sources.external(doc_cell));
        assert!(!sources.external(bare));
        assert!(writable(&Resolution::Document));

        cells.set_value(lib_cell, crate::test_values::text("mine"));
        let doc = doc_of(cells);
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        assert!(!sources.external(lib_cell));
        assert!(writable(&Resolution::Document));
        assert!(!writable(&Resolution::Library(
            libraries.iter().next().unwrap().0
        )));
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
