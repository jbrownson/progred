use super::*;
use crate::libraries::{f64, name};
use crate::selection::payload as selection_payload;
use crate::test_support::{core_libraries, doc_of, library_of, make_selection, src};
use crate::test_values::key;
use gid::{CellId, Cells, Document, new_cell_id};

#[test]
fn replacing_query_text_settles_completion_state_before_the_next_edit() {
    let doc = Document::default();
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
    let mut selected = Selection::from_payload(
        &crate::test_root(),
        &src(&doc, &libraries),
        Default::default(),
        vec![],
        payload,
    );
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
fn a_shadowing_document_definition_stays_editable() {
    let cell = new_cell_id();
    let mut library_cells = Cells::new();
    library_cells.set_value(cell, Value::record([]));
    let lib = library_of(library_cells);
    let root = crate::test_root();
    let mut doc = Rc::new(Document {
        root: Some(cell.into()),
        cells: Cells::new(),
    });
    assert!(pending_edge(&root, &Scope::default().view(src(&doc, &lib)), vec![]).is_none());

    Rc::make_mut(&mut doc)
        .cells
        .set_value(cell, Value::record([]));
    let pending = pending_edge(&root, &Scope::default().view(src(&doc, &lib)), vec![]).unwrap();
    assert_eq!(pending.path(), &[Step::Follow(gid::Resolution::Document)]);
    assert!(
        pending_edge(
            &root,
            &Scope::default().view(src(&doc, &lib)),
            vec![Step::Follow(gid::Resolution::Library(CellId::from_u128(1)))],
        )
        .is_none()
    );

    Rc::make_mut(&mut doc)
        .cells
        .set_value(cell, Value::list([]));
    let pending = pending_into(&root, &Scope::default().view(src(&doc, &lib)), &[], true).unwrap();
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
            crate::libraries::text::value("1"),
        )]),
    );
    let lib = library_of(library_cells);
    let bare = new_cell_id();
    let (mut doc, _) = doc_of(vec![
        (crate::test_values::label("at"), Value::record([])),
        (
            crate::test_values::label("tags"),
            Value::list([crate::libraries::text::value("x")]),
        ),
        (crate::test_values::label("lib"), Value::from(lib_cell)),
        (crate::test_values::label("material"), Value::from(bare)),
        (
            crate::test_values::label("s"),
            crate::libraries::text::value("leaf"),
        ),
    ]);
    Rc::make_mut(&mut doc).root = doc.root.clone();
    let scope = Scope::default();
    let sources = scope.view(src(&doc, &lib));

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
        true,
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
    assert_eq!(
        resolve_query("hello"),
        crate::libraries::text::value("hello")
    );
    assert_eq!(
        resolve_query("\"quoted\""),
        crate::libraries::text::value("quoted")
    );
    assert_eq!(
        resolve_query("\"open"),
        crate::libraries::text::value("open")
    );
    assert_eq!(
        resolve_query("\"0xff\""),
        crate::libraries::text::value("0xff")
    );
    assert_eq!(resolve_query("0xff00"), Value::from(vec![0xff, 0x00]));
    // Input is case-tolerant — the value is the bytes, lowercase
    // just the canonical spelling — and whole bytes only.
    assert_eq!(resolve_query("0xDEad"), Value::from(vec![0xde, 0xad]));
    assert_eq!(resolve_query("0xf"), crate::libraries::text::value("0xf"));
    assert_eq!(resolve_query("0x"), Value::from(vec![]));
}

#[test]
fn selecting_an_empty_value_slot_pends() {
    let bare = new_cell_id();
    let lib_cell = new_cell_id();
    let mut library_cells = Cells::new();
    library_cells.set_value(lib_cell, name::record("convention", []));
    let lib = library_of(library_cells);
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
        .set_value(bare, crate::libraries::text::value("v"));
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
    let empty = Document::default();
    assert_eq!(
        make_selection(vec![]).stage(&src(&empty, &lib)),
        crate::selection::Stage::Pending
    );
}

#[test]
fn the_pending_payload_is_derived_from_the_live_editor() {
    let mut pending = crate::selection::pending_with_query(
        &crate::test_root(),
        Default::default(),
        Vec::new(),
        "",
    );
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

#[test]
fn native_selection_needs_no_evaluator_and_agrees_with_grap() {
    let document = gid::Document::default();
    let empty = crate::library::Libraries::default();
    let stack = crate::stack::load();
    let site = vec![gid::Step::Key(gid::new_cell_id())];
    let child = vec![gid::Step::Element(
        gid::position::between(None, None).unwrap(),
    )];
    let destination: Vec<_> = site.iter().chain(&child).cloned().collect();
    for payload in [
        payload::edge(),
        ::grap::absent::value(crate::libraries::editor::selection::vocabulary::NO_SELECTION),
    ] {
        for selected in [None, Some(destination.clone()), Some(site.clone())] {
            let initial = || crate::site::PendingChanges {
                annotation: Some(Value::from(vec![42])),
                annotation_changed: true,
                selection: selected
                    .clone()
                    .map(|path| (path, payload::empty_pending())),
                selection_changed: true,
            };
            let mut native = initial();
            assert!(at(&child, payload.clone())(
                &crate::editing::Scope::default().view(crate::sources::Sources {
                    doc: &document,
                    libraries: &empty
                }),
                &site,
                &mut native,
            ));
            let mut interpreted = initial();
            assert!(crate::site::grap(
                crate::libraries::editor::selection::tests::grap_at(&child, payload.clone()),
                [],
            )(
                &crate::editing::Scope::default().view(crate::sources::Sources {
                    doc: &document,
                    libraries: &stack.libraries
                }),
                &site,
                &mut interpreted,
            ));
            assert_eq!(native, interpreted);
        }
    }
}
