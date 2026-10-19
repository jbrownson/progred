use super::*;

#[test]
fn website_lessons_are_editable_data_without_orphan_definitions() {
    for source in [
        include_str!("../../../website/public/lessons/shape.gid"),
        include_str!("../../../website/public/lessons/growing-forest.gid"),
        include_str!("../../../website/public/lessons/dot.gid"),
        include_str!("../../../website/public/lessons/copies.gid"),
        include_str!("../../../website/public/lessons/recipe.gid"),
        include_str!("../../../website/public/lessons/trunks.gid"),
        include_str!("../../../website/public/lessons/heights.gid"),
        include_str!("../../../website/public/lessons/growth.gid"),
        include_str!("../../../website/public/lessons/counted.gid"),
        include_str!("../../../website/public/lessons/names.gid"),
        include_str!("../../../website/public/lessons/bytes.gid"),
        include_str!("../../../website/public/lessons/numbers.gid"),
        include_str!("../../../website/public/lessons/unfinished.gid"),
        include_str!("../../../website/public/lessons/views.gid"),
    ] {
        let (doc, _) = parse(source).unwrap();
        let reached = root_reachable_cells(&doc);
        assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
    }
    let (doc, binders) = parse(include_str!("../../../website/public/lessons/cells.gid")).unwrap();
    assert_eq!(
        doc.root,
        Some(Value::list([
            binders["shared"].into(),
            binders["shared"].into()
        ]))
    );
    assert_eq!(
        doc.cells
            .value(binders["shared"])
            .and_then(crate::libraries::f64::read),
        Some(7.0)
    );
    let reached = root_reachable_cells(&doc);
    assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
}

/// The planet record the editor's own tests edit.
#[test]
fn values_fixture_is_a_planet_record() {
    let (doc, binders) = parse(include_str!("../../fixtures/values.gid")).unwrap();
    let root = doc.root.as_ref().unwrap().as_record().unwrap();
    assert_eq!(
        crate::libraries::text::read(root.get(&binders["planet"]).unwrap()),
        Some("Mars")
    );
    assert_eq!(
        crate::libraries::f64::read(root.get(&binders["moons"]).unwrap()),
        Some(2.0)
    );
    assert_eq!(
        root.get(&binders["colors"])
            .and_then(Value::as_list)
            .map(|colors| colors
                .values()
                .filter_map(crate::libraries::text::read)
                .collect::<Vec<_>>()),
        Some(vec!["red", "orange"])
    );
    let reached = root_reachable_cells(&doc);
    assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
}

#[test]
fn grap_fixture_shares_an_unnamed_numeric_cell() {
    let (doc, binders) = parse(include_str!("../../fixtures/grap.gid")).unwrap();
    assert_eq!(doc.cells.cells().count(), 1);
    let input = doc.cells.value(binders["input"]).unwrap();
    assert_eq!(crate::libraries::f64::read(input), Some(3.0));
    assert_eq!(crate::libraries::name::read(input), None);
    let root = doc.root.as_ref().unwrap().as_record().unwrap();
    assert_eq!(root.len(), 3);
    assert_eq!(root.get(&binders["first"]), Some(&binders["input"].into()));
    for key in ["second", "third"] {
        let item = root.get(&binders[key]).unwrap();
        let call = item
            .as_record()
            .unwrap()
            .get(&grap::vocabulary::EVALUATE)
            .unwrap()
            .as_record()
            .unwrap();
        assert_eq!(
            call.get(&crate::libraries::number::vocabulary::LEFT),
            Some(&binders["input"].into())
        );
        let argument = call
            .get(&crate::libraries::number::vocabulary::RIGHT)
            .unwrap();
        assert_eq!(crate::libraries::f64::read(argument), Some(2.0));
        assert_eq!(crate::libraries::name::read(argument), None);
    }
    let reached = root_reachable_cells(&doc);
    assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
}

#[test]
fn functions_fixture_has_a_parameter_not_a_shared_numeric_input() {
    use crate::libraries::{f64, name};
    use grap::vocabulary::{BODY, EVALUATE, FUNCTION, PARAMS};

    let (doc, names) = parse(include_str!("../../fixtures/functions.gid")).unwrap();
    assert_eq!(doc.cells.cells().count(), 2);
    assert_eq!(doc.cells.value(names["x"]), Some(&name::record("x", [])));
    let definition = doc
        .cells
        .value(names["scale"])
        .unwrap()
        .as_record()
        .unwrap();
    assert_eq!(
        definition.get(&PARAMS),
        Some(&Value::list([names["x"].into()]))
    );
    let body = definition.get(&BODY).unwrap().as_record().unwrap();
    assert_eq!(
        body.get(&crate::libraries::number::vocabulary::LEFT),
        Some(&names["x"].into())
    );
    let root = doc.root.as_ref().unwrap().as_record().unwrap();
    assert_eq!(root.len(), 3);
    assert_eq!(root.get(&names["first"]), Some(&names["scale"].into()));
    for (key, argument) in [("second", 3.0), ("third", 5.0)] {
        let item = root.get(&names[key]).unwrap();
        let call = item
            .as_record()
            .unwrap()
            .get(&EVALUATE)
            .unwrap()
            .as_record()
            .unwrap();
        assert_eq!(call.get(&FUNCTION), Some(&names["scale"].into()));
        assert_eq!(call.get(&names["x"]).and_then(f64::read), Some(argument));
    }
    let reached = root_reachable_cells(&doc);
    assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
}

fn root_reachable_cells(doc: &Document) -> HashSet<CellId> {
    fn enqueue(cell: CellId, reached: &mut HashSet<CellId>, pending: &mut Vec<CellId>) {
        if reached.insert(cell) {
            pending.push(cell);
        }
    }

    fn visit(value: &Value, reached: &mut HashSet<CellId>, pending: &mut Vec<CellId>) {
        match value {
            Value::Cell(cell) => enqueue(*cell, reached, pending),
            Value::Blob(_) => {}
            Value::List(elements) => {
                for value in elements.values() {
                    visit(value, reached, pending);
                }
            }
            Value::Record(fields) => {
                for (label, value) in fields {
                    enqueue(*label, reached, pending);
                    visit(value, reached, pending);
                }
            }
        }
    }

    let mut reached = HashSet::new();
    let mut pending = Vec::new();
    if let Some(root) = &doc.root {
        visit(root, &mut reached, &mut pending);
    }
    while let Some(cell) = pending.pop() {
        if let Some(value) = doc.cells.value(cell) {
            visit(value, &mut reached, &mut pending);
        }
    }
    reached
}

/// The checked-in sample is canonical: it parses, and printing
/// it back is the identity — the printer's golden fixture.
#[test]
fn the_sample_file_is_a_fixed_point() {
    let text = include_str!("../../../examples/sample.gid");
    let (doc, binders) = parse(text).expect("the sample parses");
    assert!(doc.root.is_some());
    assert_eq!(print(&doc, &binders), text);
}

#[test]
fn every_sample_cell_definition_is_root_reachable() {
    let (doc, _) = parse(include_str!("../../../examples/sample.gid")).expect("the sample parses");
    let reached = root_reachable_cells(&doc);
    let mut orphans: Vec<_> = doc
        .cells
        .cells()
        .filter(|cell| !reached.contains(cell))
        .copied()
        .collect();
    orphans.sort();
    assert!(orphans.is_empty(), "orphan cell definitions: {orphans:?}");
}

#[test]
fn the_grap_demo_is_a_fixed_point() {
    let text = include_str!("../../../examples/grap-demo.gid");
    let (doc, binders) = parse(text).expect("the Grap demo parses");
    assert_eq!(print(&doc, &binders), text);
}

#[test]
fn the_iop_tree_demo_is_a_fixed_point_without_orphans() {
    let text = include_str!("../../../examples/iop-tree.gid");
    let (doc, binders) = parse(text).expect("the IoP tree demo parses");
    assert_eq!(print(&doc, &binders), text);
    let reached = root_reachable_cells(&doc);
    assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
}

#[test]
fn the_fidget_demo_is_a_fixed_point() {
    let text = include_str!("../../../examples/fidget.gid");
    let (doc, binders) = parse(text).expect("the Fidget demo parses");
    assert_eq!(print(&doc, &binders), text);
}

#[test]
fn complex_fidget_examples_are_fixed_points_without_orphans() {
    use crate::command::Example;
    for (name, source) in crate::test_examples::FIDGET.into_iter().chain([
        ("shapes", Example::Fidget.source()),
        ("toolpaths", Example::Toolpaths.source()),
    ]) {
        let (doc, binders) = parse(source).expect("the example parses");
        assert_eq!(print(&doc, &binders), source, "{name}");
        let reached = root_reachable_cells(&doc);
        assert!(
            doc.cells.cells().all(|cell| reached.contains(cell)),
            "{name}"
        );
    }
}
