use super::*;

#[test]
fn replacing_query_text_settles_completion_state_before_the_next_edit() {
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = core_libraries();
    let payload = Value::record([
        (
            selection_payload::vocabulary::STAGE,
            selection_payload::vocabulary::PENDING.into(),
        ),
        (selection_payload::vocabulary::QUERY, text::value("old")),
        (
            selection_payload::vocabulary::EDITOR_TEXT,
            text::value("new"),
        ),
        (selection_payload::vocabulary::CHOICE, f64::value(2.0)),
        (
            selection_payload::vocabulary::COMPLETION_SCROLL,
            f64::value(30.0),
        ),
    ]);
    let mut selected =
        Selection::from_payload(&crate::test_root(), &src(&doc, &libraries), vec![], payload);
    assert_eq!(selected.choice(), 0);
    assert_eq!(selected.completion_scroll(), 0.0);
    selected.edit_query(|line| {
        line.set_text("old");
        true
    });
    assert_eq!(selected.choice(), 0);
    assert_eq!(selected.completion_scroll(), 0.0);
}

#[test]
fn native_line_conversions_agree_with_their_grap_entry_points() {
    use crate::libraries::{blob, color, f32, line_edit, number, u64};

    let libraries = core_libraries();
    let metadata = new_cell_id();
    for (current, function, native) in [
        (
            text::value("old"),
            text::vocabulary::UPDATE,
            line_edit::native(text::edit),
        ),
        (
            Value::from(vec![0xab]),
            blob::vocabulary::UPDATE,
            line_edit::native(blob::edit),
        ),
        (
            f32::value(1.0),
            f32::vocabulary::UPDATE,
            line_edit::native(|s, c| number::edit(s, c, f32::value)),
        ),
        (
            f64::value(1.0),
            f64::vocabulary::UPDATE,
            line_edit::native(|s, c| number::edit(s, c, f64::value)),
        ),
        (
            u64::value(1),
            u64::vocabulary::UPDATE,
            line_edit::native(|s, c| number::edit(s, c, u64::value)),
        ),
        (
            color::value(Color::new([0.2, 0.4, 0.6, 1.0])),
            color::vocabulary::UPDATE,
            line_edit::native(color::edit),
        ),
    ] {
        let current = match current {
            Value::Record(mut fields) => {
                fields.insert(metadata, text::value("preserve me"));
                Value::Record(fields)
            }
            value => value,
        };
        let doc = Document {
            root: Some(current.clone()),
            cells: Cells::new(),
        };
        let grap = line_edit::grap(grap::ffi(function));
        let sources = src(&doc, &libraries);
        for spelling in ["", "invalid", "1.5", "12", "-", "0", "ff", "123456"] {
            assert_eq!(
                native(&sources, spelling, Some(&current)),
                grap(&sources, spelling, Some(&current)),
                "conversion {function} at {spelling:?}",
            );
        }
    }
}

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
        assert!(frame.handler.unwrap().dispatch_key(
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
    let selection = Selection::from_payload(
        &crate::test_root(),
        &src(&world.model.doc, &libraries),
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
            .handler
            .unwrap()
            .dispatch_key(&mut world, &arrow(NamedKey::Home))
    );
    assert_eq!(calls.get(), 0);
    assert!(Rc::ptr_eq(&world.model.doc, &original));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        frame
            .handler
            .unwrap()
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
    let edit = |doc: &Document, path: Vec<Step>| make_editing_selection(doc, &lib, path);
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
        assert!(frame.handler.unwrap().dispatch_key(
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
        world.text_clipboard.text = Some(" world".into());
        let handler = editing_frame(&mut world, false).handler.unwrap();
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
                    modifiers: if cfg!(target_os = "macos") {
                        Modifiers::META
                    } else {
                        Modifiers::CONTROL
                    },
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
    world.model.selection = Some(Selection::from_payload(
        &crate::test_root(),
        &src(&doc, &libraries),
        vec![],
        Value::record([
            (selection_payload::vocabulary::ANCHOR, f64::value(1.0)),
            (selection_payload::vocabulary::FOCUS, f64::value(1.0)),
        ]),
    ));
    let frame = editing_frame(&mut world, false);
    assert!(world.model.selection.as_ref().unwrap().edit().is_none());
    assert!(frame.handler.unwrap().dispatch_key(
        &mut world,
        &KeyboardEvent {
            key: Key::Character("X".into()),
            ..arrow(NamedKey::End)
        }
    ));
    assert_eq!(world.model.doc.root, Some(text::value("aXb")));
}

#[test]
fn leftward_entry_is_explicit_but_ordinary_navigation_keeps_state_missing() {
    let libraries = core_libraries();
    let doc = Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    };
    use crate::navigate::Direction;

    for direction in [
        None,
        Some(Direction::Down),
        Some(Direction::Up),
        Some(Direction::Right),
        Some(Direction::Left),
    ] {
        let mut world = editing_world(&doc, &libraries);
        let frame = editing_frame(&mut world, false);
        let target = frame
            .descends
            .iter()
            .find(|target| target.path.is_empty())
            .unwrap();
        assert!((target.select)(&mut world, direction));
        match direction {
            Some(Direction::Left) => assert_eq!(
                world
                    .model
                    .selection
                    .as_ref()
                    .unwrap()
                    .edit()
                    .unwrap()
                    .selection_offsets(),
                (0, 0)
            ),
            _ => assert!(world.model.selection.as_ref().unwrap().edit().is_none()),
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
    assert!(delete_edge(&mut model.doc, &libraries, &first));
    let next = crate::navigate::selection_after_delete(&frame.descends, None, &first);
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
                .handler
                .unwrap()
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
        crate::libraries::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
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
                .handler
                .unwrap()
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
    let mut selection = make_editing_selection(&doc, &libraries, path.clone());
    assert_eq!(selection.edit().unwrap().selection_offsets(), (5, 5));
    selection.edit_mut().unwrap().cursor_to_start();
    assert_eq!(selection.edit().unwrap().selection_offsets(), (0, 0));
    assert!(!write_text(&mut doc, &libraries, &mut selection));
    let reified = Selection::from_payload(
        &crate::test_root(),
        &src(&doc, &libraries),
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
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
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
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
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
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
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
    let mut selection = make_editing_selection(&doc, &lib, element.clone());
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
fn set_value_writes_fields_elements_roots_and_bare_cells() {
    let lib = core_libraries();
    let (mut doc, cell) = doc_of(vec![(
        crate::test_values::label("x"),
        crate::test_values::text("1"),
    )]);
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("x")],
        crate::test_values::text("2")
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[Step::Follow(gid::Resolution::Document), key("x")]),
        Some(&crate::test_values::text("2"))
    );

    // A fresh Key step INSERTS a field; a deep spine rebuilds
    // through nested records and lists.
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("at")],
        Value::record([(
            crate::test_values::label("row"),
            crate::test_values::text("top")
        )]),
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &[
            Step::Follow(gid::Resolution::Document),
            key("at"),
            key("row")
        ],
        crate::test_values::text("bottom")
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[
            Step::Follow(gid::Resolution::Document),
            key("at"),
            key("row")
        ]),
        Some(&crate::test_values::text("bottom"))
    );

    // The whole cell value is addressable at Follow: conversion
    // is one set.
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document)],
        Value::list([crate::test_values::text("a")])
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[Step::Follow(gid::Resolution::Document)]),
        Some(&Value::list([crate::test_values::text("a")]))
    );

    // A bare cell takes its first value through the empty spine;
    // deeper steps into nothing decline.
    let bare = new_cell_id();
    Rc::make_mut(&mut doc).root = Some(Value::from(bare));
    assert!(!set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("x")],
        crate::test_values::text("v")
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document)],
        Value::record([(
            crate::test_values::label("x"),
            crate::test_values::text("v")
        )])
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[Step::Follow(gid::Resolution::Document), key("x")]),
        Some(&crate::test_values::text("v"))
    );

    // An inline record at the root writes on the root spine — no
    // cell involved.
    Rc::make_mut(&mut doc).root = Some(Value::record([(
        crate::test_values::label("shape"),
        Value::from(cell),
    )]));
    assert!(set_value(
        &mut doc,
        &lib,
        &[key("title")],
        crate::test_values::text("scene")
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[key("title")]),
        Some(&crate::test_values::text("scene"))
    );
    assert!(set_value(
        &mut doc,
        &lib,
        &[],
        crate::test_values::text("root")
    ));
    assert_eq!(doc.root, Some(crate::test_values::text("root")));
    // Text is a record convention, so a structural write can
    // enrich it. The text facet remains, and so does the extra
    // field.
    assert!(set_value(
        &mut doc,
        &lib,
        &[key("x")],
        crate::test_values::text("0")
    ));
    assert_eq!(
        doc.root
            .as_ref()
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&crate::test_values::label("x"))),
        Some(&crate::test_values::text("0"))
    );
    assert!(text::read(doc.root.as_ref().unwrap()).is_some());
}

#[test]
fn external_cells_decline_writes_and_bare_cells_accept() {
    let mut library_cells = Cells::new();
    let lib_cell = new_cell_id();
    library_cells.set_value(
        lib_cell,
        name::record(
            "convention",
            [(
                crate::test_values::label("a"),
                crate::test_values::text("1"),
            )],
        ),
    );
    let lib = libraries(library_cells);
    let mut doc = Rc::new(Document {
        root: Some(Value::from(lib_cell)),
        cells: Cells::new(),
    });
    // The library's cell declines writes wholesale.
    assert!(!set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("a")],
        crate::test_values::text("2")
    ));
    assert!(!set_value(
        &mut doc,
        &lib,
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ],
        crate::test_values::text("mine")
    ));
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("a")]
    ));
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document)]
    ));
    // Forking — the document taking the cell over — writes.
    Rc::make_mut(&mut doc).cells.set_value(
        lib_cell,
        Value::record([(
            crate::test_values::label("a"),
            crate::test_values::text("1"),
        )]),
    );
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("a")],
        crate::test_values::text("2")
    ));
}

#[test]
fn write_through_opens_one_step_per_editor_life() {
    let lib = core_libraries();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("a"),
    )]);
    let path = vec![Step::Follow(gid::Resolution::Document), key("name")];
    let mut selection = make_editing_selection(&doc, &lib, path);

    // First write opens the step; the rest of the run is silent,
    // as are no-op rewrites.
    selection.edit_mut().unwrap().set_text("ab");
    assert!(write_text(&mut doc, &lib, &mut selection));
    selection.edit_mut().unwrap().set_text("abc");
    assert!(!write_text(&mut doc, &lib, &mut selection));
    assert!(!write_text(&mut doc, &lib, &mut selection));

    // Breaking the run (a save) makes the next write a new step.
    break_edit_run(Some(&mut selection));
    selection.edit_mut().unwrap().set_text("abcd");
    assert!(write_text(&mut doc, &lib, &mut selection));

    // A re-minted editor is a new run by construction.
    let mut fresh = make_editing_selection(
        &doc,
        &lib,
        vec![Step::Follow(gid::Resolution::Document), key("name")],
    );
    fresh.edit_mut().unwrap().set_text("x");
    assert!(write_text(&mut doc, &lib, &mut fresh));
}

#[test]
fn delete_unlinks_fields_and_elements_and_bares_cells() {
    let lib = core_libraries();
    let child = new_cell_id();
    let (mut doc, cell) = doc_of(vec![
        (crate::test_values::label("child"), Value::from(child)),
        (
            crate::test_values::label("dash"),
            Value::list([crate::test_values::text("2"), crate::test_values::text("3")]),
        ),
    ]);
    Rc::make_mut(&mut doc)
        .cells
        .set_value(child, name::record("c", []));

    assert!(!delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("missing")]
    ));

    // Unlinking a field drops the link; the linked cell floats in
    // the table for the orphan pool.
    assert!(delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document), key("child")]
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[Step::Follow(gid::Resolution::Document), key("child")]),
        None
    );
    assert!(doc.cells.value(child).is_some());

    // An element step rebuilds the list without it.
    let dash = vec![Step::Follow(gid::Resolution::Document), key("dash")];
    let ps = positions(src(&doc, &lib).resolve_path(&dash).unwrap());
    assert!(delete_edge(
        &mut doc,
        &lib,
        &[
            Step::Follow(gid::Resolution::Document),
            key("dash"),
            Step::Element(ps[0].clone())
        ]
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&dash),
        Some(&Value::list([crate::test_values::text("3")]))
    );

    // A trailing Follow removes the cell's value: valueless
    // again, and a second delete declines.
    assert!(delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document)]
    ));
    assert!(doc.cells.value(cell).is_none());
    assert!(src(&doc, &lib).resolve_path(&[]).is_some());
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow(gid::Resolution::Document)]
    ));

    // The empty path empties the document.
    assert!(delete_edge(&mut doc, &lib, &[]));
    assert!(doc.root.is_none());
    assert!(!delete_edge(&mut doc, &lib, &[]));
}

#[test]
fn a_shadowing_document_definition_stays_editable() {
    let cell = new_cell_id();
    let mut library_cells = Cells::new();
    library_cells.set_value(cell, Value::record([]));
    let lib = libraries(library_cells);
    let root = crate::test_root();
    let mut doc = Rc::new(Document {
        root: Some(cell.into()),
        cells: Cells::new(),
    });
    assert!(pending_edge(&root, &src(&doc, &lib), vec![]).is_none());

    Rc::make_mut(&mut doc)
        .cells
        .set_value(cell, Value::record([]));
    let pending = pending_edge(&root, &src(&doc, &lib), vec![]).unwrap();
    assert_eq!(pending.path(), &[Step::Follow(gid::Resolution::Document)]);
    assert!(
        pending_edge(
            &root,
            &src(&doc, &lib),
            vec![Step::Follow(gid::Resolution::Library(CellId::from_u128(1)))],
        )
        .is_none()
    );

    Rc::make_mut(&mut doc)
        .cells
        .set_value(cell, Value::list([]));
    let pending = pending_into(&root, &src(&doc, &lib), &[]).unwrap();
    assert_eq!(
        pending.path().first(),
        Some(&Step::Follow(gid::Resolution::Document))
    );
}

#[test]
fn pendings_normalize_through_links_and_gate_on_authority() {
    let mut library_cells = Cells::new();
    let lib_cell = new_cell_id();
    library_cells.set_value(
        lib_cell,
        Value::record([(
            crate::test_values::label("a"),
            crate::test_values::text("1"),
        )]),
    );
    let lib = libraries(library_cells);
    let bare = new_cell_id();
    let (mut doc, _) = doc_of(vec![
        (crate::test_values::label("at"), Value::record([])),
        (
            crate::test_values::label("tags"),
            Value::list([crate::test_values::text("x")]),
        ),
        (crate::test_values::label("lib"), Value::from(lib_cell)),
        (crate::test_values::label("material"), Value::from(bare)),
        (
            crate::test_values::label("s"),
            crate::test_values::text("leaf"),
        ),
    ]);
    Rc::make_mut(&mut doc).root = doc.root.clone();
    let sources = src(&doc, &lib);

    // A link to a record cell pends its field under Follow; an
    // inline record pends at its own path.
    let on_cell = pending_edge(&crate::test_root(), &sources, vec![]).unwrap();
    assert_eq!(on_cell.path(), &[Step::Follow(gid::Resolution::Document)]);
    let inline = pending_edge(
        &crate::test_root(),
        &sources,
        vec![Step::Follow(gid::Resolution::Document), key("at")],
    )
    .unwrap();
    assert_eq!(
        inline.path(),
        &[Step::Follow(gid::Resolution::Document), key("at")]
    );

    // Lists, external cells, and bare cells decline fields. Text
    // is a record convention, so adding a field enriches it and
    // makes its structure visible.
    assert!(
        pending_edge(
            &crate::test_root(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("tags")]
        )
        .is_none()
    );
    assert!(
        pending_edge(
            &crate::test_root(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("s")]
        )
        .is_some()
    );
    assert!(
        pending_edge(
            &crate::test_root(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("lib")]
        )
        .is_none()
    );
    assert!(
        pending_edge(
            &crate::test_root(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("material")]
        )
        .is_none()
    );

    // A bare cell pends its first value at Follow — the
    // within-gesture's meaning there.
    let filling = pending_follow(
        &crate::test_root(),
        &sources,
        &[Step::Follow(gid::Resolution::Document), key("material")],
    )
    .unwrap();
    assert_eq!(
        filling.path(),
        &[
            Step::Follow(gid::Resolution::Document),
            key("material"),
            Step::Follow(gid::Resolution::Document)
        ]
    );
    assert!(
        pending_follow(
            &crate::test_root(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("lib")]
        )
        .is_none()
    );
    assert!(
        pending_follow(
            &crate::test_root(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("at")]
        )
        .is_none()
    );

    // Into a list through its link, appended at the end.
    let into = pending_into(
        &crate::test_root(),
        &sources,
        &[Step::Follow(gid::Resolution::Document), key("tags")],
    )
    .unwrap();
    assert!(matches!(into.path().last(), Some(Step::Element(_))));
    assert_eq!(into.path().len(), 3);

    // The within chord: fields on records, elements into lists,
    // first values into bare cells.
    assert!(pending_insert(&crate::test_root(), &sources, &[], false).is_some());
    assert!(
        pending_insert(
            &crate::test_root(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("tags")],
            false
        )
        .is_some()
    );
    assert!(
        pending_insert(
            &crate::test_root(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("material")],
            false
        )
        .is_some()
    );
    assert!(
        pending_insert(
            &crate::test_root(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("s")],
            false
        )
        .is_some()
    );
}

#[test]
fn queries_resolve_text_and_blobs() {
    assert_eq!(resolve_query("hello"), crate::test_values::text("hello"));
    assert_eq!(
        resolve_query("\"quoted\""),
        crate::test_values::text("quoted")
    );
    assert_eq!(resolve_query("\"open"), crate::test_values::text("open"));
    assert_eq!(resolve_query("\"0xff\""), crate::test_values::text("0xff"));
    assert_eq!(resolve_query("0xff00"), Value::from(vec![0xff, 0x00]));
    // Input is case-tolerant — the value is the bytes, lowercase
    // just the canonical spelling — and whole bytes only.
    assert_eq!(resolve_query("0xDEad"), Value::from(vec![0xde, 0xad]));
    assert_eq!(resolve_query("0xf"), crate::test_values::text("0xf"));
    assert_eq!(resolve_query("0x"), Value::from(vec![]));
}

#[test]
fn clipboard_spellings_round_trip() {
    let cell = new_cell_id();
    // Atoms are text and round-trip through it; structure rides
    // the private format and round-trips through its bytes.
    let atoms = [
        crate::test_values::text("plain"),
        crate::test_values::text("\"tricky\""),
        Value::from(vec![0xde, 0xad]),
    ];
    for value in atoms {
        let (text, structural) = to_clipboard(&value);
        assert!(!structural);
        assert_eq!(from_clipboard(&text), value);
    }
    let structures = [
        Value::from(cell),
        Value::list([crate::test_values::text("a"), Value::from(cell)]),
        Value::record([(
            crate::test_values::label("x"),
            crate::test_values::text("1"),
        )]),
        Value::record([(cell, Value::from(vec![0x00_u8]))]),
    ];
    for value in structures {
        let (text, structural) = to_clipboard(&value);
        assert!(structural);
        assert_eq!(from_structure(text.as_bytes()), Some(value));
    }
    // Atoms read in other apps; alien text pastes sensibly — and
    // TEXT IS NEVER STRUCTURE: characters that happen to spell
    // Value JSON read as the string they are.
    assert_eq!(to_clipboard(&crate::test_values::text("hi")).0, "\"hi\"");
    assert_eq!(to_clipboard(&Value::from(vec![0xff_u8])).0, "0xff");
    assert_eq!(
        from_clipboard("loose text"),
        crate::test_values::text("loose text")
    );
    let spelled = to_clipboard(&Value::record([])).0;
    assert_eq!(from_clipboard(&spelled), crate::test_values::text(&spelled));
}

#[test]
fn selecting_an_empty_value_slot_pends() {
    let bare = new_cell_id();
    let lib_cell = new_cell_id();
    let mut library_cells = Cells::new();
    library_cells.set_value(lib_cell, name::record("convention", []));
    let lib = libraries(library_cells);
    let mut doc = Rc::new(Document {
        root: Some(Value::from(bare)),
        cells: Cells::new(),
    });
    // A writable valueless cell's Follow slot is already
    // authoring: selecting it (the rendered placeholder) pends.
    assert_eq!(
        make_selection(vec![Step::Follow(gid::Resolution::Document)]).stage(&src(&doc, &lib)),
        crate::selection::Stage::Pending
    );
    // Valued, it selects normally.
    Rc::make_mut(&mut doc)
        .cells
        .set_value(bare, crate::test_values::text("v"));
    assert_eq!(
        make_selection(vec![Step::Follow(gid::Resolution::Document)]).stage(&src(&doc, &lib)),
        crate::selection::Stage::Edge
    );
    // An EXTERNAL cell has an ordinary value, so its Follow slot
    // selects normally and remains unwritable.
    Rc::make_mut(&mut doc).root = Some(Value::from(lib_cell));
    let external = make_selection(vec![Step::Follow(gid::Resolution::Library(
        CellId::from_u128(1),
    ))]);
    assert_eq!(
        external.stage(&src(&doc, &lib)),
        crate::selection::Stage::Edge
    );
    assert!(external.edit().is_none());
    // The empty document's root is the same rule.
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    assert_eq!(
        make_selection(vec![]).stage(&src(&empty, &lib)),
        crate::selection::Stage::Pending
    );
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

    let mut selection = make_editing_selection(&doc, &lib, path.clone());
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
    assert!(delete_edge(&mut doc, &lib, &path));
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
        for payload in [Value::record([]), crate::libraries::selection::edge()] {
            let mut world = editing_world(&doc, &libraries);
            world.model.selection = Some(Selection::from_payload(
                &crate::test_root(),
                &src(&doc, &libraries),
                path.clone(),
                payload.clone(),
            ));
            let original = world.model.doc.clone();
            let frame = editing_frame(&mut world, false);
            assert!(frame.completion.is_some());
            let selected = world.model.selection.as_ref().unwrap();
            assert!(selected.edit().is_none());
            assert_eq!(selected.payload(), payload);
            assert!(frame.handler.unwrap().dispatch_key(
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
    let marker = idle
        .descends
        .iter()
        .find(|d| d.path.as_ref() == path)
        .unwrap();
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
    assert!(idle.handler.unwrap().dispatch_pointer_down_with(
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
        &mut placed::DispatchContext::new(None, Some(target)),
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
    assert!(
        frame
            .handler
            .as_ref()
            .unwrap()
            .dispatch_key(&mut world, &arrow(NamedKey::End))
    );
    assert!(!write_text(
        &mut world.model.doc,
        &lib,
        world.model.selection.as_mut().unwrap()
    ));
    assert!(src(&world.model.doc, &lib).resolve_path(&path).is_none());
    assert!(frame.handler.unwrap().dispatch_key(
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
        crate::libraries::Definitions::from_parts(cells, Default::default()),
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

#[test]
fn custom_update_can_discard_an_absent_and_return_a_value() {
    use crate::libraries::{control::vocabulary as c, line_edit::vocabulary as l};
    let missing = new_cell_id();
    let update = grap::lambda(
        [l::INPUT, l::CURRENT],
        grap::call(
            Value::from(c::DO),
            [(
                c::EXPRESSIONS,
                Value::list([Value::from(missing), Value::from(l::INPUT)]),
            )],
        ),
    );
    let libraries = core_libraries();
    let mut doc = Rc::new(Document {
        root: Some(text::value("before")),
        cells: Cells::new(),
    });
    let function = grap::evaluate(&update, &src(&doc, &libraries), 1000).result;
    let evaluated = grap::apply(
        &function,
        [
            (l::INPUT, text::value("after")),
            (l::CURRENT, doc.root.clone().unwrap()),
        ],
        &src(&doc, &libraries),
        1000,
    );
    assert_eq!(text::read(&evaluated.result), Some("after"));
    let line = crate::display::LineEdit {
        text: "after".into(),
        placeholder: None,
        update: crate::libraries::line_edit::grap(function),
        prefix: String::new(),
        suffix: String::new(),
        family: Default::default(),
    };
    let mut selected = Selection::from_line(
        &crate::test_root(),
        &src(&doc, &libraries),
        vec![],
        line.clone(),
    );
    assert!(line_control::commit(
        &mut doc,
        &libraries,
        &mut selected,
        &line.update,
    ));
    assert_eq!(text::read(doc.root.as_ref().unwrap()), Some("after"));
}
