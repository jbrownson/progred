use super::*;

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
        make_selection(
            &doc,
            &lib,
            vec![Step::Follow(gid::Resolution::Document), key("missing")]
        )
        .edit()
        .is_none()
    );
    assert!(make_selection(&doc, &lib, vec![]).edit().is_none());
    doc.cells.set_value(
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
    doc.cells.set_value(cell, crate::test_values::text("held"));
    assert_eq!(
        edit(&doc, vec![Step::Follow(gid::Resolution::Document)])
            .edit()
            .map(LineEditState::text),
        Some("held")
    );
    // A simple name convention is just another text field.
    doc.cells.set_value(cell, name::record("roof", []));
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
fn a_line_control_installs_its_navigation_selection() {
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
    assert_eq!(selected.edit().map(LineEditState::text), Some("old"));
}

#[test]
fn leftward_navigation_sets_the_live_caret_and_payload_conversion_preserves_it() {
    let libraries = core_libraries();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        text::value("hello"),
    )]);
    let path = vec![Step::Follow(gid::Resolution::Document), key("name")];
    let mut selection = make_editing_selection(&doc, &libraries, path.clone());
    assert_eq!(selection.edit().unwrap().selection_offsets(), (5, 5));
    crate::selection::seed_from_arrow(&mut selection, &arrow(NamedKey::ArrowLeft));
    assert_eq!(selection.edit().unwrap().selection_offsets(), (0, 0));
    assert!(!write_through(&mut doc, &libraries, &mut selection));
    let reified = Selection::from_payload(
        &crate::workspace::Root::document(),
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
    write_through(&mut doc, &lib, &mut selection);
    assert_eq!(
        src(&doc, &lib).resolve_path(&path),
        Some(&crate::test_values::text("new"))
    );
    // A selection without an editor writes nothing.
    let mut plain = make_selection(
        &doc,
        &lib,
        vec![Step::Follow(gid::Resolution::Document), key("missing")],
    );
    assert!(!write_through(&mut doc, &lib, &mut plain));
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
    let mut doc = Document {
        root: Some(cell.into()),
        cells,
    };
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut selected = make_projected_selection(&doc, &libraries, path.clone());
    assert_eq!(selected.edit().unwrap().text(), gid::hex_string(&bytes));
    assert!(!write_through(&mut doc, &libraries, &mut selected));
    selected.edit_mut().unwrap().set_text("DEad");
    assert!(write_through(&mut doc, &libraries, &mut selected));
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
            !write_through(&mut doc, &libraries, &mut selected),
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
    let mut doc = Document {
        root: Some(Value::from(cell)),
        cells,
    };
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
    assert_eq!(selection.edit().map(LineEditState::text), Some("2.5"));
    selection.edit_mut().unwrap().set_text("7.25");
    assert!(write_through(&mut doc, &lib, &mut selection));
    assert_eq!(
        src(&doc, &lib).resolve_path(&path).and_then(f64::read),
        Some(7.25)
    );

    selection.edit_mut().unwrap().set_text("not a number");
    assert!(!write_through(&mut doc, &lib, &mut selection));
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
    let mut doc = Document {
        root: Some(Value::from(cell)),
        cells,
    };
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("8");
    assert!(write_through(&mut doc, &lib, &mut selection));
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
    assert!(write_through(&mut doc, &lib, &mut selection));
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
    doc.root = Some(Value::from(bare));
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
    doc.root = Some(Value::record([(
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
    let mut doc = Document {
        root: Some(Value::from(lib_cell)),
        cells: Cells::new(),
    };
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
    doc.cells.set_value(
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
    assert!(write_through(&mut doc, &lib, &mut selection));
    selection.edit_mut().unwrap().set_text("abc");
    assert!(!write_through(&mut doc, &lib, &mut selection));
    assert!(!write_through(&mut doc, &lib, &mut selection));

    // Breaking the run (a save) makes the next write a new step.
    break_edit_run(Some(&mut selection));
    selection.edit_mut().unwrap().set_text("abcd");
    assert!(write_through(&mut doc, &lib, &mut selection));

    // A re-minted editor is a new run by construction.
    let mut fresh = make_editing_selection(
        &doc,
        &lib,
        vec![Step::Follow(gid::Resolution::Document), key("name")],
    );
    fresh.edit_mut().unwrap().set_text("x");
    assert!(write_through(&mut doc, &lib, &mut fresh));
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
    doc.cells.set_value(child, name::record("c", []));

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
    doc.root = doc.root.clone();
    let sources = src(&doc, &lib);

    // A link to a record cell pends its field under Follow; an
    // inline record pends at its own path.
    let on_cell = pending_edge(&crate::workspace::Root::document(), &sources, vec![]).unwrap();
    assert_eq!(on_cell.path(), &[Step::Follow(gid::Resolution::Document)]);
    let inline = pending_edge(
        &crate::workspace::Root::document(),
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
            &crate::workspace::Root::document(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("tags")]
        )
        .is_none()
    );
    assert!(
        pending_edge(
            &crate::workspace::Root::document(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("s")]
        )
        .is_some()
    );
    assert!(
        pending_edge(
            &crate::workspace::Root::document(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("lib")]
        )
        .is_none()
    );
    assert!(
        pending_edge(
            &crate::workspace::Root::document(),
            &sources,
            vec![Step::Follow(gid::Resolution::Document), key("material")]
        )
        .is_none()
    );

    // A bare cell pends its first value at Follow — the
    // within-gesture's meaning there.
    let filling = pending_follow(
        &crate::workspace::Root::document(),
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
            &crate::workspace::Root::document(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("lib")]
        )
        .is_none()
    );
    assert!(
        pending_follow(
            &crate::workspace::Root::document(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("at")]
        )
        .is_none()
    );

    // Into a list through its link, appended at the end.
    let into = pending_into(
        &crate::workspace::Root::document(),
        &sources,
        &[Step::Follow(gid::Resolution::Document), key("tags")],
    )
    .unwrap();
    assert!(matches!(into.path().last(), Some(Step::Element(_))));
    assert_eq!(into.path().len(), 3);

    // The within chord: fields on records, elements into lists,
    // first values into bare cells.
    assert!(pending_insert(&crate::workspace::Root::document(), &sources, &[], false).is_some());
    assert!(
        pending_insert(
            &crate::workspace::Root::document(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("tags")],
            false
        )
        .is_some()
    );
    assert!(
        pending_insert(
            &crate::workspace::Root::document(),
            &sources,
            &[Step::Follow(gid::Resolution::Document), key("material")],
            false
        )
        .is_some()
    );
    assert!(
        pending_insert(
            &crate::workspace::Root::document(),
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
    let mut doc = Document {
        root: Some(Value::from(bare)),
        cells: Cells::new(),
    };
    // A writable valueless cell's Follow slot is already
    // authoring: selecting it (the rendered placeholder) pends.
    assert_eq!(
        make_selection(&doc, &lib, vec![Step::Follow(gid::Resolution::Document)]).stage(),
        crate::selection::Stage::Pending
    );
    // Valued, it selects normally.
    doc.cells.set_value(bare, crate::test_values::text("v"));
    assert_eq!(
        make_selection(&doc, &lib, vec![Step::Follow(gid::Resolution::Document)]).stage(),
        crate::selection::Stage::Edge
    );
    // An EXTERNAL cell has an ordinary value, so its Follow slot
    // selects normally and remains unwritable.
    doc.root = Some(Value::from(lib_cell));
    let external = make_selection(
        &doc,
        &lib,
        vec![Step::Follow(gid::Resolution::Library(CellId::from_u128(1)))],
    );
    assert_eq!(external.stage(), crate::selection::Stage::Edge);
    assert!(external.edit().is_none());
    // The empty document's root is the same rule.
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    assert_eq!(
        make_selection(&empty, &lib, vec![]).stage(),
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
    assert!(write_through(&mut doc, &lib, &mut selection));
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some("new"));
    assert!(!write_through(&mut doc, &lib, &mut selection));

    // Empty is an ordinary text value, not a hidden spelling of
    // field absence.
    selection.edit_mut().unwrap().set_text("");
    write_through(&mut doc, &lib, &mut selection);
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
    assert!(make_selection(&doc, &lib, path).edit().is_none());
}

#[test]
fn editing_an_anonymous_lambdas_placeholder_creates_its_name_field() {
    let lib = core_libraries();
    let (mut doc, cell) = doc_of(vec![
        (grap::vocabulary::PARAMS, Value::list([])),
        (grap::vocabulary::BODY, Value::from(vec![1])),
    ]);
    let path = vec![
        Step::Follow(gid::Resolution::Document),
        Step::Key(name::vocabulary::NAME),
    ];
    let mut selection = make_projected_selection(&doc, &lib, path.clone());

    assert_eq!(selection.edit().map(LineEditState::text), Some(""));
    selection.edit_mut().unwrap().set_text("tree");
    assert!(write_through(&mut doc, &lib, &mut selection,));
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some("tree"));
    assert_eq!(
        src(&doc, &lib).resolve_path(&path).and_then(text::read),
        Some("tree")
    );
}

#[test]
fn custom_update_can_discard_an_absent_and_return_a_value() {
    use progred_libraries::{control::vocabulary as c, line_edit::vocabulary as l};
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
    let mut doc = Document {
        root: Some(text::value("before")),
        cells: Cells::new(),
    };
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
    let line = progred_display::LineEdit {
        text: "after".into(),
        placeholder: None,
        update: function,
        prefix: String::new(),
        suffix: String::new(),
        family: Default::default(),
    };
    let mut selected = Selection::from_line(
        &crate::workspace::Root::document(),
        &src(&doc, &libraries),
        vec![],
        line,
    );
    assert!(crate::selection::write_through(
        &mut doc,
        &libraries,
        &mut selected
    ));
    assert_eq!(text::read(doc.root.as_ref().unwrap()), Some("after"));
}
