use super::*;
use crate::display::widget::navigation::Direction;

fn step(world: &mut World, direction: Direction) -> bool {
    editing_frame(world, false)
        .resolve_for_dispatch()
        .dispatch(
            world,
            puri::handler::Event::Navigate(direction),
            &mut Default::default(),
        )
        .handled()
}

/// Exercise the actual document-authored layouts, not Rust substitutes for them.
fn example_section(section: &str) -> (World, crate::gid_text::Binders) {
    let (mut doc, binders) =
        crate::gid_text::parse(crate::command::Example::Navigation.source()).unwrap();
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
    let frame = editing_frame(&mut world, false);
    for name in ["a", "b", "c", "d", "e"] {
        assert!(
            frame
                .descends
                .iter()
                .any(|d| d.path.as_ref() == field(name))
        );
    }
    // Two columns of unequal length; the second left item has nested containers.
    world.model.selection = Some(make_selection(field("a")));
    assert!(step(&mut world, Direction::Right));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("d"));
    assert!(step(&mut world, Direction::Right));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("b"));
    assert!(step(&mut world, Direction::Down));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("c"));
    assert!(step(&mut world, Direction::Up));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), field("b"));

    // Running a custom Grap layout did not turn the displayed data into a copy.
    world.model.selection = Some(make_selection(field("a")));
    assert!(step(&mut world, Direction::Right));
    let selection = world.model.selection.as_ref().unwrap();
    assert_eq!(
        selection.scope().source(selection.path()).unwrap().as_ref(),
        field("d")
    );
    assert!(
        editing_frame(&mut world, false)
            .resolve_for_dispatch()
            .dispatch_key(
                &mut world,
                &KeyboardEvent {
                    key: Key::Character("X".into()),
                    state: KeyState::Down,
                    ..Default::default()
                },
            )
    );
    assert_eq!(
        world
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_record()
            .unwrap()
            .get(&binders["d"]),
        Some(&text::value("XRight 1"))
    );
}

#[test]
fn logical_example_offset_baselines_align_navigation_with_the_declared_rows() {
    let (mut world, binders) = example_section("baseline_case");
    let field = |name: &str| vec![Step::Key(binders[name])];
    let frame = editing_frame(&mut world, false);
    let rect = |name: &str| {
        frame
            .descends
            .iter()
            .find(|d| d.path.as_ref() == field(name))
            .unwrap()
            .rect
    };
    assert!((rect("c").y0 - rect("d").y0).abs() < 0.01);
    assert!(rect("a").y1 < rect("b").y0);
    assert!(rect("b").y1 < rect("d").y0);

    for (direction, destination) in [
        (Direction::Left, "c"),
        (Direction::Right, "e"),
        (Direction::Up, "b"),
        (Direction::Down, "e"),
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
        assert!(step(&mut world, Direction::Right));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), field(name));
    }
    for name in ["d", "c", "b", "a"] {
        assert!(step(&mut world, Direction::Left));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), field(name));
    }
}

#[test]
fn logical_example_deep_multiline_cells_each_have_an_entry_step() {
    let (mut world, _) = example_section("deep");
    let mut path = vec![];
    world.model.selection = Some(make_selection(path.clone()));
    for _ in 0..3 {
        path.push(Step::Follow(gid::Resolution::Document));
        assert!(step(&mut world, Direction::Down));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    for _ in 0..3 {
        path.pop();
        assert!(step(&mut world, Direction::Up));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
}

#[test]
fn logical_down_from_a_multiline_cell_enters_the_first_line_before_the_body() {
    let cell = gid::new_cell_id();
    let root = Value::list([cell.into(), text::value("Tools")]);
    let positions = positions(&root);
    let path = vec![Step::Element(positions[0].clone())];
    let function: Path = path
        .iter()
        .cloned()
        .chain([Step::Follow(gid::Resolution::Document)])
        .collect();
    let function_name: Path = function
        .iter()
        .cloned()
        .chain([Step::Key(name::vocabulary::NAME)])
        .collect();
    let body: Path = path
        .iter()
        .cloned()
        .chain([
            Step::Follow(gid::Resolution::Document),
            Step::Key(::grap::vocabulary::BODY),
        ])
        .collect();
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        name::record(
            "cube",
            [
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
    assert!(step(&mut world, Direction::Down));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), function);
    assert!(step(&mut world, Direction::Down));
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        function_name
    );
    assert!(step(&mut world, Direction::Down));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), body);
}

#[test]
fn logical_down_enters_vertical_lists_but_passes_single_line_cells_and_lists() {
    for cell in [false, true] {
        let id = gid::new_cell_id();
        let inner = Value::list([text::value("a"), text::value("b")]);
        let mut cells = Cells::new();
        cells.set_value(
            id,
            crate::display::overlay_value(
                &crate::libraries::f64::value(45.0),
                name::record("first", []),
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
        assert!(step(&mut world, Direction::Down));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), first);
        assert!(step(&mut world, Direction::Up));
        assert!(world.model.selection.as_ref().unwrap().path().is_empty());
        world.model.selection = Some(make_selection(first));
        assert!(step(&mut world, Direction::Down));
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
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(vec![]));
    for path in &expected[1..] {
        assert!(step(&mut world, Direction::Right));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    assert!(!step(&mut world, Direction::Right));
    for path in expected[..expected.len() - 1].iter().rev() {
        assert!(step(&mut world, Direction::Left));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    assert!(!step(&mut world, Direction::Left));
}

#[test]
fn logical_navigation_retains_jump_editing_targets() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let field = gid::new_cell_id();
    let root = Value::record([
        (OUTLINE, Value::list([field.into(), field.into()])),
        (field, text::value("edit me")),
    ]);
    let entries = positions(root.as_record().unwrap().get(&OUTLINE).unwrap());
    let mut cells = Cells::new();
    cells.set_value(field, name::record("Section", []));
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells,
    });
    let heading = vec![Step::Key(OUTLINE), Step::Element(entries[0].clone())];
    let body: Path = heading.iter().cloned().chain([Step::Key(field)]).collect();
    world.model.selection = Some(make_selection(heading));
    assert!(step(&mut world, Direction::Down));
    let selection = world.model.selection.as_ref().unwrap();
    assert_eq!(selection.path(), body);
    assert_eq!(
        selection.scope().source(selection.path()).as_deref(),
        Some([Step::Key(field)].as_slice())
    );
}

#[test]
fn logical_computed_children_remain_readonly_and_text_handles_keys_first() {
    let root = Value::record([(::grap::vocabulary::EVALUATE, text::value("computed"))]);
    let source = vec![Step::Key(::grap::vocabulary::EVALUATE)];
    let result = vec![Step::Key(
        crate::libraries::presentation::vocabulary::RESULT,
    )];
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(source.clone()));
    assert!(step(&mut world, Direction::Right));
    let selection = world.model.selection.as_ref().unwrap();
    assert_eq!(selection.path(), result);
    assert!(selection.scope().source(selection.path()).is_none());

    world.model.selection = Some(make_selection(vec![]));
    let document = world.model.doc.clone();
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    let right = KeyboardEvent {
        key: Key::Named(NamedKey::ArrowRight),
        state: KeyState::Down,
        ..Default::default()
    };
    assert!(runner.keyboard_event(&right, 1.0, viewport));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        source
    );
    assert_eq!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .selection_offsets(),
        (0, 0)
    );
    assert!(runner.keyboard_event(&right, 1.0, viewport));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        source
    );
    assert_eq!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .selection_offsets(),
        (1, 1)
    );
    assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
}
