//! Arrows over document-authored layouts.

use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::driver::Driver;
use crate::frame::test_support::editing_frame;
use crate::libraries::{name, text};
use crate::location::Location;
use crate::test_support::{make_selection, positions};
use gid::{Cells, Document, Path, Step, Value};
use puri::handler::NavigationDirection;
use std::rc::Rc;
use ui_events::keyboard::NamedKey;

type World = crate::Editor;

fn step(world: &mut World, direction: NavigationDirection) -> bool {
    editing_frame(world).navigate(world, direction)
}

/// Exercise the actual document-authored layouts, not Rust substitutes for them.
fn example_section(section: &str) -> (World, crate::gid_text::Binders) {
    let (mut doc, binders) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
    doc.root = Some(
        doc.root
            .as_ref()
            .unwrap()
            .as_record()
            .unwrap()
            .get(&binders[section])
            .unwrap()
            .clone(),
    );
    let world = crate::test_editor(doc);
    (world, binders)
}

#[test]
fn logical_example_custom_columns_interleave_lines_and_keep_editable_sources() {
    let (mut world, binders) = example_section("side_by_side");
    let field = |name: &str| vec![Step::Key(binders[name])];
    let frame = editing_frame(&mut world);
    for name in ["a", "b", "c", "d", "e"] {
        assert!(frame.landmarks.landmark(&field(name)).is_some());
    }
    // Two columns of unequal length; the second left item has nested containers.
    world.model.selection = Some(make_selection(field("a")));
    assert!(step(&mut world, NavigationDirection::Right));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("d"));
    assert!(step(&mut world, NavigationDirection::Right));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("b"));
    assert!(step(&mut world, NavigationDirection::Down));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("c"));
    assert!(step(&mut world, NavigationDirection::Up));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("b"));

    // Running a custom Grap layout did not turn the displayed data into a copy.
    world.model.selection = Some(make_selection(field("a")));
    assert!(step(&mut world, NavigationDirection::Right));
    assert_eq!(
        world.model.selection.as_ref().unwrap().location(),
        Some(Location::root(field("d")))
    );
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.type_text("X");
    assert_eq!(driver.value(&field("d")), text::value("XRight 1"));
}

#[test]
fn logical_example_offset_baselines_align_navigation_with_the_declared_rows() {
    let (mut world, binders) = example_section("baseline_case");
    let field = |name: &str| vec![Step::Key(binders[name])];
    let frame = editing_frame(&mut world);
    let rect = |name: &str| frame.landmarks.landmark(&field(name)).unwrap().rect;
    assert!((rect("c").y0 - rect("d").y0).abs() < 0.01);
    assert!(rect("a").y1 < rect("b").y0);
    assert!(rect("b").y1 < rect("d").y0);

    for (direction, destination) in [
        (NavigationDirection::Left, "c"),
        (NavigationDirection::Right, "e"),
        (NavigationDirection::Up, "b"),
        (NavigationDirection::Down, "e"),
    ] {
        world.model.selection = Some(make_selection(field("d")));
        assert!(step(&mut world, direction));
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            field(destination)
        );
    }
    world.model.selection = Some(make_selection(field("a")));
    for name in ["b", "c", "d", "e"] {
        assert!(step(&mut world, NavigationDirection::Right));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), field(name));
    }
    for name in ["d", "c", "b", "a"] {
        assert!(step(&mut world, NavigationDirection::Left));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), field(name));
    }
}

#[test]
fn logical_example_deep_multiline_cells_share_a_vertical_entrance() {
    let (mut world, binders) = example_section("deep");
    let outline = crate::libraries::presentation::vocabulary::SECTIONS;
    let first = positions(
        world
            .model
            .doc
            .cells
            .value(binders["deep_inner"])
            .unwrap()
            .as_record()
            .unwrap()
            .get(&outline)
            .unwrap(),
    )[0]
    .clone();
    let content: Path = std::iter::repeat_n(Step::Follow, 3)
        .chain([Step::Key(outline), Step::Element(first)])
        .collect();
    let mut path = vec![];
    world.model.selection = Some(make_selection(path.clone()));
    // Horizontal traversal still visits each enclosing value.
    for _ in 0..3 {
        path.push(Step::Follow);
        assert!(step(&mut world, NavigationDirection::Right));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    // Every enclosing value enters the same first content line vertically.
    for depth in 0..=3 {
        world.model.selection = Some(make_selection(path[..depth].to_vec()));
        assert!(step(&mut world, NavigationDirection::Down));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), content);
        assert!(step(&mut world, NavigationDirection::Up));
        assert!(world.model.selection.as_ref().unwrap().path().is_empty());
    }
}

#[test]
fn logical_down_from_a_multiline_cell_enters_the_first_line_before_the_body() {
    let cell = gid::new_cell_id();
    let root = Value::list([cell.into(), text::value("Tools")]);
    let positions = positions(&root);
    let path = vec![Step::Element(positions[0].clone())];
    let function: Path = path.iter().cloned().chain([Step::Follow]).collect();
    let function_name: Path = function
        .iter()
        .cloned()
        .chain([Step::Key(name::vocabulary::NAME)])
        .collect();
    let body: Path = path
        .iter()
        .cloned()
        .chain([Step::Follow, Step::Key(::grap::vocabulary::BODY)])
        .collect();
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        name::record(
            "cube",
            [
                (
                    ::grap::isa::ISA,
                    gid::Value::from(::grap::vocabulary::kind::LAMBDA).into(),
                ),
                (::grap::vocabulary::PARAMS, Value::list([])),
                (
                    ::grap::vocabulary::BODY,
                    text::value("a long function body ".repeat(40)),
                ),
            ],
        ),
    );
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells,
    });
    world.model.selection = Some(make_selection(path.clone()));
    assert!(step(&mut world, NavigationDirection::Right));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), function);
    for from in [path.clone(), function] {
        world.model.selection = Some(make_selection(from));
        assert!(step(&mut world, NavigationDirection::Down));
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            function_name
        );
        assert!(step(&mut world, NavigationDirection::Up));
        // The containing list also shares this leading wrapper-only level.
        assert!(world.model.selection.as_ref().unwrap().path().is_empty());
        world.model.selection = Some(make_selection(function_name.clone()));
        assert!(step(&mut world, NavigationDirection::Down));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), body);
    }
}

#[test]
fn arrows_enter_an_anonymous_lambda_at_its_name_slot_without_opening_completion() {
    use NavigationDirection::{Down, Right};
    let mut world = crate::editor_of(grap::lambda(
        [],
        text::value("a long function body ".repeat(40)),
    ));
    world.model.selection = Some(make_selection(vec![]));
    // The λ is the missing name's place, typed into as a line, so arriving
    // there opens no completion.
    for (direction, field) in [
        (Down, crate::libraries::name::vocabulary::NAME),
        (Right, grap::vocabulary::PARAMS),
        (Down, grap::vocabulary::BODY),
    ] {
        assert!(step(&mut world, direction));
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            [Step::Key(field)]
        );
        assert!(editing_frame(&mut world).completion.is_none());
    }
}

#[test]
fn logical_down_enters_vertical_lists_but_passes_single_line_cells_and_lists() {
    for cell in [false, true] {
        let id = gid::new_cell_id();
        let inner = Value::list([text::value("a"), text::value("b")]);
        let mut cells = Cells::new();
        cells.set_value(
            id,
            name::record(
                "first",
                crate::libraries::f64::value(45.0)
                    .as_record()
                    .unwrap()
                    .clone(),
            ),
        );
        let root = Value::list([
            if cell { id.into() } else { inner },
            text::value("last ".repeat(80)),
        ]);
        let items = positions(&root);
        let first = vec![Step::Element(items[0].clone())];
        let second = vec![Step::Element(items[1].clone())];
        let mut world = crate::test_editor(Document {
            root: Some(root),
            cells,
        });
        world.model.selection = Some(make_selection(vec![]));
        assert!(step(&mut world, NavigationDirection::Down));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), first);
        assert!(step(&mut world, NavigationDirection::Up));
        assert!(world.model.selection.as_ref().unwrap().path().is_empty());
        world.model.selection = Some(make_selection(first));
        assert!(step(&mut world, NavigationDirection::Down));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), second);
    }
}

#[test]
fn logical_reading_order_visits_container_stops_and_wraps_between_lines() {
    let inner = Value::list([text::value("a"), text::value("b")]);
    let inner_positions = positions(&inner);
    let root = Value::list([inner, text::value("last ".repeat(80))]);
    let outer = positions(&root);
    let expected = [
        vec![],
        vec![Step::Element(outer[0].clone())],
        vec![
            Step::Element(outer[0].clone()),
            Step::Element(inner_positions[0].clone()),
        ],
        vec![
            Step::Element(outer[0].clone()),
            Step::Element(inner_positions[1].clone()),
        ],
        vec![Step::Element(outer[1].clone())],
    ];
    let mut world = crate::editor_of(root);
    world.model.selection = Some(make_selection(vec![]));
    for path in &expected[1..] {
        assert!(step(&mut world, NavigationDirection::Right));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    assert!(!step(&mut world, NavigationDirection::Right));
    for path in expected[..expected.len() - 1].iter().rev() {
        assert!(step(&mut world, NavigationDirection::Left));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    assert!(!step(&mut world, NavigationDirection::Left));
}

#[test]
fn logical_navigation_retains_jump_editing_targets() {
    use crate::libraries::presentation::vocabulary::{SECTIONS, kind};
    let field = gid::new_cell_id();
    let root = Value::record([
        (::grap::isa::ISA, kind::OUTLINE.into()),
        (SECTIONS, Value::list([field.into(), field.into()])),
        (field, text::value("edit me")),
    ]);
    let entries = positions(root.as_record().unwrap().get(&SECTIONS).unwrap());
    let mut cells = Cells::new();
    cells.set_value(field, name::record("Section", []));
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells,
    });
    let heading = vec![Step::Key(SECTIONS), Step::Element(entries[0].clone())];
    // The body is drawn below its heading, jumped to the field it shows.
    let body = crate::occurrence::of(&heading)
        .into_iter()
        .chain([
            crate::occurrence::Step::Key(field),
            crate::occurrence::Step::jump(Location::root(vec![Step::Key(field)])),
        ])
        .collect::<crate::occurrence::Path>();
    world.model.selection = Some(make_selection(heading));
    assert!(step(&mut world, NavigationDirection::Down));
    let selection = world.model.selection.as_ref().unwrap();
    assert_eq!(selection.path(), body);
    assert_eq!(
        selection.location(),
        Some(Location::root(vec![Step::Key(field)]))
    );
}

#[test]
fn logical_computed_children_remain_readonly_and_text_handles_keys_first() {
    let root = crate::libraries::grap::evaluation(text::value("computed"));
    let source = vec![Step::Key(::grap::vocabulary::EXPRESSION)];
    let mut world = crate::editor_of(root);
    world.model.selection = Some(make_selection(source.clone()));
    assert!(step(&mut world, NavigationDirection::Right));
    let selection = world.model.selection.as_ref().unwrap();
    assert_eq!(selection.path(), crate::test_examples::result_path(&[]));
    assert!(selection.location().is_none());

    let document = world.model.doc.clone();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select::<Step>(&[]);
    for offset in [0, 1] {
        driver.press(NamedKey::ArrowRight);
        assert_eq!(driver.selection(), source);
        assert_eq!(driver.line_selection(), Some((offset, offset)));
    }
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
}
