use super::*;
use crate::display::widget::offers::Entry;
use crate::location::Location;
use crate::sources::Sources;
use gid::{Cells, Document, Step, Value, new_cell_id};
use std::rc::Rc;

#[test]
fn detached_drawing_has_no_input_source_but_still_traces_called_cells() {
    let path = vec![Step::Key(new_cell_id())];
    assert_eq!(
        from_grap(grap::SourceOrigin::Input(path.clone()), None),
        None
    );
    let cell = new_cell_id();
    assert_eq!(
        from_grap(
            grap::SourceOrigin::Cell {
                cell,
                path: path.clone()
            },
            None
        ),
        Some(SourceTrace::InCell {
            cell,
            path: path.into()
        })
    );
}

fn secondary(sources: &Sources, path: Vec<Step>) -> Option<Secondary> {
    sources
        .resolve_path(&path)
        .map(|value| match value.as_cell() {
            Some(cell) => Secondary::Cell(cell),
            None => Secondary::from_location(
                sources,
                &Location::root(path.as_slice()),
                &crate::test_document(),
            ),
        })
}

#[test]
fn a_definition_has_one_source_trace_and_secondary_mark_however_its_reached() {
    let cell = new_cell_id();
    let field = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(field, Value::from(vec![1]))]));
    let root = Value::list([Value::from(cell), Value::from(cell)]);
    let positions: Vec<_> = root.as_list().unwrap().keys().cloned().collect();
    let doc = Document {
        root: Some(root),
        cells,
    };
    let libraries = crate::library::Libraries::default();
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let path = |position: usize| {
        vec![
            Step::Element(positions[position].clone()),
            Step::Follow,
            Step::Key(field),
        ]
    };
    let (left, right) = (path(0), path(1));
    let trace = SourceTrace::from_location(&sources, &Location::root(&left[..]));
    let input = SourceTrace::from_location(&sources, &Location::root(&left[..2]));
    assert_eq!(
        Some(trace.clone()),
        crate::hover::from_grap(
            grap::SourceOrigin::Input(vec![Step::Key(field)]),
            Some(&input)
        )
    );
    assert_eq!(
        Some(Secondary::from_trace(&trace, &crate::test_document())),
        secondary(&sources, left.clone())
    );
    assert_eq!(
        trace,
        SourceTrace::from_location(&sources, &Location::root(&right[..]))
    );
    assert_eq!(secondary(&sources, left), secondary(&sources, right));
    assert_eq!(
        secondary(&sources, vec![Step::Element(positions[0].clone())]),
        secondary(&sources, vec![Step::Element(positions[1].clone())]),
    );
}

#[test]
fn nested_values_match_by_cell_and_relative_path() {
    let shared = new_cell_id();
    let other = new_cell_id();
    let outer = crate::test_values::label("outer");
    let inner = crate::test_values::label("inner");
    let peer = crate::test_values::label("peer");
    let contents = Value::record([
        (
            outer,
            Value::record([(inner, crate::libraries::text::value("asdf"))]),
        ),
        (peer, crate::libraries::text::value("asdf")),
    ]);
    let mut cells = Cells::new();
    cells.set_value(shared, contents.clone());
    cells.set_value(other, contents);
    let root = Value::list([Value::from(shared), Value::from(shared), Value::from(other)]);
    let positions: Vec<_> = root.as_list().expect("root list").keys().cloned().collect();
    let doc = Document {
        root: Some(root),
        cells,
    };
    let libraries = crate::library::Libraries::default();
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let nested = |position| {
        vec![
            Step::Element(position),
            Step::Follow,
            Step::Key(outer),
            Step::Key(inner),
        ]
    };

    assert_eq!(
        secondary(&sources, nested(positions[0].clone())),
        secondary(&sources, nested(positions[1].clone()))
    );
    assert_ne!(
        secondary(&sources, nested(positions[0].clone())),
        secondary(
            &sources,
            vec![
                Step::Element(positions[1].clone()),
                Step::Follow,
                Step::Key(peer),
            ],
        )
    );
    assert_ne!(
        secondary(&sources, nested(positions[0].clone())),
        secondary(&sources, nested(positions[2].clone()))
    );
    assert_eq!(
        secondary(&sources, vec![Step::Element(positions[0].clone())]),
        secondary(&sources, vec![Step::Element(positions[1].clone())])
    );
}

#[test]
fn entry_hover_marks_follow_the_visible_offers() {
    let cell = new_cell_id();
    let offers = |value: Value| Offers {
        entries: Rc::from([Entry {
            display: "offer".to_string(),
            subscripts: Vec::new(),
            detail: None,
            matches: vec![],
            face: crate::display::Face::Label,
            source: value.as_cell(),
            activate: Rc::new(|_: &mut crate::Editor| {}),
        }]),
        chosen: 0,
    };

    assert_eq!(
        hover_secondary(
            None,
            None,
            Some(&offers(Value::from(cell))),
            &Hover::Entry(0)
        ),
        Some(Secondary::Cell(cell))
    );
    assert_eq!(
        hover_secondary(
            None,
            None,
            Some(&offers(crate::libraries::text::value("offer"))),
            &Hover::Entry(0)
        ),
        None
    );
    assert_eq!(
        hover_secondary::<()>(None, None, None, &Hover::Entry(0)),
        None
    );
}
