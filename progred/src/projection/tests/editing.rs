use super::*;
use puri::edit::TextClipboard;

#[test]
fn a_reminted_line_uses_its_current_conversion_not_selection_wiring() {
    fn projection(marker: CellId) -> Projection<EditingWorld> {
        Projection::new([crate::display::partial(move |input| {
            let spelling = text::read(input.value?)?;
            Some(crate::libraries::line_edit::layout(
                spelling,
                crate::libraries::line_edit::native(move |spelling, _| {
                    Some(Value::record([
                        (
                            text::vocabulary::UTF8,
                            Value::from(spelling.as_bytes().to_vec()),
                        ),
                        (marker, Value::record([])),
                    ]))
                }),
                "",
                "",
            ))
        })])
    }
    let first = new_cell_id();
    let second = new_cell_id();
    let libraries = core_libraries();
    let mut world = editing_world(
        &Document {
            root: Some(text::value("a")),
            cells: Cells::new(),
        },
        &libraries,
    );
    world.model.selection = Some(make_selection(vec![]));
    for (marker, typed, expected) in [(first, "b", "ab"), (second, "c", "abc")] {
        let frame = editing_frame_with_projection(&mut world, false, Some(&projection(marker)));
        assert!(frame.resolve_for_dispatch().dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Character(typed.into()),
                ..arrow(NamedKey::End)
            }
        ));
        assert_eq!(
            text::read(world.model.doc.root.as_ref().unwrap()),
            Some(expected)
        );
        let fields = world.model.doc.root.as_ref().unwrap().as_record().unwrap();
        assert!(fields.contains_key(&marker));
        assert_eq!(fields.len(), 2);
    }
    let payload = world.model.selection.as_ref().unwrap().payload();
    let selection = Selection::from_scoped_payload(
        &crate::test_root(),
        &src(&world.model.doc, &libraries),
        Default::default(),
        vec![],
        payload.clone(),
    );
    assert_eq!(selection.payload(), payload);
    assert_eq!(selection.edit().unwrap().text(), "abc");
}

#[test]
fn caret_motion_does_not_run_a_line_conversion() {
    let calls = Rc::new(std::cell::Cell::new(0));
    let projection = Projection::new([crate::display::partial({
        let calls = calls.clone();
        move |input| {
            Some(crate::libraries::line_edit::layout(
                text::read(input.value?)?,
                crate::libraries::line_edit::native({
                    let calls = calls.clone();
                    move |spelling, current| {
                        calls.set(calls.get() + 1);
                        text::edit(spelling, current)
                    }
                }),
                "",
                "",
            ))
        }
    })]);
    let libraries = core_libraries();
    let mut world = editing_world(
        &Document {
            root: Some(text::value("abc")),
            cells: Cells::new(),
        },
        &libraries,
    );
    world.model.selection = Some(make_selection(vec![]));
    let original = world.model.doc.clone();
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        frame
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &arrow(NamedKey::Home))
    );
    assert_eq!(calls.get(), 0);
    assert!(Rc::ptr_eq(&world.model.doc, &original));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        frame
            .resolve_for_dispatch()
            .dispatch_ime(&mut world, &puri::handler::ImeEvent::Commit("x".into()))
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        text::read(world.model.doc.root.as_ref().unwrap()),
        Some("xabc")
    );
}

#[test]
fn line_projection_descriptions_mount_the_rust_editor() {
    let lib = core_libraries();
    let (mut doc, cell) = doc_of(vec![
        (
            crate::test_values::label("name"),
            crate::test_values::text("old"),
        ),
        (
            crate::test_values::label("x"),
            crate::test_values::text("1.5"),
        ),
    ]);
    let edit = |doc: &Document, path: Vec<Step>| make_projected_editing_selection(doc, &lib, path);
    assert_eq!(
        edit(
            &doc,
            vec![Step::Follow(gid::Resolution::Document), key("name")]
        )
        .edit()
        .map(LineEditState::text),
        Some("old")
    );
    assert_eq!(
        edit(
            &doc,
            vec![Step::Follow(gid::Resolution::Document), key("x")]
        )
        .edit()
        .map(LineEditState::text),
        Some("1.5")
    );
    // Missing fields and links carry no editor.
    assert!(
        make_selection(vec![
            Step::Follow(gid::Resolution::Document),
            key("missing")
        ])
        .edit()
        .is_none()
    );
    assert!(make_selection(vec![]).edit().is_none());
    Rc::make_mut(&mut doc).cells.set_value(
        cell,
        Value::record([(crate::test_values::label("b"), Value::from(vec![0xff_u8]))]),
    );
    assert_eq!(
        edit(
            &doc,
            vec![Step::Follow(gid::Resolution::Document), key("b")]
        )
        .edit()
        .map(LineEditState::text),
        Some("ff")
    );
    // A cell holding text edits at its Follow path.
    Rc::make_mut(&mut doc)
        .cells
        .set_value(cell, crate::test_values::text("held"));
    assert_eq!(
        edit(&doc, vec![Step::Follow(gid::Resolution::Document)])
            .edit()
            .map(LineEditState::text),
        Some("held")
    );
    // A simple name convention is just another text field.
    Rc::make_mut(&mut doc)
        .cells
        .set_value(cell, name::record("roof", []));
    assert_eq!(
        edit(
            &doc,
            vec![
                Step::Follow(gid::Resolution::Document),
                Step::Key(name::vocabulary::NAME),
            ],
        )
        .edit()
        .map(LineEditState::text),
        Some("roof")
    );
}

#[test]
fn a_line_control_selects_without_storing_its_default_editor() {
    let lib = core_libraries();
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("old"),
    )]);
    let selected = make_projected_selection(
        &doc,
        &lib,
        vec![Step::Follow(gid::Resolution::Document), key("name")],
    );
    assert!(selected.edit().is_none());
    assert_eq!(selected.payload(), selection_payload::edge());
}

#[test]
fn a_plain_selection_accepts_first_input_using_the_line_default() {
    let libraries = core_libraries();
    for (value, input, expected) in [
        (text::value("hë🦀"), "!", text::value("hë🦀!")),
        (text::value(""), "first", text::value("first")),
        (f64::value(12.0), "3", f64::value(123.0)),
        (Value::from(vec![0xab]), "cd", Value::from(vec![0xab, 0xcd])),
    ] {
        let doc = Document {
            root: Some(value),
            cells: Cells::new(),
        };
        let mut world = editing_world(&doc, &libraries);
        world.model.selection = Some(make_selection(vec![]));
        let frame = editing_frame(&mut world, false);
        assert!(world.model.selection.as_ref().unwrap().edit().is_none());
        assert!(frame.resolve_for_dispatch().dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Character(input.into()),
                ..arrow(NamedKey::End)
            }
        ));
        assert_eq!(world.model.doc.root, Some(expected));
        let selected = world.model.selection.as_mut().unwrap();
        selected.edit_mut().unwrap().cursor_to_start();
        editing_frame(&mut world, false);
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .edit()
                .unwrap()
                .selection_offsets(),
            (0, 0)
        );
    }
}

#[test]
fn a_plain_selection_accepts_ime_and_clipboard_without_prior_initialization() {
    let libraries = core_libraries();
    let doc = Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    };
    for ime in [false, true] {
        let mut world = editing_world(&doc, &libraries);
        world.model.selection = Some(make_selection(vec![]));
        world.text_clipboard.set_text(" world");
        let frame = editing_frame(&mut world, false);
        let handler = frame.resolve_for_dispatch();
        assert!(if ime {
            handler.dispatch_ime(
                &mut world,
                &puri::handler::ImeEvent::Commit(" world".into()),
            )
        } else {
            handler.dispatch_key(
                &mut world,
                &KeyboardEvent {
                    key: Key::Character("v".into()),
                    modifiers: crate::modifiers::command(),
                    ..arrow(NamedKey::End)
                },
            )
        });
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .edit()
                .unwrap()
                .text(),
            "hello world"
        );
    }
}

#[test]
fn a_caret_override_does_not_need_to_duplicate_text_or_write_back_rules() {
    let libraries = core_libraries();
    let doc = Document {
        root: Some(text::value("ab")),
        cells: Cells::new(),
    };
    let mut world = editing_world(&doc, &libraries);
    world.model.selection = Some(Selection::from_scoped_payload(
        &crate::test_root(),
        &src(&doc, &libraries),
        Default::default(),
        vec![],
        Value::record([
            (selection_payload::vocabulary::ANCHOR, f64::value(1.0)),
            (selection_payload::vocabulary::FOCUS, f64::value(1.0)),
        ]),
    ));
    let frame = editing_frame(&mut world, false);
    assert!(world.model.selection.as_ref().unwrap().edit().is_none());
    assert!(frame.resolve_for_dispatch().dispatch_key(
        &mut world,
        &KeyboardEvent {
            key: Key::Character("X".into()),
            ..arrow(NamedKey::End)
        }
    ));
    assert_eq!(world.model.doc.root, Some(text::value("aXb")));
}

#[test]
fn directional_line_arrival_is_distinct_from_ordinary_selection() {
    let libraries = core_libraries();
    let doc = Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    };
    use puri::handler::NavigationDirection;

    for direction in [
        None,
        Some(NavigationDirection::Down),
        Some(NavigationDirection::Up),
        Some(NavigationDirection::Right),
        Some(NavigationDirection::Left),
    ] {
        let mut world = editing_world(&doc, &libraries);
        let frame = editing_frame(&mut world, false);
        let target = frame
            .landmarks
            .iter()
            .find(|target| target.path.is_empty())
            .unwrap();
        assert!((target.select)(&mut world, direction));
        let selected = world.model.selection.as_ref().unwrap();
        if matches!(
            direction,
            Some(NavigationDirection::Left | NavigationDirection::Right)
        ) {
            let offset = if direction == Some(NavigationDirection::Right) {
                0
            } else {
                5
            };
            assert_eq!(
                selected.edit().unwrap().selection_offsets(),
                (offset, offset)
            );
        } else {
            assert!(selected.edit().is_none());
            assert_eq!(selected.initial_line("hello").selection_offsets(), (5, 5));
        }
    }
}

#[test]
fn deletion_and_history_landings_need_no_line_initialization() {
    let libraries = core_libraries();
    let doc = Document {
        root: Some(Value::list([text::value("first"), text::value("survivor")])),
        cells: Cells::new(),
    };
    let positions = positions(doc.root.as_ref().unwrap());
    let first = vec![Step::Element(positions[0].clone())];
    let second = vec![Step::Element(positions[1].clone())];
    let mut model = crate::model::Model::new(doc.clone());
    let root = model.workspace.document_root().clone();
    model.selection = Some(Selection::edge(&root, first.clone()));
    model.history.record(model.snapshot());
    let mut world = editing_world(&doc, &libraries);
    let frame = editing_frame(&mut world, false);
    assert!(delete_edge(
        &mut model.doc,
        &libraries,
        &crate::location::Location::root(&first)
    ));
    let next = crate::navigate::selection_after_removing(&frame.landmarks, None, &first, true);
    assert_eq!(next, second);
    model.selection = Some(Selection::edge(&root, next));
    for (undo, expected) in [(false, "survivor!"), (true, "first!")] {
        if undo {
            assert!(model.step_history(true));
        }
        let selected = model.selection.as_ref().unwrap();
        assert!(selected.edit().is_none());
        world.model.doc = model.doc.clone();
        world.model.selection = Some(make_selection(selected.path().to_vec()));
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_key(
                    &mut world,
                    &KeyboardEvent {
                        key: Key::Character("!".into()),
                        ..arrow(NamedKey::End)
                    }
                )
        );
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .edit()
                .unwrap()
                .text(),
            expected
        );
    }
}

#[test]
fn raw_and_read_only_projections_do_not_activate_a_default_line_editor() {
    let cell = new_cell_id();
    let library = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, text::value("read only"));
    let mut libraries = core_libraries();
    libraries.insert(
        library,
        crate::library::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
    );
    for (raw, value, path) in [
        (true, text::value("raw"), vec![]),
        (
            false,
            cell.into(),
            vec![Step::Follow(gid::Resolution::Library(library))],
        ),
    ] {
        let doc = Document {
            root: Some(value),
            cells: Cells::new(),
        };
        let mut world = editing_world(&doc, &libraries);
        world.model.selection = Some(make_selection(path));
        let frame = editing_frame(&mut world, raw);
        assert!(!frame.handler.is_some_and(|handler| handler.dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Character("!".into()),
                ..arrow(NamedKey::End)
            }
        )));
        assert!(world.model.selection.as_ref().unwrap().edit().is_none());
    }
}

#[test]
fn annotated_numbers_navigate_and_edit_only_the_digits() {
    use crate::libraries::{f32, u64};

    let libraries = core_libraries();
    for (original, spelling, expected) in [
        (f32::value(24.5), "24.5", f32::value(17.0)),
        (f64::value(-2.75), "-2.75", f64::value(17.0)),
        (u64::value(12), "12", u64::value(17)),
        (
            f64::value(std::primitive::f64::INFINITY),
            "inf",
            f64::value(17.0),
        ),
    ] {
        let doc = Rc::new(Document {
            root: Some(original),
            cells: Cells::new(),
        });
        let mut selected = make_projected_editing_selection(&doc, &libraries, vec![]);
        assert_eq!(selected.path(), &[]);
        assert_eq!(selected.edit().map(LineEditState::text), Some(spelling));
        *selected.edit_mut().unwrap() =
            LineEditState::from_parts(spelling, 0, spelling.len(), None, None);
        let mut world = editing_world(&doc, &libraries);
        world.model.selection = Some(selected);
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_ime(&mut world, &puri::handler::ImeEvent::Commit("17".into()),)
        );
        assert_eq!(world.model.doc.root, Some(expected));
    }
}

#[test]
fn payload_conversion_preserves_an_explicit_caret() {
    let libraries = core_libraries();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        text::value("hello"),
    )]);
    let path = vec![Step::Follow(gid::Resolution::Document), key("name")];
    let mut selection = make_projected_editing_selection(&doc, &libraries, path.clone());
    assert_eq!(selection.edit().unwrap().selection_offsets(), (5, 5));
    selection.edit_mut().unwrap().cursor_to_start();
    assert_eq!(selection.edit().unwrap().selection_offsets(), (0, 0));
    assert!(!write_text(&mut doc, &libraries, &mut selection));
    let reified = Selection::from_scoped_payload(
        &crate::test_root(),
        &src(&doc, &libraries),
        Default::default(),
        path,
        selection.payload(),
    );
    assert_eq!(reified.edit().unwrap().selection_offsets(), (0, 0));
    assert_eq!(reified.edit().unwrap().text(), "hello");
}

#[test]
fn edits_write_through_to_the_field() {
    let lib = core_libraries();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("old"),
    )]);
    let path = vec![Step::Follow(gid::Resolution::Document), key("name")];
    let mut selection = make_projected_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("new");
    write_text(&mut doc, &lib, &mut selection);
    assert_eq!(
        src(&doc, &lib).resolve_path(&path),
        Some(&crate::test_values::text("new"))
    );
    // A selection without an editor writes nothing.
    let mut plain = make_selection(vec![
        Step::Follow(gid::Resolution::Document),
        key("missing"),
    ]);
    assert!(!write_text(&mut doc, &lib, &mut plain));
    assert_eq!(
        src(&doc, &lib).resolve_path(&path),
        Some(&crate::test_values::text("new"))
    );
}

#[test]
fn blob_navigation_edits_complete_hex_and_keeps_the_last_valid_bytes() {
    let libraries = core_libraries();
    let cell = new_cell_id();
    let bytes: Vec<u8> = (0..=31).collect();
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(bytes.clone()));
    let mut doc = Rc::new(Document {
        root: Some(cell.into()),
        cells,
    });
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut selected = make_projected_editing_selection(&doc, &libraries, path.clone());
    assert_eq!(selected.edit().unwrap().text(), gid::hex_string(&bytes));
    assert!(!write_with(
        &mut doc,
        &libraries,
        &mut selected,
        crate::libraries::blob::edit
    ));
    selected.edit_mut().unwrap().set_text("DEad");
    assert!(write_with(
        &mut doc,
        &libraries,
        &mut selected,
        crate::libraries::blob::edit
    ));
    assert_eq!(doc.cells.value(cell), Some(&Value::from(vec![0xde, 0xad])));
    for (input, expected) in [
        ("DEadf", None),
        ("DEadff", Some(vec![0xde, 0xad, 0xff])),
        ("xx", None),
        ("", Some(vec![])),
    ] {
        let before = doc.clone();
        selected.edit_mut().unwrap().set_text(input);
        assert!(
            !write_with(
                &mut doc,
                &libraries,
                &mut selected,
                crate::libraries::blob::edit
            ),
            "one edit run opens one undo step"
        );
        assert_eq!(selected.edit().unwrap().text(), input);
        assert_eq!(doc.root, before.root);
        match expected {
            Some(bytes) => assert_eq!(doc.cells.value(cell), Some(&Value::from(bytes))),
            None => assert_eq!(doc.cells.value(cell), before.cells.value(cell)),
        }
    }
}

#[test]
fn compact_f64_values_edit_as_decimal_text() {
    let lib = core_libraries();
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, f64::value(2.5));
    let mut doc = Rc::new(Document {
        root: Some(Value::from(cell)),
        cells,
    });
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut selection = make_projected_editing_selection(&doc, &lib, path.clone());
    assert_eq!(selection.edit().map(LineEditState::text), Some("2.5"));
    selection.edit_mut().unwrap().set_text("7.25");
    assert!(write_with(&mut doc, &lib, &mut selection, |s, c| {
        crate::libraries::number::edit(s, c, f64::value)
    }));
    assert_eq!(
        src(&doc, &lib).resolve_path(&path).and_then(f64::read),
        Some(7.25)
    );

    selection.edit_mut().unwrap().set_text("not a number");
    assert!(!write_with(&mut doc, &lib, &mut selection, |s, c| {
        crate::libraries::number::edit(s, c, f64::value)
    }));
    assert_eq!(
        src(&doc, &lib).resolve_path(&path).and_then(f64::read),
        Some(7.25)
    );
}

#[test]
fn editing_an_f64_keeps_unrelated_fields() {
    let lib = core_libraries();
    let cell = new_cell_id();
    let unit = crate::test_values::label("unit");
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        Value::record(
            f64::value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(unit, crate::test_values::text("mm")),
        ),
    );
    let mut doc = Rc::new(Document {
        root: Some(Value::from(cell)),
        cells,
    });
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut selection = make_projected_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("8");
    assert!(write_with(&mut doc, &lib, &mut selection, |s, c| {
        crate::libraries::number::edit(s, c, f64::value)
    }));
    let value = src(&doc, &lib).resolve_path(&path).unwrap();
    assert_eq!(f64::read(value), Some(8.0));
    assert_eq!(
        value
            .as_record()
            .and_then(|fields| fields.get(&unit))
            .and_then(text::read),
        Some("mm")
    );
}

#[test]
fn element_edits_rebuild_the_list_at_the_owning_cell() {
    let lib = core_libraries();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("dash"),
        Value::list([crate::test_values::text("2"), crate::test_values::text("3")]),
    )]);
    let list_path = vec![Step::Follow(gid::Resolution::Document), key("dash")];
    let ps = positions(src(&doc, &lib).resolve_path(&list_path).unwrap());
    let element = vec![
        Step::Follow(gid::Resolution::Document),
        key("dash"),
        Step::Element(ps[1].clone()),
    ];

    // Editing an element writes the whole rebuilt list at the
    // owning cell; the sibling keeps its position and value.
    let mut selection = make_projected_editing_selection(&doc, &lib, element.clone());
    selection.edit_mut().unwrap().set_text("9");
    assert!(write_text(&mut doc, &lib, &mut selection));
    assert_eq!(
        src(&doc, &lib).resolve_path(&element),
        Some(&crate::test_values::text("9"))
    );
    assert_eq!(
        src(&doc, &lib).resolve_path(&list_path),
        Some(&Value::list([
            crate::test_values::text("2"),
            crate::test_values::text("9")
        ]))
    );
    assert_eq!(
        positions(src(&doc, &lib).resolve_path(&list_path).unwrap()),
        ps
    );
}

#[test]
fn write_through_opens_one_step_per_editor_life() {
    let lib = core_libraries();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("a"),
    )]);
    let path = vec![Step::Follow(gid::Resolution::Document), key("name")];
    let mut selection = make_projected_editing_selection(&doc, &lib, path);

    // First write opens the step; the rest of the run is silent,
    // as are no-op rewrites.
    selection.edit_mut().unwrap().set_text("ab");
    assert!(write_text(&mut doc, &lib, &mut selection));
    selection.edit_mut().unwrap().set_text("abc");
    assert!(!write_text(&mut doc, &lib, &mut selection));
    assert!(!write_text(&mut doc, &lib, &mut selection));

    // Breaking the run (a save) makes the next write a new step.
    selection.set_recorded(false);
    selection.edit_mut().unwrap().set_text("abcd");
    assert!(write_text(&mut doc, &lib, &mut selection));

    // A re-minted editor is a new run by construction.
    let mut fresh = make_projected_editing_selection(
        &doc,
        &lib,
        vec![Step::Follow(gid::Resolution::Document), key("name")],
    );
    fresh.edit_mut().unwrap().set_text("x");
    assert!(write_text(&mut doc, &lib, &mut fresh));
}

#[test]
fn a_simple_name_is_an_ordinary_editable_field() {
    let lib = core_libraries();
    let (mut doc, cell) = doc_of(vec![
        name::field("old"),
        (
            crate::test_values::label("x"),
            crate::test_values::text("1"),
        ),
    ]);
    let path = vec![
        Step::Follow(gid::Resolution::Document),
        Step::Key(name::vocabulary::NAME),
    ];

    let mut selection = make_projected_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("new");
    assert!(write_text(&mut doc, &lib, &mut selection));
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some("new"));
    assert!(!write_text(&mut doc, &lib, &mut selection));

    // Empty is an ordinary text value, not a hidden spelling of
    // field absence.
    selection.edit_mut().unwrap().set_text("");
    write_text(&mut doc, &lib, &mut selection);
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some(""));
    assert_eq!(
        doc.cells
            .value(cell)
            .and_then(Value::as_record)
            .and_then(|fields| { fields.get(&name::vocabulary::NAME) })
            .and_then(text::read),
        Some("")
    );

    // Removing the field uses the same structural deletion as any
    // other record field; the rest of the value remains.
    assert!(delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&path)
    ));
    assert_eq!(doc.cells.value(cell).and_then(name::read), None);
    assert!(
        doc.cells
            .value(cell)
            .and_then(Value::as_record)
            .is_some_and(|fields| fields.contains_key(&crate::test_values::label("x")))
    );
    assert!(make_selection(path).edit().is_none());
}

#[test]
fn missing_controls_default_without_mutating_selection_until_input() {
    let libraries = core_libraries();
    let cell = new_cell_id();
    let field = new_cell_id();
    let position = gid::position::between(None, None).unwrap();
    for (value, path) in [
        (None, vec![]),
        (
            Some(Value::from(cell)),
            vec![Step::Follow(gid::Resolution::Document)],
        ),
        (Some(Value::record([])), vec![Step::Key(field)]),
        (Some(Value::list([])), vec![Step::Element(position)]),
    ] {
        let doc = Document {
            root: value,
            cells: Cells::new(),
        };
        for payload in [Value::record([]), crate::selection::payload::edge()] {
            let mut world = editing_world(&doc, &libraries);
            world.model.selection = Some(Selection::from_scoped_payload(
                &crate::test_root(),
                &src(&doc, &libraries),
                Default::default(),
                path.clone(),
                payload.clone(),
            ));
            let original = world.model.doc.clone();
            let frame = editing_frame(&mut world, false);
            assert!(frame.completion.is_some());
            let selected = world.model.selection.as_ref().unwrap();
            assert!(selected.edit().is_none());
            assert_eq!(selected.payload(), payload);
            assert!(frame.resolve_for_dispatch().dispatch_key(
                &mut world,
                &KeyboardEvent {
                    key: Key::Character("new name".into()),
                    ..arrow(NamedKey::End)
                }
            ));
            assert_eq!(
                world
                    .model
                    .selection
                    .as_ref()
                    .unwrap()
                    .edit()
                    .unwrap()
                    .text(),
                "new name"
            );
            assert!(!write_text(
                &mut world.model.doc,
                &libraries,
                world.model.selection.as_mut().unwrap()
            ));
            assert!(Rc::ptr_eq(&world.model.doc, &original));
            world.model.selection = None;
            assert!(editing_frame(&mut world, false).completion.is_none());
            assert!(Rc::ptr_eq(&world.model.doc, &original));
        }
    }
}

#[test]
fn an_anonymous_lambda_name_opens_a_picker_without_creating_a_field() {
    let lib = core_libraries();
    let (doc, cell) = doc_of(vec![
        (grap::vocabulary::PARAMS, Value::list([])),
        (grap::vocabulary::BODY, Value::from(vec![1])),
    ]);
    let path = vec![
        Step::Follow(gid::Resolution::Document),
        Step::Key(name::vocabulary::NAME),
    ];
    let mut world = editing_world(&doc, &lib);
    let idle = editing_frame(&mut world, false);
    let original = world.model.doc.clone();
    let marker = idle.landmarks.landmark(&path).unwrap();
    let point = marker.rect.center();
    let target = Hovered::Tree(Hover::Value(Rc::from(path.clone())));
    assert_eq!(
        editing_frame_at(&mut world, false, None, Some(point))
            .claim
            .map(|(_, claim)| claim),
        Some(puri::hover::Claim::Direct(target.clone()))
    );
    let mut state = PointerState::default();
    state.position.x = point.x;
    state.position.y = point.y;
    assert!(idle.resolve_for_dispatch().dispatch_pointer_down_with(
        &mut world,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state,
        },
        &mut crate::frame::DispatchContext::new(None, Some(target)),
    ));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&src(&world.model.doc, &lib)),
        Stage::Pending
    );
    assert!(world.model.selection.as_ref().unwrap().edit().is_none());
    assert_eq!(
        world.model.selection.as_ref().unwrap().payload(),
        crate::selection::payload::edge()
    );
    assert!(editing_frame(&mut world, false).completion.is_some());
    assert!(!write_text(
        &mut world.model.doc,
        &lib,
        world.model.selection.as_mut().unwrap()
    ));
    assert!(src(&world.model.doc, &lib).resolve_path(&path).is_none());

    world.model.selection = None;
    assert!(editing_frame(&mut world, false).completion.is_none());
    assert!(Rc::ptr_eq(&world.model.doc, &original));

    // Keyboard navigation enters the same missing location.
    world.model.selection = Some(make_projected_selection(&doc, &lib, path.clone()));
    assert!(!write_text(
        &mut world.model.doc,
        &lib,
        world.model.selection.as_mut().unwrap()
    ));
    assert!(src(&world.model.doc, &lib).resolve_path(&path).is_none());
    let frame = editing_frame(&mut world, false);
    assert!(frame.completion.is_some());
    let handler = frame.resolve_for_dispatch();
    assert!(handler.dispatch_key(&mut world, &arrow(NamedKey::End)));
    assert!(!write_text(
        &mut world.model.doc,
        &lib,
        world.model.selection.as_mut().unwrap()
    ));
    assert!(src(&world.model.doc, &lib).resolve_path(&path).is_none());
    assert!(handler.dispatch_key(
        &mut world,
        &KeyboardEvent {
            key: Key::Character("\"tree\"".into()),
            ..arrow(NamedKey::End)
        }
    ));
    let selected = world.model.selection.as_mut().unwrap();
    assert_eq!(selected.stage(&src(&world.model.doc, &lib)), Stage::Pending);
    assert_eq!(selected.edit().unwrap().text(), "\"tree\"");
    assert!(!write_text(&mut world.model.doc, &lib, selected));
    assert!(src(&world.model.doc, &lib).resolve_path(&path).is_none());

    let prepared = crate::completion::prepare(
        &src(&world.model.doc, &lib),
        world.model.selection.as_ref().unwrap(),
        &Annotations::default(),
        text::value("tree"),
        None,
        None,
    )
    .unwrap();
    assert!(prepared.document_changed);
    assert_eq!(prepared.path, path);
    assert_eq!(
        prepared.document.cells.value(cell).and_then(name::read),
        Some("tree")
    );
    assert_eq!(
        make_projected_editing_selection(&prepared.document, &lib, path)
            .edit()
            .map(LineEditState::text),
        Some("tree")
    );
}

#[test]
fn a_read_only_anonymous_lambda_cannot_open_name_entry() {
    let cell = new_cell_id();
    let library = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, grap::lambda([], Value::from(vec![1])));
    let mut libraries = core_libraries();
    libraries.insert(
        library,
        crate::library::Definitions::from_parts(cells, Default::default()),
    );
    let doc = Document {
        root: Some(cell.into()),
        cells: Cells::new(),
    };
    let path = vec![
        Step::Follow(gid::Resolution::Library(library)),
        Step::Key(name::vocabulary::NAME),
    ];
    let mut world = editing_world(&doc, &libraries);
    world.model.selection = Some(make_projected_selection(&doc, &libraries, path));
    let original = world.model.doc.clone();
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&src(&doc, &libraries)),
        Stage::Edge
    );
    assert!(world.model.selection.as_ref().unwrap().edit().is_none());
    let selected = editing_frame(&mut world, false);
    assert!(selected.completion.is_none());
    assert!(!write_text(
        &mut world.model.doc,
        &libraries,
        world.model.selection.as_mut().unwrap()
    ));
    assert!(Rc::ptr_eq(&world.model.doc, &original));
}
