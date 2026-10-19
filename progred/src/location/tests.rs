use super::*;
use crate::libraries::{name, text};
use crate::library::Libraries;
use crate::test_support::{core_libraries, doc_of, library_of, positions, src};
use crate::test_values::key;
use gid::{Cells, Document, Value, new_cell_id};

fn sources<'a>(doc: &'a Document, libraries: &'a Libraries) -> Sources<'a> {
    Sources { doc, libraries }
}

#[test]
fn a_location_reads_from_its_start() {
    let (field, cell) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(field, text::value("inside"))]));
    let doc = Document {
        root: Some(Value::record([(field, text::value("at the root"))])),
        cells,
    };
    let libraries = Libraries::default();
    let sources = sources(&doc, &libraries);
    let path = [Step::Key(field)];
    assert_eq!(
        sources.read(&Location::root(&path[..])),
        Some(&text::value("at the root"))
    );
    assert_eq!(
        sources.read(&Location::in_cell(cell, Resolution::Document, &path[..])),
        Some(&text::value("inside"))
    );
    assert_eq!(
        sources.read(&Location::in_cell(
            cell,
            Resolution::Library(cell),
            &path[..]
        )),
        None
    );
}

#[test]
fn a_route_through_a_cell_is_stored_in_its_definition() {
    let (field, cell, inner) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(field, Value::from(inner))]));
    let doc = Document {
        root: Some(Value::record([(field, Value::from(cell))])),
        cells,
    };
    let libraries = Libraries::default();
    let sources = sources(&doc, &libraries);
    let follow = || Step::Follow(Resolution::Document);
    let route = Location::root(vec![
        Step::Key(field),
        follow(),
        Step::Key(field),
        follow(),
        Step::Key(field),
    ]);
    assert_eq!(
        route.stored(&sources),
        Some(Location::in_cell(
            inner,
            Resolution::Document,
            vec![Step::Key(field)]
        ))
    );
    let from_cell = Location::in_cell(cell, Resolution::Document, vec![Step::Key(field), follow()]);
    assert_eq!(
        from_cell.stored(&sources),
        Some(Location::in_cell(inner, Resolution::Document, Vec::new()))
    );
    let spine = Location::root(vec![Step::Key(field)]);
    assert_eq!(spine.stored(&sources), Some(spine.clone()));
    let broken = Location::root(vec![Step::Key(new_cell_id()), follow()]);
    assert_eq!(broken.stored(&sources), None);
}

#[test]
fn writes_land_where_the_document_owns_the_data() {
    let (field, cell) = (new_cell_id(), new_cell_id());
    let doc = Document {
        root: Some(Value::record([(field, Value::from(text::ID))])),
        cells: Cells::new(),
    };
    let stack = crate::stack::load();
    let sources = sources(&doc, &stack.libraries);
    assert!(writable(&sources, &Location::root(vec![Step::Key(field)])));
    assert!(writable(
        &sources,
        &Location::in_cell(cell, Resolution::Document, Vec::new())
    ));
    assert!(!writable(
        &sources,
        &Location::in_cell(text::ID, Resolution::Library(text::ID), Vec::new())
    ));
    assert!(!writable(
        &sources,
        &Location::root(vec![
            Step::Key(field),
            Step::Follow(Resolution::Library(text::ID))
        ])
    ));
    assert!(!writable(
        &sources,
        &Location::root(vec![
            Step::Key(new_cell_id()),
            Step::Follow(Resolution::Document)
        ])
    ));
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
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("x")]),
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
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("at")]),
        Value::record([(
            crate::test_values::label("row"),
            crate::test_values::text("top")
        )]),
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &Location::root(&[
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
        &Location::root(&[Step::Follow(gid::Resolution::Document)]),
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
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("x")]),
        crate::test_values::text("v")
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &Location::root(&[Step::Follow(gid::Resolution::Document)]),
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
        &Location::root(&[key("title")]),
        crate::test_values::text("scene")
    ));
    assert_eq!(
        src(&doc, &lib).resolve_path(&[key("title")]),
        Some(&crate::test_values::text("scene"))
    );
    assert!(set_value(
        &mut doc,
        &lib,
        &Location::root(&[]),
        crate::test_values::text("root")
    ));
    assert_eq!(doc.root, Some(crate::test_values::text("root")));
    // Text is a record convention, so a structural write can
    // enrich it. The text facet remains, and so does the extra
    // field.
    assert!(set_value(
        &mut doc,
        &lib,
        &Location::root(&[key("x")]),
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
    let lib = library_of(library_cells);
    let mut doc = Rc::new(Document {
        root: Some(Value::from(lib_cell)),
        cells: Cells::new(),
    });
    // The library's cell declines writes wholesale.
    assert!(!set_value(
        &mut doc,
        &lib,
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("a")]),
        crate::test_values::text("2")
    ));
    assert!(!set_value(
        &mut doc,
        &lib,
        &Location::root(&[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ]),
        crate::test_values::text("mine")
    ));
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("a")])
    ));
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &Location::root(&[Step::Follow(gid::Resolution::Document)])
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
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("a")]),
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
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("missing")])
    ));

    // Unlinking a field drops the link; the linked cell floats in
    // the table for the orphan pool.
    assert!(delete_edge(
        &mut doc,
        &lib,
        &Location::root(&[Step::Follow(gid::Resolution::Document), key("child")])
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
        &Location::root(&[
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
        &Location::root(&[Step::Follow(gid::Resolution::Document)])
    ));
    assert!(doc.cells.value(cell).is_none());
    assert!(src(&doc, &lib).resolve_path(&[]).is_some());
    assert!(!delete_edge(
        &mut doc,
        &lib,
        &Location::root(&[Step::Follow(gid::Resolution::Document)])
    ));

    // The empty path empties the document.
    assert!(delete_edge(&mut doc, &lib, &Location::root(&[])));
    assert!(doc.root.is_none());
    assert!(!delete_edge(&mut doc, &lib, &Location::root(&[])));
}
