//! Layout navigation through real projections and the ordinary event chain.

use crate::display as d;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::driver::Driver;
use crate::frame::test_support::editing_frame;
use crate::libraries::{f64, name, text};
use crate::location::{Location, Start};
use crate::projection::Projection;
use crate::test_support::{make_selection, positions};
use gid::{Cells, Document, Path, Step, Value, new_cell_id};
use puri::handler::test_support::DispatchShorthands;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, NamedKey};

type World = crate::Editor;
use puri::handler::NavigationDirection;

fn sample_projection() -> Projection {
    Projection::new([d::runtime_partial(|input| {
        if let Some(items) = d::structure::list_items(input, None) {
            let items: Vec<_> = items.into_iter().map(|(_, layout)| layout).collect();
            Some(d::alternatives([
                d::row(8.0, items.clone()),
                d::col(0, 8.0, items),
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
    (crate::editor_of(root), paths)
}

#[test]
fn navigation_destinations_follow_nested_container_boundaries_and_chosen_layout() {
    use NavigationDirection::*;
    for wide in [false, true] {
        let (mut world, paths) = fixture(wide);
        world.stack.projection = sample_projection();
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
                let frame = editing_frame(&mut world);
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
    use NavigationDirection::*;
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
                let ordinary = world.stack.projection.partial().clone();
                world.stack.projection = Projection::new([
                    d::runtime_partial(move |input| {
                        input.value?.field(wrapper)?;
                        Some(d::descend(
                            [Step::Key(wrapper)],
                            d::Projected::overriding(d::instead(d::structure::list_column(
                                8.0, None,
                            ))),
                        ))
                    }),
                    ordinary,
                ]);
                let frame = editing_frame(&mut world);
                let rect = |path: &Path| frame.landmarks.landmark(path).unwrap().rect;
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
                    let frame = editing_frame(&mut world);
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
    world.stack.projection = sample_projection();
    for (key, expected, raw_handles) in steps {
        let frame = editing_frame(&mut world);
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
    let jumped = |key| {
        vec![
            crate::occurrence::Step::Key(key),
            crate::occurrence::Step::jump(Location::root(vec![Step::Key(source)])),
        ]
    };
    let mut world = crate::editor_of(Value::record([(source, text::value("shared"))]));
    world.model.selection = Some(crate::selection::Selection::edge(jumped(first)));
    world.stack.projection = Projection::new([
        d::runtime_partial(move |input| {
            input.value?.field(source)?;
            Some(d::row(
                8.0,
                [
                    d::jump(
                        [Step::Key(first)],
                        crate::location::Location::root(vec![Step::Key(source)]),
                        d::Projected::ordinary(),
                    ),
                    d::jump(
                        [Step::Key(second)],
                        crate::location::Location::root(vec![Step::Key(source)]),
                        d::Projected::ordinary(),
                    ),
                ],
            ))
        }),
        sample_projection().partial().clone(),
    ]);
    let frame = editing_frame(&mut world);
    assert!(frame.navigate(&mut world, NavigationDirection::Right));
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        jumped(second)
    );
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.type_text("!");
    assert_eq!(
        driver.value(&[]),
        Value::record([(source, text::value("!shared"))])
    );
}

#[test]
fn navigation_runner_installs_successor_handlers_before_the_next_key_without_paint() {
    let (mut world, paths) = fixture(false);
    world.stack.projection = sample_projection();
    let mut runner = crate::EditorRunner::new(world);
    runner.editor.model.selection = Some(make_selection(paths[2].clone()));
    let viewport = Driver::VIEWPORT;
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
    let mut driver = Driver::of(f64::value(45.0).overlaid(name::record("tilt", [])));
    driver.select(&[Step::Key(name::vocabulary::NAME)]);
    driver.press(NamedKey::ArrowRight);
    assert!(driver.selection().is_empty());
    assert_eq!(driver.line_selection(), Some((0, 0)));
}

#[test]
fn an_arrow_from_a_selection_on_nothing_drawn_selects_the_root() {
    // A number draws as its spelling, not its encoding's field, so a
    // selection there is on nothing drawn.
    let number = crate::test_values::key("number");
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([(
            crate::test_values::label("number"),
            f64::value(42.0),
        )])),
        cells: Cells::new(),
    });
    world.model.selection = Some(make_selection(vec![
        number,
        Step::Key(f64::vocabulary::F64),
    ]));
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    assert!(!driver.drawn(&driver.selection()));
    driver.press(NamedKey::ArrowDown);
    assert_eq!(driver.selection(), crate::occurrence::of(&[]));
}

#[test]
fn navigation_example_projects_and_navigates_without_an_app_window() {
    let (doc, _) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let selections: Vec<crate::occurrence::Path> = (0..5)
        .map(|_| {
            driver.press(NamedKey::ArrowDown);
            driver.selection()
        })
        .collect();
    // Each press moves on to a stop not selected before.
    assert!(
        selections
            .iter()
            .enumerate()
            .all(|(index, path)| !selections[..index].contains(path)),
        "{selections:?}"
    );
}

/// Where an outline draws the body of its section `key`, below the section's
/// `heading`: jumped to the field `key` of the outline stored at `outline`.
fn section_body(
    heading: &[crate::occurrence::Step],
    outline: &[Step],
    key: gid::CellId,
) -> crate::occurrence::Path {
    heading
        .iter()
        .cloned()
        .chain([
            crate::occurrence::Step::Key(key),
            crate::occurrence::Step::jump(Location::root(
                outline
                    .iter()
                    .cloned()
                    .chain([Step::Key(key)])
                    .collect::<Path>(),
            )),
        ])
        .collect()
}

fn navigation_example_cells() -> (World, Vec<crate::occurrence::Path>) {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
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
    let prefix = section_body(
        &crate::occurrence::of(&[Step::Key(OUTLINE), Step::Element(section_position.clone())]),
        &[],
        section,
    );
    let cells = fields
        .get(&section)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .take(2)
        .map(|position| crate::occurrence::below(&prefix, &[Step::Element(position.clone())]))
        .collect();
    (crate::test_editor(doc), cells)
}

#[test]
fn navigation_workshop_nested_outlines_preserve_occurrences_and_shared_edits() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
    let root = doc.root.as_ref().unwrap();
    let field = |value: &Value, key| value.as_record().unwrap().get(&key).unwrap().clone();
    // The heading of the `occurrence`th section `key` of the outline `value`
    // drawn at `parent`.
    let section_heading = |parent: &[crate::occurrence::Step], value: &Value, key, occurrence| {
        let outline = field(value, OUTLINE);
        let (position, _) = outline
            .as_list()
            .unwrap()
            .iter()
            .filter(|(_, value)| value.as_cell() == Some(key))
            .nth(occurrence)
            .unwrap();
        crate::occurrence::below(
            parent,
            &[Step::Key(OUTLINE), Step::Element(position.clone())],
        )
    };
    let workshop = field(root, binders["workshop"]);
    let inspection = field(&workshop, binders["inspection"]);
    let checks = field(&inspection, binders["checks"]);
    let check_positions = positions(&checks);
    let prefix = section_body(
        &section_heading(
            &section_body(
                &section_heading(&[], root, binders["workshop"], 0),
                &[],
                binders["workshop"],
            ),
            &workshop,
            binders["inspection"],
            0,
        ),
        &[Step::Key(binders["workshop"])],
        binders["inspection"],
    );
    let document = doc.clone();
    // The same checks are projected twice, inside two nested outline jumps.
    for occurrence in 0..2 {
        let heading = section_heading(&prefix, &inspection, binders["checks"], occurrence);
        let body = section_body(
            &heading,
            &[
                Step::Key(binders["workshop"]),
                Step::Key(binders["inspection"]),
            ],
            binders["checks"],
        );
        let first = crate::occurrence::below(&body, &[Step::Element(check_positions[0].clone())]);
        let cell = crate::occurrence::below(&body, &[Step::Element(check_positions[1].clone())]);
        let name = crate::occurrence::below(
            &cell,
            &[
                Step::Follow(gid::Resolution::Document),
                Step::Key(name::vocabulary::NAME),
            ],
        );
        let mut world = crate::test_editor(document.clone());
        let before = world.model.doc.clone();
        world.model.selection = Some(crate::selection::Selection::edge(heading));
        for (direction, path) in [
            (NavigationDirection::Down, &body),
            (NavigationDirection::Down, &first),
            (NavigationDirection::Down, &cell),
            (NavigationDirection::Right, &name),
        ] {
            let frame = editing_frame(&mut world);
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
        // The cell's definition stores what's selected, however the outline
        // jumps reached it.
        let sources = world.sources();
        let selected = world.model.selection.as_ref().unwrap().location().unwrap();
        let stored = selected.stored(&sources);
        assert_eq!(stored, Location::root(source.as_slice()).stored(&sources));
        assert!(matches!(
            stored.map(|location| location.start),
            Some(Start::Cell { .. })
        ));
        assert!(Rc::ptr_eq(&world.model.doc, &before));
        let mut driver = Driver::new(world, Driver::VIEWPORT);
        driver.type_text("X");
        let doc = &driver.editor().model.doc;
        assert_eq!(
            field(
                doc.cells.value(binders["width"]).unwrap(),
                name::vocabulary::NAME
            ),
            text::value("Xwidth")
        );
        assert_eq!(doc.root, document.root);
    }
}

#[test]
fn navigation_outline_connects_heading_and_body_before_adjacent_sections() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
    let fields = doc.root.as_ref().unwrap().as_record().unwrap();
    let outline = fields.get(&OUTLINE).unwrap().as_list().unwrap();
    let sections: Vec<_> = outline
        .iter()
        .map(|(position, value)| {
            let heading =
                crate::occurrence::of(&[Step::Key(OUTLINE), Step::Element(position.clone())]);
            let body = section_body(&heading, &[], value.as_cell().unwrap());
            (heading, body)
        })
        .collect();
    let lists = fields.get(&binders["lists"]).unwrap().as_list().unwrap();
    let list_sections: Vec<_> = outline
        .values()
        .enumerate()
        .filter(|(_, value)| value.as_cell() == Some(binders["lists"]))
        .map(|(index, _)| index)
        .collect();
    let first_lists = list_sections[0];
    let (last_list_position, last_list) = lists.iter().next_back().unwrap();
    let last_item_position = last_list.as_list().unwrap().keys().next_back().unwrap();
    let last_text = crate::occurrence::below(
        &sections[first_lists].1,
        &[
            Step::Element(last_list_position.clone()),
            Step::Element(last_item_position.clone()),
        ],
    );
    for width in [900.0, 240.0] {
        let mut driver = Driver::new(
            crate::test_editor(doc.clone()),
            kurbo::Size::new(width, 600.0),
        );
        let document = driver.editor().model.doc.clone();
        let mut walk = |start: &crate::occurrence::Path,
                        moves: &[(NamedKey, &crate::occurrence::Path)]| {
            driver.select(start);
            for (key, expected) in moves {
                driver.press(*key);
                assert_eq!(driver.selection(), **expected);
                assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
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
        let last_list = crate::occurrence::below(
            &sections[first_lists].1,
            &[Step::Element(last_list_position.clone())],
        );
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
    let (world, cells) = navigation_example_cells();
    let name = crate::occurrence::below(
        &cells[0],
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ],
    );
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&cells[0]);
    for (key, expected) in [
        (NamedKey::ArrowDown, &cells[1]),
        (NamedKey::ArrowUp, &cells[0]),
        (NamedKey::ArrowRight, &name),
        (NamedKey::ArrowDown, &cells[1]),
    ] {
        driver.press(key);
        assert_eq!(driver.selection(), *expected);
    }
}

#[test]
fn navigation_horizontal_row_wrapping_preserves_text_arrival_direction() {
    let root = Value::list([text::value("abc"), text::value("de")]);
    let paths: Vec<Path> = positions(&root)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    let mut world = crate::editor_of(root);
    world.stack.projection = Projection::new([
        d::structure::list_column(8.0, None),
        world.stack.projection.partial().clone(),
    ]);
    let document = world.model.doc.clone();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&paths[0]);
    // The ordinary initial caret is at the end. Wrapping must preserve the
    // horizontal arrival, rather than turning Right into a Down event.
    for (key, row, offset) in [
        (NamedKey::ArrowRight, 1, Some(0)),
        (NamedKey::ArrowLeft, 0, Some(3)),
        // Down uses the widget's ordinary default, with no explicit caret.
        (NamedKey::ArrowDown, 1, None),
    ] {
        driver.press(key);
        assert_eq!(driver.selection(), paths[row]);
        assert_eq!(driver.line_selection(), offset.map(|n| (n, n)));
        assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
    }
}

#[test]
fn navigation_example_cells_use_horizontal_contents_and_vertical_siblings() {
    let (world, cells) = navigation_example_cells();
    let number = |cell: &crate::occurrence::Path| {
        crate::occurrence::below(cell, &[Step::Follow(gid::Resolution::Document)])
    };
    let name = |cell: &crate::occurrence::Path| {
        crate::occurrence::below(&number(cell), &[Step::Key(name::vocabulary::NAME)])
    };
    let document = world.model.doc.clone();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&cells[0]);
    let mut step = |key, path: crate::occurrence::Path, offset: Option<usize>| {
        driver.press(key);
        assert_eq!(driver.selection(), path);
        assert_eq!(driver.line_selection(), offset.map(|n| (n, n)));
        assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
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
        let world = crate::editor_of(outer.clone());
        let document = world.model.doc.clone();
        let mut driver = Driver::new(world, Driver::VIEWPORT);
        driver.select(&paths[start]);
        // Left must first move through the text to its boundary.
        if start == 2 {
            for _ in 0..5 {
                driver.press(NamedKey::ArrowLeft);
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
            driver.press(direction);
            assert_eq!(driver.selection(), *expected);
            assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
        }
    }
}

#[test]
fn a_leaf_s_own_stop_is_its_occurrence_s_only_stop() {
    let root = Value::list([text::value("first"), text::value("second")]);
    let paths: Vec<_> = positions(&root)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    let mut world = crate::editor_of(root);
    let ordinary = sample_projection();
    world.stack.projection = Projection::new([
        d::runtime_partial(|input| crate::libraries::text::display(input)),
        ordinary.partial().clone(),
    ]);
    world.model.selection = Some(make_selection(paths[1].clone()));
    let frame = editing_frame(&mut world);
    assert!(frame.navigate(&mut world, NavigationDirection::Left));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), paths[0]);
}

#[test]
fn navigation_example_lists_leave_vertical_navigation_to_the_outer_list() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
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
    let prefix = section_body(
        &crate::occurrence::of(&[Step::Key(OUTLINE), Step::Element(section_position.clone())]),
        &[],
        section,
    );
    let lists: Vec<_> = fields
        .get(&section)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .map(|position| crate::occurrence::below(&prefix, &[Step::Element(position.clone())]))
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
    let first_text = crate::occurrence::below(&lists[0], &[Step::Element(first_item)]);
    for width in [900.0, 240.0] {
        let mut driver = Driver::new(
            crate::test_editor(doc.clone()),
            kurbo::Size::new(width, 600.0),
        );
        driver.select(&lists[0]);
        for (key, expected) in [
            (NamedKey::ArrowDown, &lists[1]),
            (NamedKey::ArrowDown, &lists[2]),
            (NamedKey::ArrowUp, &lists[1]),
            (NamedKey::ArrowUp, &lists[0]),
            (NamedKey::ArrowRight, &first_text),
        ] {
            driver.press(key);
            assert_eq!(driver.selection(), *expected, "width={width}, key={key:?}");
        }
        assert_eq!(driver.line_selection(), Some((0, 0)));
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
    let world = crate::editor_of(outer);
    let document = world.model.doc.clone();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&e);
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
        driver.press(key);
        assert_eq!(driver.selection(), *path);
        assert_eq!(driver.line_selection(), offset.map(|n| (n, n)));
        assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
    }
}

#[test]
fn navigation_empty_list_stops_once_in_either_direction() {
    let outer = Value::list([text::value("before"), Value::list([]), text::value("after")]);
    let paths: Vec<_> = positions(&outer)
        .into_iter()
        .map(|p| vec![Step::Element(p)])
        .collect();
    let mut driver = Driver::of(outer);
    driver.select(&paths[0]);
    for (key, expected) in [
        (NamedKey::ArrowRight, 1),
        (NamedKey::ArrowRight, 2),
        (NamedKey::ArrowLeft, 1),
        (NamedKey::ArrowLeft, 0),
    ] {
        driver.press(key);
        assert_eq!(driver.selection(), paths[expected]);
    }
}

#[test]
fn content_after_a_multiline_block_stays_on_its_content_line() {
    use crate::libraries::presentation::vocabulary::OUTLINE;
    let (doc, binders) = crate::gid_text::parse(crate::test_examples::NAVIGATION).unwrap();
    let fields = doc.root.as_ref().unwrap().as_record().unwrap();
    let section = |name: &str| {
        let (position, _) = fields
            .get(&OUTLINE)
            .unwrap()
            .as_list()
            .unwrap()
            .iter()
            .find(|(_, value)| value.as_cell() == Some(binders[name]))
            .unwrap();
        section_body(
            &crate::occurrence::of(&[Step::Key(OUTLINE), Step::Element(position.clone())]),
            &[],
            binders[name],
        )
    };
    let child = crate::occurrence::below;
    let items = |case: &str| -> Vec<crate::occurrence::Path> {
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
    let mut driver = Driver::new(
        crate::test_editor(doc.clone()),
        kurbo::Size::new(900.0, 2400.0),
    );
    let mut check =
        |from: &crate::occurrence::Path, key: NamedKey, to: &crate::occurrence::Path| {
            driver.select(from);
            driver.press(key);
            assert_eq!(driver.selection(), *to, "{key:?} from {from:?}");
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

#[test]
fn a_value_a_grap_function_draws_is_an_arrow_stop() {
    // An angle draws as a dial, its degrees and a word, all from a Grap
    // function that declares nothing for navigation: the degrees are a stop
    // of their own, and the angle is one before them.
    let (doc, _) =
        crate::gid_text::parse(include_str!("../../../../examples/libraries.gid")).unwrap();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let angles = Step::Key(gid::CellId::from_u128(0x1658642f21a8218fbfec1e4825521991));
    let degrees = Step::Key(gid::CellId::from_u128(0xca9a2253bfe9bb037b1daf8416c70a8c));
    let angle = driver.element(&[angles], 0, &[]);
    let number = [&angle[..], &[degrees]].concat();
    driver.select(&number);
    // The degrees' caret moves first, as far as its spelling goes.
    for _ in 0..8 {
        driver.press(NamedKey::ArrowLeft);
        if driver.selection() != crate::occurrence::of(&number) {
            break;
        }
    }
    assert_eq!(driver.selection(), crate::occurrence::of(&angle));
}
