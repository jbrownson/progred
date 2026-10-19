use super::*;
use crate::driver::Driver;
use puri::edit::TextClipboard;
use puri::handler::ImeEvent;
use ui_events::keyboard::Modifiers;

/// Select the whole line being edited, so what's typed next replaces it.
fn select_line(driver: &mut Driver) {
    assert!(driver.keystroke(Key::Named(NamedKey::Home), Modifiers::SHIFT));
}

/// Click the window's empty corner, which drops the selection.
fn deselect(driver: &mut Driver) {
    driver.click_at(Point::new(
        Driver::VIEWPORT.width - 4.0,
        Driver::VIEWPORT.height - 4.0,
    ));
    assert!(driver.editor().model.selection.is_none());
}

#[test]
fn a_reminted_line_uses_its_current_conversion_not_selection_wiring() {
    fn projection(marker: CellId) -> Projection {
        Projection::new([crate::display::partial(move |input| {
            let spelling = text::read(input.value?)?;
            Some(crate::display::line_edit(crate::display::LineEdit {
                text: spelling.into(),
                update: crate::libraries::line_edit::native(move |spelling, _| {
                    Some(Value::record([
                        (
                            text::vocabulary::UTF8,
                            Value::from(spelling.as_bytes().to_vec()),
                        ),
                        (marker, Value::record([])),
                    ]))
                }),
                prefix: String::new(),
                suffix: String::new(),
                family: Default::default(),
            }))
        })])
    }
    let mut driver = Driver::of(text::value("a"));
    driver.select(&[]);
    for (marker, typed, expected) in [(new_cell_id(), "b", "ab"), (new_cell_id(), "c", "abc")] {
        let mut stack = driver.editor().stack.clone();
        stack.projection = projection(marker);
        driver.stack(stack);
        driver.type_text(typed);
        let root = driver.value(&[]);
        assert_eq!(text::read(&root), Some(expected));
        let fields = root.as_record().unwrap();
        assert!(fields.contains_key(&marker));
        assert_eq!(fields.len(), 2);
    }
    let editor = driver.editor();
    let payload = editor.model.selection.as_ref().unwrap().payload();
    let selection = Selection::from_payload(
        &crate::test_root(),
        &editor.sources(),
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
    let mut editor = crate::editor_of(text::value("abc"));
    editor.stack.projection = Projection::new([crate::display::partial({
        let calls = calls.clone();
        move |input| {
            Some(crate::display::line_edit(crate::display::LineEdit {
                text: text::read(input.value?)?.into(),
                update: crate::libraries::line_edit::native({
                    let calls = calls.clone();
                    move |spelling, current| {
                        calls.set(calls.get() + 1);
                        text::edit(spelling, current)
                    }
                }),
                prefix: String::new(),
                suffix: String::new(),
                family: Default::default(),
            }))
        }
    })]);
    let mut driver = Driver::new(editor, Driver::VIEWPORT);
    driver.select(&[]);
    let original = driver.editor().model.doc.clone();
    driver.press(NamedKey::Home);
    assert_eq!(calls.get(), 0);
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &original));
    assert!(driver.ime(ImeEvent::Commit("x".into())));
    assert_eq!(calls.get(), 1);
    assert_eq!(text::read(&driver.value(&[])), Some("xabc"));
}

#[test]
fn line_projection_descriptions_mount_the_rust_editor() {
    let lib = core_libraries();
    let (mut doc, cell) = doc_of(vec![
        (
            crate::test_values::label("name"),
            crate::libraries::text::value("old"),
        ),
        (
            crate::test_values::label("x"),
            crate::libraries::text::value("1.5"),
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
        .set_value(cell, crate::libraries::text::value("held"));
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
        crate::libraries::text::value("old"),
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
    for (value, input, expected) in [
        (text::value("hë🦀"), "!", text::value("hë🦀!")),
        (text::value(""), "first", text::value("first")),
        (f64::value(12.0), "3", f64::value(123.0)),
        (Value::from(vec![0xab]), "cd", Value::from(vec![0xab, 0xcd])),
    ] {
        let mut driver = Driver::of(value);
        driver.select(&[]);
        assert!(driver.line().is_none());
        driver.type_text(input);
        assert_eq!(driver.value(&[]), expected);
        // The caret the typing started stays where it's moved.
        driver.press(NamedKey::Home);
        assert_eq!(driver.line_selection(), Some((0, 0)));
    }
}

#[test]
fn a_plain_selection_accepts_ime_and_clipboard_without_prior_initialization() {
    for ime in [false, true] {
        let mut driver = Driver::of(text::value("hello"));
        driver.runner.editor.text_clipboard.set_text(" world");
        driver.select(&[]);
        assert!(if ime {
            driver.ime(ImeEvent::Commit(" world".into()))
        } else {
            driver.keystroke(Key::Character("v".into()), crate::test_support::command())
        });
        assert_eq!(driver.line(), Some("hello world"));
    }
}

#[test]
fn a_caret_override_does_not_need_to_duplicate_text_or_write_back_rules() {
    let mut editor = crate::editor_of(text::value("ab"));
    let selection = Selection::from_payload(
        &crate::test_root(),
        &editor.sources(),
        Default::default(),
        vec![],
        Value::record([
            (selection_payload::vocabulary::ANCHOR, f64::value(1.0)),
            (selection_payload::vocabulary::FOCUS, f64::value(1.0)),
        ]),
    );
    editor.model.selection = Some(selection);
    let mut driver = Driver::new(editor, Driver::VIEWPORT);
    assert!(driver.line().is_none());
    driver.type_text("X");
    assert_eq!(driver.value(&[]), text::value("aXb"));
}

#[test]
fn directional_line_arrival_is_distinct_from_ordinary_selection() {
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
        let mut world = crate::test_editor(doc.clone());
        let frame = editing_frame(&mut world);
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
    model.selection = Some(Selection::edge(&root, Default::default(), first.clone()));
    model.history.record(model.snapshot());
    let drawn = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    assert!(delete_edge(
        &mut model.doc,
        &libraries,
        &crate::location::Location::root(&first)
    ));
    let next = crate::navigate::selection_after_removing(
        &drawn.runner.frame.dispatch.landmarks,
        Some(&crate::test_root()),
        &first,
        true,
    );
    assert_eq!(next, second);
    model.selection = Some(Selection::edge(&root, Default::default(), next));
    for (undo, expected) in [(false, "survivor!"), (true, "first!")] {
        if undo {
            assert!(model.step_history(true));
        }
        let selected = model.selection.as_ref().unwrap();
        assert!(selected.edit().is_none());
        let mut editor = crate::test_editor((*model.doc).clone());
        editor.model.selection = Some(make_selection(selected.path().to_vec()));
        let mut driver = Driver::new(editor, Driver::VIEWPORT);
        driver.type_text("!");
        assert_eq!(driver.line(), Some(expected));
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
        let mut editor = {
            let mut editor = crate::test_editor(Document {
                root: Some(value),
                cells: Cells::new(),
            });
            editor.stack.libraries = libraries.clone();
            editor
        };
        if raw {
            editor.model.workspace.document.projection = crate::workspace::Projection::Raw;
        }
        editor.model.selection = Some(make_selection(path));
        let mut driver = Driver::new(editor, Driver::VIEWPORT);
        let original = driver.editor().model.doc.clone();
        // Typing over a raw value opens a hole for its replacement, and a
        // read-only one takes nothing; neither edits the value's spelling.
        assert_eq!(
            driver.keystroke(Key::Character("!".into()), Modifiers::empty()),
            raw
        );
        assert_eq!(
            driver.stage(),
            Some(if raw { Stage::Pending } else { Stage::Edge })
        );
        assert!(Rc::ptr_eq(&driver.editor().model.doc, &original));
    }
}

#[test]
fn annotated_numbers_navigate_and_edit_only_the_digits() {
    use crate::libraries::{f32, u64};

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
        let mut driver = Driver::of(original);
        driver.select(&[]);
        driver.press(NamedKey::End);
        assert_eq!(driver.selection(), []);
        assert_eq!(driver.line(), Some(spelling));
        select_line(&mut driver);
        assert!(driver.ime(ImeEvent::Commit("17".into())));
        assert_eq!(driver.value(&[]), expected);
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
    let reified = Selection::from_payload(
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
        crate::libraries::text::value("old"),
    )]);
    let path = vec![Step::Follow(gid::Resolution::Document), key("name")];
    let mut selection = make_projected_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("new");
    write_text(&mut doc, &lib, &mut selection);
    assert_eq!(
        src(&doc, &lib).resolve_path(&path),
        Some(&crate::libraries::text::value("new"))
    );
    // A selection without an editor writes nothing.
    let mut plain = make_selection(vec![
        Step::Follow(gid::Resolution::Document),
        key("missing"),
    ]);
    assert!(!write_text(&mut doc, &lib, &mut plain));
    assert_eq!(
        src(&doc, &lib).resolve_path(&path),
        Some(&crate::libraries::text::value("new"))
    );
}

#[test]
fn blob_navigation_edits_complete_hex_and_keeps_the_last_valid_bytes() {
    let cell = new_cell_id();
    let bytes: Vec<u8> = (0..=31).collect();
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(bytes.clone()));
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(cell.into()),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let path = [Step::Follow(gid::Resolution::Document)];
    let original = driver.editor().model.doc.clone();
    driver.select(&path);
    driver.press(NamedKey::End);
    assert_eq!(driver.line(), Some(gid::hex_string(&bytes).as_str()));
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &original));
    for (input, expected) in [
        ("DEad", Some(vec![0xde, 0xad])),
        ("DEadf", None),
        ("DEadff", Some(vec![0xde, 0xad, 0xff])),
        ("xx", None),
        ("", Some(vec![])),
    ] {
        let before = driver.editor().model.doc.clone();
        select_line(&mut driver);
        match input {
            "" => driver.press(NamedKey::Backspace),
            input => driver.type_text(input),
        }
        assert_eq!(driver.line(), Some(input));
        let doc = &driver.editor().model.doc;
        assert_eq!(doc.root, before.root);
        assert_eq!(
            doc.cells.value(cell),
            expected
                .map(Value::from)
                .as_ref()
                .or(before.cells.value(cell))
        );
    }
    // The whole run of edits is one undo step.
    assert!(driver.keystroke(Key::Character("z".into()), crate::test_support::command()));
    assert_eq!(
        driver.editor().model.doc.cells.value(cell),
        Some(&Value::from(bytes))
    );
}

#[test]
fn compact_f64_values_edit_as_decimal_text() {
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, f64::value(2.5));
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(Value::from(cell)),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let path = [Step::Follow(gid::Resolution::Document)];
    driver.select(&path);
    driver.press(NamedKey::End);
    assert_eq!(driver.line(), Some("2.5"));
    // What isn't a number leaves the last one written.
    for (typed, expected) in [("7.25", 7.25), ("not a number", 7.25)] {
        select_line(&mut driver);
        driver.type_text(typed);
        assert_eq!(driver.number(&path), Some(expected));
    }
}

#[test]
fn editing_an_f64_keeps_unrelated_fields() {
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
                .update(unit, crate::libraries::text::value("mm")),
        ),
    );
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(Value::from(cell)),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let path = [Step::Follow(gid::Resolution::Document)];
    driver.select(&path);
    driver.press(NamedKey::End);
    select_line(&mut driver);
    driver.type_text("8");
    let value = driver.value(&path);
    assert_eq!(f64::read(&value), Some(8.0));
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
        Value::list([
            crate::libraries::text::value("2"),
            crate::libraries::text::value("3"),
        ]),
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
        Some(&crate::libraries::text::value("9"))
    );
    assert_eq!(
        src(&doc, &lib).resolve_path(&list_path),
        Some(&Value::list([
            crate::libraries::text::value("2"),
            crate::libraries::text::value("9")
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
        crate::libraries::text::value("a"),
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
            crate::libraries::text::value("1"),
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
            let mut editor = crate::test_editor(doc.clone());
            editor.model.selection = Some(Selection::from_payload(
                &crate::test_root(),
                &src(&doc, &libraries),
                Default::default(),
                path.clone(),
                payload.clone(),
            ));
            let mut driver = Driver::new(editor, Driver::VIEWPORT);
            let original = driver.editor().model.doc.clone();
            assert!(driver.picker_entries().is_some());
            assert!(driver.line().is_none());
            assert_eq!(
                driver.editor().model.selection.as_ref().unwrap().payload(),
                payload
            );
            driver.type_text("new name");
            assert_eq!(driver.line(), Some("new name"));
            let editor = &mut driver.runner.editor;
            assert!(!write_text(
                &mut editor.model.doc,
                &libraries,
                editor.model.selection.as_mut().unwrap()
            ));
            assert!(Rc::ptr_eq(&driver.editor().model.doc, &original));
            deselect(&mut driver);
            assert!(driver.picker_entries().is_none());
            assert!(Rc::ptr_eq(&driver.editor().model.doc, &original));
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
    let unwritten = |driver: &mut Driver| {
        let editor = &mut driver.runner.editor;
        assert!(!write_text(
            &mut editor.model.doc,
            &lib,
            editor.model.selection.as_mut().unwrap()
        ));
        assert!(editor.sources().resolve_path(&path).is_none());
    };
    let mut driver = Driver::new(crate::test_editor((*doc).clone()), Driver::VIEWPORT);
    let original = driver.editor().model.doc.clone();
    let point = driver.rect(&path).center();
    assert_eq!(
        driver.hover_at(point),
        Some(Hovered::Tree(Hover::Value(Rc::from(path.clone()))))
    );
    driver.click_at(point);
    assert_eq!(driver.selection(), path);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert_eq!(driver.line(), Some(""), "the picker's query starts empty");
    assert!(driver.picker_entries().is_some());
    unwritten(&mut driver);
    deselect(&mut driver);
    assert!(driver.picker_entries().is_none());
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &original));

    // Keyboard navigation enters the same missing location.
    driver.select(&path);
    unwritten(&mut driver);
    assert!(driver.picker_entries().is_some());
    driver.press(NamedKey::End);
    unwritten(&mut driver);
    driver.type_text("\"tree\"");
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert_eq!(driver.line(), Some("\"tree\""));
    unwritten(&mut driver);

    let editor = driver.editor();
    let prepared = crate::completion::prepare(
        &editor.sources(),
        editor.model.selection.as_ref().unwrap(),
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
    let mut world = {
        let mut editor = crate::test_editor(doc.clone());
        editor.stack.libraries = libraries.clone();
        editor
    };
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
    let selected = editing_frame(&mut world);
    assert!(selected.completion.is_none());
    assert!(!write_text(
        &mut world.model.doc,
        &libraries,
        world.model.selection.as_mut().unwrap()
    ));
    assert!(Rc::ptr_eq(&world.model.doc, &original));
}
