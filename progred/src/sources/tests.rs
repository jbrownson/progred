use super::*;
use crate::libraries::name;
use gid::{Cells, new_cell_id};

fn libraries(id: CellId, cells: Cells) -> Libraries {
    Libraries::from_contributions([(
        id,
        crate::test_support::library(id, "test", cells, grap::ForeignFunctions::default()),
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
        crate::library::Library::<(), ()>::new(crate::library::Definitions::from_parts(
            Cells::new(),
            grap::ForeignFunctions::default().register(
                function,
                grap::ForeignFunction::from_value(|_, _, _| {
                    panic!("a document definition must shadow the library call")
                }),
            ),
        )),
    )]);
    let empty = doc_of(Cells::new());
    let native = Sources {
        doc: &empty,
        libraries: &libraries,
    }
    .resolve(function)
    .unwrap();
    assert_eq!(native.resolution, Resolution::Library(library));
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
        let resolved = sources.resolve(function).unwrap();
        assert_eq!(resolved.resolution, Resolution::Document);
        assert!(!resolved.native);
        assert_eq!(Some(resolved.value), doc.cells.value(function));
        for result in [
            grap::evaluate_value(&grap::call(function.into(), []), &sources, 100),
            grap::apply_value(&function.into(), [], &sources, None, 100),
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
    let definition = |value| grap::lambda([], grap::call(Value::from(probe), [(result, value)]));
    let mut cells = Cells::new();
    cells.set_value(function, definition(::grap::absent::decline()));
    let doc = doc_of(cells);
    for order in [library_ids, [library_ids[1], library_ids[0]]] {
        let libraries = Libraries::from_contributions(order.map(|id| {
            let mut cells = Cells::new();
            cells.set_value(function, definition(Value::from(id)));
            (
                id,
                crate::test_support::library(
                    id,
                    "source",
                    cells,
                    grap::ForeignFunctions::default(),
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
                    .push(context.source_origin(call).unwrap());
                Ok(context.value(&context.field(call, result).unwrap()).clone())
            };
            let overlay = grap::ForeignOverlay::from_value(&functions, &observe);
            let evaluation = if host_apply {
                grap::apply_value(&Value::from(function), [], &sources, Some(&overlay), 100)
            } else {
                grap::evaluate_runtime_scoped(
                    &grap::RuntimeValue::from(grap::call(Value::from(function), [])),
                    &sources,
                    &overlay,
                    100,
                )
                .into_value()
            };
            assert_eq!(evaluation.result, ::grap::absent::decline());
            assert_eq!(
                origins.into_inner(),
                [Resolution::Document].map(|resolution| {
                    grap::SourceOrigin::Cell {
                        cell: function,
                        resolution,
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
                crate::libraries::text::value("1"),
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
        crate::libraries::name::of(&sources, cell),
        Some(crate::display::Name::Plain("mine"))
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
            .map(|definition| definition.resolution)
            .collect::<Vec<_>>(),
        [Resolution::Document, Resolution::Library(library_id)]
    );
    assert_eq!(
        crate::libraries::name::of(&sources, library_id),
        Some(crate::display::Name::Plain("test"))
    );
}

#[test]
fn follow_names_a_library_source_not_its_load_order() {
    let cell = new_cell_id();
    let left_id = new_cell_id();
    let right_id = new_cell_id();
    let library = |id, name| {
        let mut cells = Cells::new();
        cells.set_value(cell, crate::libraries::text::value(name));
        crate::test_support::library(id, name, cells, grap::ForeignFunctions::default())
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
            crate::libraries::text::value(first),
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
    library_cells.set_value(lib_cell, crate::libraries::text::value("lib"));
    let libraries = libraries(new_cell_id(), library_cells);
    let mut cells = Cells::new();
    cells.set_value(doc_cell, crate::libraries::text::value("doc"));

    let owned = |cell, resolution| crate::location::owned(Start::Cell { cell, resolution });
    let library = Resolution::Library(libraries.iter().next().unwrap().0);
    let doc = doc_of(cells.clone());
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    assert!(sources.external(lib_cell));
    assert!(!owned(lib_cell, library));
    assert!(!sources.external(doc_cell));
    assert!(!sources.external(bare));
    assert!(owned(bare, Resolution::Document));

    cells.set_value(lib_cell, crate::libraries::text::value("mine"));
    let doc = doc_of(cells);
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    assert!(!sources.external(lib_cell));
    assert!(owned(lib_cell, Resolution::Document));
    assert!(!owned(lib_cell, library));
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
                    crate::libraries::text::value("one"),
                    Value::record([(
                        crate::test_values::label("x"),
                        crate::libraries::text::value("deep"),
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
        Some(&crate::libraries::text::value("scene"))
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
        Some(&crate::libraries::text::value("deep"))
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
