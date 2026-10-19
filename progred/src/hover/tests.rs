use super::*;
use crate::display::widget::offers::Entry;
use crate::location::Location;
use crate::sources::Sources;
use gid::{CellId, Cells, Document, Value, new_cell_id};

#[test]
fn detached_drawing_has_no_input_source_but_still_traces_called_cells() {
    let path = vec![Step::Key(new_cell_id())];
    assert_eq!(
        from_grap(grap::SourceOrigin::Input(path.clone()), None),
        None
    );
    let cell = new_cell_id();
    let resolution = gid::Resolution::Document;
    assert_eq!(
        from_grap(
            grap::SourceOrigin::Cell {
                cell,
                resolution,
                path: path.clone()
            },
            None
        ),
        Some(SourceTrace::InCell {
            cell,
            resolution,
            path: path.into()
        })
    );
}

fn secondary(sources: &Sources, path: Vec<Step>) -> Option<Secondary> {
    sources
        .resolve_path(&path)
        .map(|value| match value.as_cell() {
            Some(cell) => Secondary::Cell(cell),
            None => Secondary::from_location(sources, &Location::root(path.as_slice())),
        })
}

#[test]
fn definitions_have_distinct_source_traces_and_secondary_marks() {
    let cell = new_cell_id();
    let field = new_cell_id();
    let library_ids = [new_cell_id(), new_cell_id()];
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(field, Value::from(vec![1]))]));
    let root = Value::list([Value::from(cell), Value::from(cell)]);
    let positions: Vec<_> = root.as_list().unwrap().keys().cloned().collect();
    let doc = Document {
        root: Some(root),
        cells,
    };
    let libraries = |order: [CellId; 2]| {
        crate::library::Libraries::from_contributions(order.map(|id| {
            (
                id,
                crate::libraries::name::named_library::<crate::Editor>(
                    id,
                    "source",
                    crate::library::Definitions::from_parts(
                        doc.cells.clone(),
                        grap::ForeignFunctions::default(),
                    ),
                ),
            )
        }))
        .0
    };
    let path = |position: usize, resolution| {
        vec![
            Step::Element(positions[position].clone()),
            Step::Follow(resolution),
            Step::Key(field),
        ]
    };
    let definitions = [
        gid::Resolution::Document,
        gid::Resolution::Library(library_ids[0]),
        gid::Resolution::Library(library_ids[1]),
    ];
    let first = libraries(library_ids);
    let reordered = libraries([library_ids[1], library_ids[0]]);
    let original = Sources {
        doc: &doc,
        libraries: &first,
    };
    for libraries in [&first, &reordered] {
        let sources = Sources {
            doc: &doc,
            libraries,
        };
        for left in definitions {
            let left_path = path(0, left);
            let trace = SourceTrace::from_location(&sources, &Location::root(&left_path[..]));
            let input = SourceTrace::from_location(&sources, &Location::root(&left_path[..2]));
            assert_eq!(
                Some(trace.clone()),
                crate::hover::from_grap(
                    grap::SourceOrigin::Input(vec![Step::Key(field)]),
                    Some(&input)
                )
            );
            assert_eq!(
                Some(Secondary::from_trace(&trace)),
                secondary(&sources, left_path.clone())
            );
            assert_eq!(
                trace,
                SourceTrace::from_location(&original, &Location::root(&left_path[..]))
            );
            for right in definitions {
                let right_path = path(1, right);
                assert_eq!(
                    sources.resolve_path(&left_path),
                    sources.resolve_path(&right_path)
                );
                assert_eq!(
                    trace == SourceTrace::from_location(&sources, &Location::root(&right_path[..])),
                    left == right,
                );
                assert_eq!(
                    secondary(&sources, left_path.clone()) == secondary(&sources, right_path),
                    left == right,
                );
            }
        }
        assert_eq!(
            secondary(&sources, vec![Step::Element(positions[0].clone())]),
            secondary(&sources, vec![Step::Element(positions[1].clone())]),
        );
    }
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
            Value::record([(inner, crate::test_values::text("asdf"))]),
        ),
        (peer, crate::test_values::text("asdf")),
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
            Step::Follow(gid::Resolution::Document),
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
                Step::Follow(gid::Resolution::Document),
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
            detail: None,
            matches: vec![],
            face: crate::display::Face::Label,
            source: value.as_cell(),
            activate: Rc::new(|_: &mut crate::Editor| {}),
        }]),
    };

    assert_eq!(
        hover_secondary(None, Some(&offers(Value::from(cell))), &Hover::Entry(0)),
        Some(Secondary::Cell(cell))
    );
    assert_eq!(
        hover_secondary(
            None,
            Some(&offers(crate::test_values::text("offer"))),
            &Hover::Entry(0)
        ),
        None
    );
    assert_eq!(hover_secondary::<()>(None, None, &Hover::Entry(0)), None);
}
