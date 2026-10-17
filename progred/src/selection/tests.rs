use super::*;
use crate::libraries::{f64, name};
use crate::selection::payload as selection_payload;
use crate::test_support::{core_libraries, doc_of, make_selection, positions, src};
use crate::test_values::key;
use gid::{Cells, new_cell_id};

fn libraries(cells: Cells) -> Libraries {
    Libraries::from_contributions([(
        CellId::from_u128(1),
        crate::libraries::Library::<(), ()>::named(
            CellId::from_u128(1),
            "test",
            crate::libraries::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
        ),
    )])
    .0
}

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
fn set_value_writes_fields_elements_roots_and_bare_cells() {
    let lib = core_libraries();
    let (mut doc, cell) = doc_of(vec![(
        crate::test_values::label("x"),
        crate::test_values::text("1"),
    )]);
    assert!(set_value(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("x")]),
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
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("at")]),
        Value::record([(
            crate::test_values::label("row"),
            crate::test_values::text("top")
        )]),
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[
            Step::Follow(gid::Resolution::Document),
            key("at"),
            key("row")
        ]),
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
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document)]),
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
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("x")]),
        crate::test_values::text("v")
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document)]),
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
        &crate::location::Location::root(&[key("title")]),
        crate::test_values::text("scene")
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[key("title")]),
        Some(&crate::test_values::text("scene"))
    );
    assert!(set_value(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[]),
        crate::test_values::text("root")
    ));
    assert_eq!(doc.root, Some(crate::test_values::text("root")));
    // Text is a record convention, so a structural write can
    // enrich it. The text facet remains, and so does the extra
    // field.
    assert!(set_value(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[key("x")]),
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
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("a")]),
        crate::test_values::text("2")
    ));
    assert!(!set_value(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ]),
        crate::test_values::text("mine")
    ));
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("a")])
    ));
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document)])
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
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("a")]),
        crate::test_values::text("2")
    ));
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
        &crate::location::Location::root(&[
            Step::Follow(gid::Resolution::Document),
            key("missing")
        ])
    ));

    // Unlinking a field drops the link; the linked cell floats in
    // the table for the orphan pool.
    assert!(delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document), key("child")])
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
        &crate::location::Location::root(&[
            Step::Follow(gid::Resolution::Document),
            key("dash"),
            Step::Element(ps[0].clone())
        ])
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
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document)])
    ));
    assert!(doc.cells.value(cell).is_none());
    assert!(src(&doc, &lib).resolve_path(&[]).is_some());
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[Step::Follow(gid::Resolution::Document)])
    ));

    // The empty path empties the document.
    assert!(delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[])
    ));
    assert!(doc.root.is_none());
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &crate::location::Location::root(&[])
    ));
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
fn runtime_fold_classification_matches_stored_data() {
    use grap::RuntimeValue as Runtime;
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let lib = core_libraries();
    let sources = src(&doc, &lib);
    let extra = new_cell_id();
    let cases = [
        (Runtime::record([]), None),
        (Runtime::list([]), None),
        (Runtime::list([Runtime::f64(3.0)]), Some(false)),
        (Runtime::record([(extra, Runtime::f64(3.0))]), Some(false)),
        (Runtime::f64(3.0), None),
        (
            Runtime::record([
                (
                    f64::vocabulary::F64,
                    Value::from(3.0_f64.to_le_bytes().to_vec()).into(),
                ),
                (extra, Runtime::list([])),
            ]),
            None,
        ),
        (
            Runtime::record([(text::UTF8, Value::from(b"hello".to_vec()).into())]),
            None,
        ),
        (
            Runtime::record([(text::UTF8, Value::from(vec![0xff]).into())]),
            Some(false),
        ),
        (
            Runtime::record([(f64::vocabulary::F64, Value::from(vec![0]).into())]),
            Some(false),
        ),
        (Value::from(vec![1]).into(), None),
        (Value::from(extra).into(), None),
    ];
    for (value, expected) in cases {
        assert_eq!(
            crate::selection::collapse_default_for_value(&sources, &value, false),
            expected
        );
        assert_eq!(
            crate::selection::collapse_default_for_value(&sources, &value.to_value().into(), false),
            expected,
        );
    }
}

#[test]
fn the_pending_payload_is_derived_from_the_live_editor() {
    let mut pending = crate::selection::pending_with_query(&crate::test_root(), Vec::new(), "");
    pending.set_completion_view(24.0, 2);
    pending.edit_query(|line| line.handle_ime(&puri::handler::ImeEvent::Commit("ab".to_string())));
    assert_eq!(pending.choice(), 0);
    assert_eq!(pending.completion_scroll(), 0.0);
    assert_eq!(selection_payload::query(&pending.payload()), Some("ab"));
    pending.edit_query(|line| {
        line.set_text("");
        true
    });
    assert_eq!(pending.choice(), 0);
}
