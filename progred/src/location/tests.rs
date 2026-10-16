use super::*;
use crate::libraries::{Libraries, text};
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
