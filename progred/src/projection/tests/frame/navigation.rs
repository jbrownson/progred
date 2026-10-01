//! Layout navigation through real projections and the ordinary event chain.
use super::*;
use crate::display::{self as d, widget::navigation as nav};
use nav::Direction;

fn sample_projection() -> Projection<World> {
    Projection::new([d::runtime_partial(|input| {
        if let Some(items) = d::structure::list_items(input, None) {
            let items: Vec<_> = items.into_iter().map(|(_, layout)| layout).collect();
            Some(d::alternatives([
                nav::nav_group(d::row(8.0, items.clone())),
                nav::nav_group(d::col(0, 8.0, items)),
            ]))
        } else {
            crate::libraries::text::display(input)
        }
    })])
}

fn fixture(wide: bool) -> (World, Vec<Path>) {
    let nested = Value::list([text::value("first"), text::value("second")]);
    let root = Value::list([
        nested,
        text::value(if wide {
            "last ".repeat(100)
        } else {
            "last".into()
        }),
    ]);
    let outer: Vec<_> = root.as_list().unwrap().keys().cloned().collect();
    let inner: Vec<_> = root
        .as_list()
        .unwrap()
        .get(&outer[0])
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let paths = vec![
        vec![],
        vec![Step::Element(outer[0].clone())],
        vec![
            Step::Element(outer[0].clone()),
            Step::Element(inner[0].clone()),
        ],
        vec![
            Step::Element(outer[0].clone()),
            Step::Element(inner[1].clone()),
        ],
        vec![Step::Element(outer[1].clone())],
    ];
    (
        crate::test_editor(Document {
            root: Some(root),
            cells: Cells::new(),
        }),
        paths,
    )
}

#[test]
fn navigation_destinations_follow_nested_container_boundaries_and_chosen_layout() {
    use Direction::*;
    for wide in [false, true] {
        let (mut world, paths) = fixture(wide);
        let projection = sample_projection();
        let expected = if wide {
            [
                [None, Some(1), None, Some(1)],
                [Some(0), Some(2), Some(0), Some(4)],
                [Some(1), Some(3), Some(0), Some(4)],
                [Some(2), Some(4), Some(0), Some(4)],
                [Some(3), None, Some(1), None],
            ]
        } else {
            [
                [None, Some(1), None, None],
                [Some(0), Some(2), None, None],
                [Some(1), Some(3), None, None],
                [Some(2), Some(4), None, None],
                [Some(3), None, None, None],
            ]
        };
        for (from, destinations) in expected.iter().enumerate() {
            for (direction, to) in [Left, Right, Up, Down].into_iter().zip(destinations) {
                world.model.selection = Some(make_selection(paths[from].clone()));
                let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
                let handled = frame.navigate(&mut world, direction);
                assert_eq!(
                    handled,
                    to.is_some(),
                    "wide={wide}, from={from}, {direction:?}"
                );
                assert_eq!(
                    world.model.selection.as_ref().unwrap().path(),
                    paths[to.unwrap_or(from)]
                );
            }
        }
    }
}

#[test]
fn containers_follow_logical_lines_including_cells_around_multiline_contents() {
    use Direction::*;
    for record in [false, true] {
        for in_cell in [false, true] {
            for vertical in [false, true] {
                let (wrapper, a, b, cell) =
                    (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
                let mut cells = Cells::new();
                cells.set_value(a, name::record("A", []));
                cells.set_value(b, name::record("B", []));
                let first = text::value(if vertical {
                    "long ".repeat(100)
                } else {
                    "first".into()
                });
                let container = if record {
                    Value::record([(a, first), (b, text::value("last"))])
                } else {
                    Value::list([first, text::value("last")])
                };
                let child_steps = if record {
                    vec![Step::Key(a), Step::Key(b)]
                } else {
                    positions(&container)
                        .into_iter()
                        .map(Step::Element)
                        .collect()
                };
                let item = if in_cell {
                    cells.set_value(cell, container);
                    Value::Cell(cell)
                } else {
                    container
                };
                let siblings = Value::list([text::value("before"), item, text::value("after")]);
                let paths: Vec<Path> = positions(&siblings)
                    .into_iter()
                    .map(|p| vec![Step::Key(wrapper), Step::Element(p)])
                    .collect();
                let body: Path = paths[1]
                    .iter()
                    .cloned()
                    .chain(in_cell.then_some(Step::Follow(gid::Resolution::Document)))
                    .collect();
                let children: Vec<Path> = child_steps
                    .into_iter()
                    .map(|step| body.iter().cloned().chain([step]).collect())
                    .collect();
                let mut world = crate::test_editor(Document {
                    root: Some(Value::record([(wrapper, siblings)])),
                    cells,
                });
                let ordinary = world.stack.projection.partial.clone();
                let projection = Projection::new([
                    d::runtime_partial(move |input| {
                        input.value?.field(wrapper)?;
                        Some(d::descend(
                            Step::Key(wrapper),
                            Some(d::structure::list_column(8.0, None)),
                            None,
                        ))
                    }),
                    ordinary,
                ]);
                let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
                let rect = |path: &Path| {
                    frame
                        .descends
                        .iter()
                        .find(|stop| stop.path.as_ref() == path)
                        .unwrap()
                        .rect
                };
                assert_eq!(rect(&children[1]).y0 > rect(&children[0]).y1, vertical);
                let (forward, backward, across) = if vertical {
                    (Down, Up, Left)
                } else {
                    (Right, Left, Up)
                };
                let checks = [
                    (&body, forward, Some(&children[0])),
                    (
                        &children[0],
                        backward,
                        Some(if vertical { &paths[1] } else { &body }),
                    ),
                    (
                        &children[0],
                        across,
                        Some(if vertical { &body } else { &paths[0] }),
                    ),
                    (
                        &paths[1],
                        if in_cell { Up } else { across },
                        Some(&paths[0]),
                    ),
                    (
                        &paths[1],
                        Right,
                        Some(if in_cell { &body } else { &children[0] }),
                    ),
                    (
                        &paths[1],
                        Down,
                        Some(if !vertical { &paths[2] } else { &children[0] }),
                    ),
                    (
                        &children[0],
                        Down,
                        Some(if vertical { &children[1] } else { &paths[2] }),
                    ),
                    (
                        &paths[2],
                        Up,
                        Some(if vertical { &children[1] } else { &paths[1] }),
                    ),
                ];
                let before = world.model.doc.clone();
                for (from, direction, to) in checks {
                    world.model.selection = Some(make_selection(from.clone()));
                    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
                    let handled = frame.navigate(&mut world, direction);
                    assert_eq!(
                        handled,
                        to.is_some(),
                        "record={record}, cell={in_cell}, vertical={vertical}, {direction:?}"
                    );
                    assert_eq!(
                        world.model.selection.as_ref().unwrap().path(),
                        to.unwrap_or(from),
                        "record={record}, cell={in_cell}, vertical={vertical}, {direction:?}"
                    );
                    assert!(Rc::ptr_eq(&world.model.doc, &before));
                }
            }
        }
    }
}

#[test]
fn navigation_raw_text_has_first_refusal_and_successors_need_no_paint() {
    let (mut world, paths) = fixture(false);
    world.model.selection = Some(make_selection(paths[2].clone()));
    // These are consecutive dispatches, each against a freshly placed frame.
    // Never paint: the installed handlers, not the last presented pixels, own
    // the current selection. This is also EditorRunner::update_frame's policy.
    let steps = [
        (NamedKey::ArrowLeft, 2, true),
        (NamedKey::ArrowRight, 2, true),
        (NamedKey::ArrowRight, 3, false),
        (NamedKey::ArrowRight, 3, true),
        (NamedKey::ArrowRight, 3, true),
        (NamedKey::ArrowRight, 3, true),
        (NamedKey::ArrowRight, 3, true),
        (NamedKey::ArrowRight, 3, true),
        (NamedKey::ArrowRight, 3, true),
        (NamedKey::ArrowRight, 4, false),
    ];
    for (key, expected, raw_handles) in steps {
        let projection = sample_projection();
        let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
        let event = KeyboardEvent {
            key: Key::Named(key),
            state: KeyState::Down,
            ..Default::default()
        };
        let (handlers, mut context) = frame.resolve_with_context();
        let handled = handlers.dispatch_key(&mut world, &event);
        assert_eq!(handled, raw_handles);
        if !handled {
            let direction = crate::navigate::direction(&event).unwrap();
            assert!(
                handlers
                    .dispatch(
                        &mut world,
                        puri::handler::Event::Navigate(direction),
                        &mut context
                    )
                    .handled()
            );
        }
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            paths[expected]
        );
    }
}

#[test]
fn navigation_arrival_through_jump_retains_editing_context() {
    let source = new_cell_id();
    let first = new_cell_id();
    let second = new_cell_id();
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([(source, text::value("shared"))])),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(vec![Step::Key(first)]));
    let build = |world: &mut World| {
        let ordinary = sample_projection();
        let projection = Projection::new([
            d::runtime_partial(move |input| {
                input.value?.field(source)?;
                Some(nav::nav_group(d::row(
                    8.0,
                    [
                        d::jump([Step::Key(first)], [Step::Key(source)]),
                        d::jump([Step::Key(second)], [Step::Key(source)]),
                    ],
                )))
            }),
            ordinary.partial,
        ]);
        let frame = editing_frame_with_projection(world, false, Some(&projection));
        frame
    };
    let frame = build(&mut world);
    assert!(frame.navigate(&mut world, Direction::Right));
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        [Step::Key(second)]
    );
    let frame = build(&mut world);
    assert!(frame.resolve_for_dispatch().dispatch_key(
        &mut world,
        &KeyboardEvent {
            key: Key::Character("!".into()),
            state: KeyState::Down,
            ..Default::default()
        }
    ));
    assert_eq!(
        world.model.doc.root,
        Some(Value::record([(source, text::value("!shared"))]))
    );
}

#[test]
fn navigation_runner_installs_successor_handlers_before_the_next_key_without_paint() {
    let (mut world, paths) = fixture(false);
    world.stack.projection = sample_projection();
    let mut runner = crate::EditorRunner::new(world);
    runner.editor.model.selection = Some(make_selection(paths[2].clone()));
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    for expected in [3, 3, 3, 3, 3, 3, 3, 4] {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(NamedKey::ArrowRight),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
        assert_eq!(
            runner.editor.model.selection.as_ref().unwrap().path(),
            paths[expected]
        );
    }
}

#[test]
fn navigation_named_number_hands_off_from_name_at_text_boundary() {
    let mut world = crate::test_editor(Document {
        root: Some(d::overlay_value(
            &f64::value(45.0),
            name::record("tilt", []),
        )),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(vec![Step::Key(name::vocabulary::NAME)]));
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    assert!(runner.keyboard_event(
        &KeyboardEvent {
            key: Key::Named(NamedKey::ArrowRight),
            state: KeyState::Down,
            ..Default::default()
        },
        1.0,
        viewport
    ));
    assert_eq!(runner.editor.model.selection.as_ref().unwrap().path(), []);
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
}

#[test]
fn navigation_example_projects_and_navigates_without_an_app_window() {
    let (doc, _) = crate::gid_text::parse(NAVIGATION_EXAMPLE).unwrap();
    let mut runner = crate::EditorRunner::new(crate::test_editor(doc));
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    for _ in 0..5 {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(NamedKey::ArrowDown),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
    }
}

fn navigation_example_cells() -> (World, Vec<Path>) {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(NAVIGATION_EXAMPLE).unwrap();
    let fields = doc.root.as_ref().unwrap().as_record().unwrap();
    let (section_position, section) = fields
        .get(&OUTLINE)
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .find(|(_, value)| value.as_cell() == Some(binders["parameters"]))
        .unwrap();
    let section = section.as_cell().unwrap();
    let prefix = vec![
        Step::Key(OUTLINE),
        Step::Element(section_position.clone()),
        Step::Key(section),
    ];
    let cells: Vec<Path> = fields
        .get(&section)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .take(2)
        .map(|position| {
            prefix
                .iter()
                .cloned()
                .chain([Step::Element(position.clone())])
                .collect()
        })
        .collect();
    (crate::test_editor(doc), cells)
}

#[test]
fn navigation_workshop_nested_outlines_preserve_occurrences_and_shared_edits() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(NAVIGATION_EXAMPLE).unwrap();
    let root = doc.root.as_ref().unwrap();
    let field = |value: &Value, key| value.as_record().unwrap().get(&key).unwrap().clone();
    let section = |value: &Value, key, occurrence| -> Path {
        let outline = field(value, OUTLINE);
        let (position, _) = outline
            .as_list()
            .unwrap()
            .iter()
            .filter(|(_, value)| value.as_cell() == Some(key))
            .nth(occurrence)
            .unwrap();
        vec![
            Step::Key(OUTLINE),
            Step::Element(position.clone()),
            Step::Key(key),
        ]
    };
    let workshop = field(root, binders["workshop"]);
    let inspection = field(&workshop, binders["inspection"]);
    let checks = field(&inspection, binders["checks"]);
    let check_positions = positions(&checks);
    let prefix: Path = section(root, binders["workshop"], 0)
        .into_iter()
        .chain(section(&workshop, binders["inspection"], 0))
        .collect();
    let document = doc.clone();
    // The same checks are projected twice, inside two nested outline jumps.
    for occurrence in 0..2 {
        let body: Path = prefix
            .iter()
            .cloned()
            .chain(section(&inspection, binders["checks"], occurrence))
            .collect();
        let heading = body[..body.len() - 1].to_vec();
        let first: Path = body
            .iter()
            .cloned()
            .chain([Step::Element(check_positions[0].clone())])
            .collect();
        let cell: Path = body
            .iter()
            .cloned()
            .chain([Step::Element(check_positions[1].clone())])
            .collect();
        let name: Path = cell
            .iter()
            .cloned()
            .chain([
                Step::Follow(gid::Resolution::Document),
                Step::Key(name::vocabulary::NAME),
            ])
            .collect();
        let mut world = crate::test_editor(document.clone());
        let before = world.model.doc.clone();
        world.model.selection = Some(make_selection(heading.clone()));
        for (direction, path) in [
            (Direction::Down, &body),
            (Direction::Down, &first),
            (Direction::Down, &cell),
            (Direction::Right, &name),
        ] {
            let frame = editing_frame(&mut world, false);
            assert!(frame.navigate(&mut world, direction));
            assert_eq!(world.model.selection.as_ref().unwrap().path(), *path);
        }
        let source = vec![
            Step::Key(binders["workshop"]),
            Step::Key(binders["inspection"]),
            Step::Key(binders["checks"]),
            Step::Element(check_positions[1].clone()),
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ];
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .scope()
                .source(&name)
                .unwrap()
                .as_ref(),
            source
        );
        assert!(Rc::ptr_eq(&world.model.doc, &before));
        let frame = editing_frame(&mut world, false);
        assert!(frame.resolve_for_dispatch().dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Character("X".into()),
                state: KeyState::Down,
                ..Default::default()
            }
        ));
        assert_eq!(
            field(
                world.model.doc.cells.value(binders["width"]).unwrap(),
                name::vocabulary::NAME
            ),
            text::value("Xwidth")
        );
        assert_eq!(world.model.doc.root, document.root);
    }
}

#[test]
fn navigation_outline_connects_heading_and_body_before_adjacent_sections() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(NAVIGATION_EXAMPLE).unwrap();
    let fields = doc.root.as_ref().unwrap().as_record().unwrap();
    let sections: Vec<_> = fields
        .get(&OUTLINE)
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .map(|(position, value)| {
            let heading = vec![Step::Key(OUTLINE), Step::Element(position.clone())];
            let body: Path = heading
                .iter()
                .cloned()
                .chain([Step::Key(value.as_cell().unwrap())])
                .collect();
            (heading, body)
        })
        .collect();
    let lists = fields.get(&binders["lists"]).unwrap().as_list().unwrap();
    let list_sections: Vec<_> = sections
        .iter()
        .enumerate()
        .filter(|(_, (_, body))| body.last() == Some(&Step::Key(binders["lists"])))
        .map(|(index, _)| index)
        .collect();
    let first_lists = list_sections[0];
    let (last_list_position, last_list) = lists.iter().next_back().unwrap();
    let last_item_position = last_list.as_list().unwrap().keys().next_back().unwrap();
    let last_text: Path = sections[first_lists]
        .1
        .iter()
        .cloned()
        .chain([
            Step::Element(last_list_position.clone()),
            Step::Element(last_item_position.clone()),
        ])
        .collect();
    for width in [900.0, 240.0] {
        let mut runner = crate::EditorRunner::new(crate::test_editor(doc.clone()));
        let viewport = kurbo::Size::new(width, 600.0);
        let document = runner.editor.model.doc.clone();
        let mut walk = |start: &Path, moves: &[(NamedKey, &Path)]| {
            runner.editor.model.selection = Some(make_selection(start.clone()));
            runner.refresh_frame(1.0, viewport);
            for (key, expected) in moves {
                assert!(runner.keyboard_event(
                    &KeyboardEvent {
                        key: Key::Named(key.clone()),
                        state: KeyState::Down,
                        ..Default::default()
                    },
                    1.0,
                    viewport
                ));
                assert_eq!(
                    runner.editor.model.selection.as_ref().unwrap().path(),
                    *expected
                );
                assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
            }
        };
        // Include the repeated "Nested lists" section: occurrences remain distinct.
        for &index in &list_sections {
            walk(
                &sections[index].0,
                &[
                    (NamedKey::ArrowDown, &sections[index].1),
                    (NamedKey::ArrowUp, &sections[index].0),
                ],
            );
        }
        let last_list: Path = sections[first_lists]
            .1
            .iter()
            .cloned()
            .chain([Step::Element(last_list_position.clone())])
            .collect();
        walk(
            &last_text,
            &[
                (NamedKey::ArrowDown, &sections[first_lists + 1].0),
                (
                    NamedKey::ArrowUp,
                    if width == 900.0 {
                        &last_list
                    } else {
                        &last_text
                    },
                ),
            ],
        );
    }
}

#[test]
fn navigation_example_cells_leave_vertical_navigation_to_the_list() {
    let (mut world, cells) = navigation_example_cells();
    let name: Path = cells[0]
        .iter()
        .cloned()
        .chain([
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ])
        .collect();
    world.model.selection = Some(make_selection(cells[0].clone()));
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    for (key, expected) in [
        (NamedKey::ArrowDown, &cells[1]),
        (NamedKey::ArrowUp, &cells[0]),
        (NamedKey::ArrowRight, &name),
        (NamedKey::ArrowDown, &cells[1]),
    ] {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(key),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
        assert_eq!(
            runner.editor.model.selection.as_ref().unwrap().path(),
            expected
        );
    }
}

#[test]
fn navigation_horizontal_row_wrapping_preserves_text_arrival_direction() {
    let root = Value::list([text::value("abc"), text::value("de")]);
    let paths: Vec<Path> = positions(&root)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells: Cells::new(),
    });
    world.stack.projection = Projection::new([
        d::structure::list_column(8.0, None),
        world.stack.projection.partial.clone(),
    ]);
    world.model.selection = Some(make_selection(paths[0].clone()));
    let document = world.model.doc.clone();
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    // The ordinary initial caret is at the end. Wrapping must preserve the
    // horizontal arrival, rather than turning Right into a Down event.
    for (key, row, offset) in [
        (NamedKey::ArrowRight, 1, Some(0)),
        (NamedKey::ArrowLeft, 0, Some(3)),
        // Down uses the widget's ordinary default, with no explicit caret.
        (NamedKey::ArrowDown, 1, None),
    ] {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(key),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport,
        ));
        let selection = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(selection.path(), paths[row]);
        assert_eq!(
            selection.edit().map(|line| line.selection_offsets()),
            offset.map(|n| (n, n))
        );
        assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
    }
}

#[test]
fn navigation_example_cells_use_horizontal_contents_and_vertical_siblings() {
    let (mut world, cells) = navigation_example_cells();
    let number = |cell: &Path| -> Path {
        cell.iter()
            .cloned()
            .chain([Step::Follow(gid::Resolution::Document)])
            .collect()
    };
    let name = |cell: &Path| -> Path {
        number(cell)
            .into_iter()
            .chain([Step::Key(name::vocabulary::NAME)])
            .collect()
    };
    world.model.selection = Some(make_selection(cells[0].clone()));
    let document = world.model.doc.clone();
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    let mut step = |key, path: Path, offset: Option<usize>| {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(key),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
        let selection = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(selection.path(), path);
        assert_eq!(
            selection.edit().map(|line| line.selection_offsets()),
            offset.map(|n| (n, n))
        );
        assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
    };
    step(NamedKey::ArrowRight, name(&cells[0]), Some(0));
    for offset in 1..=5 {
        step(NamedKey::ArrowRight, name(&cells[0]), Some(offset));
    }
    step(NamedKey::ArrowRight, number(&cells[0]), Some(0));
    step(NamedKey::ArrowRight, number(&cells[0]), Some(1));
    step(NamedKey::ArrowRight, number(&cells[0]), Some(2));
    step(NamedKey::ArrowRight, cells[1].clone(), None);
    step(NamedKey::ArrowRight, name(&cells[1]), Some(0));
    step(NamedKey::ArrowLeft, cells[1].clone(), None);
    step(NamedKey::ArrowLeft, number(&cells[0]), Some(2));
    step(NamedKey::ArrowLeft, number(&cells[0]), Some(1));
    step(NamedKey::ArrowLeft, number(&cells[0]), Some(0));
    step(NamedKey::ArrowLeft, name(&cells[0]), Some(5));
    for offset in (0..5).rev() {
        step(NamedKey::ArrowLeft, name(&cells[0]), Some(offset));
    }
    step(NamedKey::ArrowLeft, cells[0].clone(), None);
    step(NamedKey::ArrowDown, cells[1].clone(), None);
    step(NamedKey::ArrowUp, cells[0].clone(), None);
}

#[test]
fn navigation_empty_cell_stops_before_entering_missing_contents() {
    let outer = Value::list([
        text::value("before"),
        Value::Cell(gid::new_cell_id()),
        text::value("after"),
    ]);
    let paths: Vec<Path> = positions(&outer)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    for (start, direction) in [(0, NamedKey::ArrowRight), (2, NamedKey::ArrowLeft)] {
        let mut world = crate::test_editor(Document {
            root: Some(outer.clone()),
            cells: Cells::new(),
        });
        world.model.selection = Some(make_selection(paths[start].clone()));
        let document = world.model.doc.clone();
        let mut runner = crate::EditorRunner::new(world);
        let viewport = kurbo::Size::new(900.0, 600.0);
        runner.refresh_frame(1.0, viewport);
        // Left must first move through the text to its boundary.
        if start == 2 {
            for _ in 0..5 {
                assert!(runner.keyboard_event(
                    &KeyboardEvent {
                        key: Key::Named(NamedKey::ArrowLeft),
                        state: KeyState::Down,
                        ..Default::default()
                    },
                    1.0,
                    viewport
                ));
            }
        }
        let contents: Path = paths[1]
            .iter()
            .cloned()
            .chain([Step::Follow(gid::Resolution::Document)])
            .collect();
        let stops = if start == 0 {
            [&paths[1], &contents]
        } else {
            [&contents, &paths[1]]
        };
        for expected in stops {
            assert!(runner.keyboard_event(
                &KeyboardEvent {
                    key: Key::Named(direction.clone()),
                    state: KeyState::Down,
                    ..Default::default()
                },
                1.0,
                viewport
            ));
            assert_eq!(
                runner.editor.model.selection.as_ref().unwrap().path(),
                expected
            );
            assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
        }
    }
}

#[test]
fn navigation_whole_wrapper_reuses_a_leaf_at_the_same_occurrence() {
    let root = Value::list([text::value("first"), text::value("second")]);
    let paths: Vec<_> = positions(&root)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells: Cells::new(),
    });
    let ordinary = sample_projection();
    let projection = Projection::new([
        d::runtime_partial(|input| {
            crate::libraries::text::display(input).map(|child| nav::nav_group(child))
        }),
        ordinary.partial,
    ]);
    world.model.selection = Some(make_selection(paths[1].clone()));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(frame.navigate(&mut world, Direction::Left));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), paths[0]);
}

#[test]
fn navigation_example_lists_leave_vertical_navigation_to_the_outer_list() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(NAVIGATION_EXAMPLE).unwrap();
    let section = binders["lists"];
    let fields = doc.root.as_ref().unwrap().as_record().unwrap();
    let (section_position, _) = fields
        .get(&OUTLINE)
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .find(|(_, value)| value.as_cell() == Some(section))
        .unwrap();
    let prefix = vec![
        Step::Key(OUTLINE),
        Step::Element(section_position.clone()),
        Step::Key(section),
    ];
    let lists: Vec<Path> = fields
        .get(&section)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .map(|position| {
            prefix
                .iter()
                .cloned()
                .chain([Step::Element(position.clone())])
                .collect()
        })
        .collect();
    let first_item = fields
        .get(&section)
        .unwrap()
        .as_list()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let first_text: Path = lists[0]
        .iter()
        .cloned()
        .chain([Step::Element(first_item)])
        .collect();
    for width in [900.0, 240.0] {
        let mut world = crate::test_editor(doc.clone());
        world.model.selection = Some(make_selection(lists[0].clone()));
        let mut runner = crate::EditorRunner::new(world);
        let viewport = kurbo::Size::new(width, 600.0);
        runner.refresh_frame(1.0, viewport);
        for (key, expected) in [
            (NamedKey::ArrowDown, &lists[1]),
            (NamedKey::ArrowDown, &lists[2]),
            (NamedKey::ArrowUp, &lists[1]),
            (NamedKey::ArrowUp, &lists[0]),
            (NamedKey::ArrowRight, &first_text),
        ] {
            assert!(runner.keyboard_event(
                &KeyboardEvent {
                    key: Key::Named(key),
                    state: KeyState::Down,
                    ..Default::default()
                },
                1.0,
                viewport
            ));
            assert_eq!(
                runner.editor.model.selection.as_ref().unwrap().path(),
                expected,
                "width={width}, key={key:?}"
            );
        }
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
    }
}

#[test]
fn navigation_nested_list_has_one_leading_stop_in_both_directions() {
    let inner = Value::list([text::value("F"), text::value("G")]);
    let inner_positions = positions(&inner);
    let outer = Value::list([text::value("E"), inner, text::value("H")]);
    let outer_positions = positions(&outer);
    let e = vec![Step::Element(outer_positions[0].clone())];
    let list = vec![Step::Element(outer_positions[1].clone())];
    let f: Path = list
        .iter()
        .cloned()
        .chain([Step::Element(inner_positions[0].clone())])
        .collect();
    let g: Path = list
        .iter()
        .cloned()
        .chain([Step::Element(inner_positions[1].clone())])
        .collect();
    let h = vec![Step::Element(outer_positions[2].clone())];
    let mut world = crate::test_editor(Document {
        root: Some(outer),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(e.clone()));
    let document = world.model.doc.clone();
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    for (key, path, offset) in [
        (NamedKey::ArrowRight, &list, None),
        (NamedKey::ArrowRight, &f, Some(0)),
        (NamedKey::ArrowRight, &f, Some(1)),
        (NamedKey::ArrowRight, &g, Some(0)),
        (NamedKey::ArrowRight, &g, Some(1)),
        (NamedKey::ArrowRight, &h, Some(0)),
        (NamedKey::ArrowLeft, &g, Some(1)),
        (NamedKey::ArrowLeft, &g, Some(0)),
        (NamedKey::ArrowLeft, &f, Some(1)),
        (NamedKey::ArrowLeft, &f, Some(0)),
        (NamedKey::ArrowLeft, &list, None),
        (NamedKey::ArrowLeft, &e, Some(1)),
    ] {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(key),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
        let selection = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(selection.path(), path);
        assert_eq!(
            selection.edit().map(|line| line.selection_offsets()),
            offset.map(|n| (n, n))
        );
        assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
    }
}

#[test]
fn navigation_empty_list_stops_once_in_either_direction() {
    let outer = Value::list([text::value("before"), Value::list([]), text::value("after")]);
    let paths: Vec<_> = positions(&outer)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    let mut world = crate::test_editor(Document {
        root: Some(outer),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(paths[0].clone()));
    let mut runner = crate::EditorRunner::new(world);
    let viewport = kurbo::Size::new(900.0, 600.0);
    runner.refresh_frame(1.0, viewport);
    for (key, expected) in [
        (NamedKey::ArrowRight, 1),
        (NamedKey::ArrowRight, 2),
        (NamedKey::ArrowLeft, 1),
        (NamedKey::ArrowLeft, 0),
    ] {
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(key),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
        assert_eq!(
            runner.editor.model.selection.as_ref().unwrap().path(),
            paths[expected]
        );
    }
}

#[test]
fn content_after_a_multiline_block_stays_on_its_content_line() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(NAVIGATION_EXAMPLE).unwrap();
    let fields = doc.root.as_ref().unwrap().as_record().unwrap();
    let section = |name: &str| -> Path {
        let (position, _) = fields
            .get(&OUTLINE)
            .unwrap()
            .as_list()
            .unwrap()
            .iter()
            .find(|(_, value)| value.as_cell() == Some(binders[name]))
            .unwrap();
        vec![
            Step::Key(OUTLINE),
            Step::Element(position.clone()),
            Step::Key(binders[name]),
        ]
    };
    let child = |path: &Path, steps: &[Step]| -> Path { [path.as_slice(), steps].concat() };
    let items = |case: &str| -> Vec<Path> {
        let block = child(&section(case), &[Step::Key(binders["block"])]);
        fields
            .get(&binders[case])
            .unwrap()
            .as_record()
            .unwrap()
            .get(&binders["block"])
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .map(|position| child(&block, &[Step::Element(position.clone())]))
            .collect()
    };
    let viewport = kurbo::Size::new(900.0, 2400.0);
    let mut runner = crate::EditorRunner::new(crate::test_editor(doc.clone()));
    let mut check = |from: &Path, key: NamedKey, to: &Path| {
        runner.editor.model.selection = Some(make_selection(from.clone()));
        runner.refresh_frame(1.0, viewport);
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(key.clone()),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            viewport
        ));
        assert_eq!(
            runner.editor.model.selection.as_ref().unwrap().path(),
            to.as_slice(),
            "{key:?} from {from:?}"
        );
    };

    let body = section("after_label");
    let list = child(&body, &[Step::Key(binders["block"])]);
    let beside = child(&body, &[Step::Key(binders["beside"])]);
    let [first, second, _] = items("after_label").try_into().unwrap();
    check(&list, NamedKey::ArrowRight, &first);
    check(&list, NamedKey::ArrowDown, &first);
    check(&first, NamedKey::ArrowRight, &beside);
    check(&beside, NamedKey::ArrowRight, &second);
    check(&beside, NamedKey::ArrowDown, &second);

    let body = section("after_value");
    let list = child(&body, &[Step::Key(binders["block"])]);
    let beside = child(&body, &[Step::Key(binders["beside"])]);
    let beside_item = {
        let value = fields
            .get(&binders["after_value"])
            .unwrap()
            .as_record()
            .unwrap()
            .get(&binders["beside"])
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        child(&beside, &[Step::Element(value)])
    };
    let [first, second, _] = items("after_value").try_into().unwrap();
    check(&list, NamedKey::ArrowRight, &first);
    check(&first, NamedKey::ArrowRight, &beside);
    check(&beside, NamedKey::ArrowRight, &beside_item);
    check(&beside_item, NamedKey::ArrowRight, &second);
    check(&beside_item, NamedKey::ArrowDown, &second);
}
