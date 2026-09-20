use super::*;
use crate::libraries::f64 as f64_convention;

#[test]
fn website_values_use_ordinary_text_and_number_editing() {
    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/values.gid"
    ))
    .unwrap();
    let mut world = crate::test_editor_with_stack(
        doc.clone(),
        crate::stack::load_selected(&[
            crate::libraries::name::ID,
            text::ID,
            crate::libraries::blob::ID,
            crate::libraries::number::ID,
            f64_convention::ID,
        ])
        .unwrap(),
    );
    for (field, typed) in [("greeting", " Welcome!"), ("count", "5")] {
        let frame = editing_frame(&mut world, false);
        let target = frame
            .descends
            .iter()
            .find(|d| d.path.as_ref() == [Step::Key(names[field])])
            .unwrap();
        assert!((target.select)(&mut world, None));
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_key(
                    &mut world,
                    &KeyboardEvent {
                        key: Key::Character(typed.into()),
                        state: KeyState::Down,
                        ..Default::default()
                    },
                )
        );
    }
    let fields = world.model.doc.root.as_ref().unwrap().as_record().unwrap();
    assert_eq!(
        text::read(fields.get(&names["greeting"]).unwrap()),
        Some("Hello, world! Welcome!")
    );
    assert_eq!(
        f64_convention::read(fields.get(&names["count"]).unwrap()),
        Some(35.0)
    );
    assert_ne!(world.model.doc.root, doc.root);
}

#[test]
fn website_list_instructions_insert_through_a_comma_and_select_the_whole_list() {
    let (doc, _) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/lists.gid"
    ))
    .unwrap();
    let mut world = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            crate::libraries::name::ID,
            text::ID,
            crate::libraries::blob::ID,
        ])
        .unwrap(),
    );
    let frame = editing_frame(&mut world, false);
    let elements: Vec<_> = frame
        .descends
        .iter()
        .filter(|d| matches!(d.path.as_ref(), [Step::Element(_)]))
        .collect();
    let point = Point::new(
        (elements[0].rect.x1 + elements[1].rect.x0) / 2.0,
        elements[0].rect.center().y,
    );
    let frame = editing_frame_at(&mut world, false, None, Some(point));
    let (_, Claim::Direct(hover)) = frame.claim.as_ref().unwrap() else {
        panic!("comma hover")
    };
    let mut dispatch = placed::DispatchContext::new(Some(crate::test_root()), Some(hover.clone()));
    let mut event = press(point.x, false);
    event.state.position.y = point.y;
    assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
        &mut world,
        &event,
        &mut dispatch
    ));
    for key in [
        Key::Character("\"peaches\"".into()),
        Key::Named(NamedKey::Enter),
    ] {
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_key(
                    &mut world,
                    &KeyboardEvent {
                        key,
                        state: KeyState::Down,
                        ..Default::default()
                    }
                )
        );
    }
    assert_eq!(
        world
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_list()
            .unwrap()
            .values()
            .filter_map(text::read)
            .collect::<Vec<_>>(),
        ["apples", "peaches", "pears", "plums"]
    );
    let frame = editing_frame(&mut world, false);
    let root = frame.descends.iter().find(|d| d.path.is_empty()).unwrap();
    assert!((root.select)(&mut world, None));
    assert!(world.model.selection.as_ref().unwrap().path().is_empty());
}

#[test]
fn website_cells_share_values_and_support_constructing_and_picking_a_new_cell() {
    fn key(world: &mut crate::Editor, key: Key) {
        assert!(
            editing_frame(world, false)
                .resolve_for_dispatch()
                .dispatch_key(
                    world,
                    &KeyboardEvent {
                        key,
                        state: KeyState::Down,
                        ..Default::default()
                    },
                )
        );
    }
    fn click(world: &mut crate::Editor, point: Point, pick: bool) {
        let frame = editing_frame_at(world, false, None, Some(point));
        let (_, Claim::Direct(hover)) = frame.claim.as_ref().unwrap() else {
            panic!("lesson click must have a direct target")
        };
        let mut dispatch =
            placed::DispatchContext::new(Some(crate::test_root()), Some(hover.clone()));
        let mut event = press(point.x, pick);
        event.state.position = (point.x, point.y).into();
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            world,
            &event,
            &mut dispatch
        ));
    }
    fn gap(world: &mut crate::Editor) {
        let frame = editing_frame(world, false);
        let elements: Vec<_> = frame
            .descends
            .iter()
            .filter(|d| matches!(d.path.as_ref(), [Step::Element(_)]))
            .collect();
        click(
            world,
            Point::new(
                (elements[0].rect.x1 + elements[1].rect.x0) / 2.0,
                elements[0].rect.center().y,
            ),
            false,
        );
    }
    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/cells.gid"
    ))
    .unwrap();
    let shared = names["shared"];
    let mut world = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            crate::libraries::name::ID,
            text::ID,
            crate::libraries::blob::ID,
            crate::libraries::number::ID,
            f64_convention::ID,
        ])
        .unwrap(),
    );

    let frame = editing_frame(&mut world, false);
    let number = frame
        .descends
        .iter()
        .find(|d| {
            matches!(
                d.path.as_ref(),
                [Step::Element(_), Step::Follow(gid::Resolution::Document)]
            )
        })
        .unwrap();
    assert!((number.select)(&mut world, None));
    key(&mut world, Key::Character("5".into()));
    assert_eq!(
        world
            .model
            .doc
            .cells
            .value(shared)
            .and_then(f64_convention::read),
        Some(75.0)
    );
    assert!(
        world
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_list()
            .unwrap()
            .values()
            .all(|value| value.as_cell() == Some(shared))
    );

    gap(&mut world);
    key(&mut world, Key::Character("(".into()));
    let cell_path = world.model.selection.as_ref().unwrap().path().to_vec();
    let fresh = world
        .model
        .selection
        .as_ref()
        .unwrap()
        .value(&world.sources())
        .unwrap()
        .as_cell()
        .unwrap();
    assert_ne!(fresh, shared);
    assert!(world.model.doc.cells.value(fresh).is_none());
    let definition_path: Vec<_> = cell_path
        .iter()
        .cloned()
        .chain([Step::Follow(gid::Resolution::Document)])
        .collect();
    let frame = editing_frame(&mut world, false);
    let empty = frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == definition_path)
        .unwrap();
    click(&mut world, empty.rect.center(), false);
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        definition_path
    );
    key(&mut world, Key::Character("11".into()));
    key(&mut world, Key::Named(NamedKey::Enter));
    assert_eq!(
        world
            .model
            .doc
            .cells
            .value(fresh)
            .and_then(f64_convention::read),
        Some(11.0),
        "new cell contents: {:?}; selection: {:?}",
        world.model.doc.cells.value(fresh),
        world.model.selection.as_ref().unwrap().path(),
    );

    gap(&mut world);
    let frame = editing_frame(&mut world, false);
    let cell = frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == cell_path)
        .unwrap();
    let number = frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == definition_path)
        .unwrap();
    click(
        &mut world,
        Point::new((cell.rect.x0 + number.rect.x0) / 2.0, cell.rect.center().y),
        true,
    );
    assert_eq!(
        world
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_list()
            .unwrap()
            .values()
            .filter(|value| value.as_cell() == Some(fresh))
            .count(),
        2,
        "picked root: {:?}; selection: {:?}",
        world.model.doc.root,
        world.model.selection.as_ref().unwrap().path(),
    );
    let linked_path = world.model.selection.as_ref().unwrap().path().to_vec();
    assert_ne!(linked_path, cell_path);
    let linked_definition: Vec<_> = linked_path
        .into_iter()
        .chain([Step::Follow(gid::Resolution::Document)])
        .collect();
    let frame = editing_frame(&mut world, false);
    assert!((frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == linked_definition)
        .unwrap()
        .select)(&mut world, None));
    key(&mut world, Key::Character("0".into()));
    assert_eq!(
        world
            .model
            .doc
            .cells
            .value(fresh)
            .and_then(f64_convention::read),
        Some(110.0)
    );
    assert_eq!(
        world
            .model
            .doc
            .cells
            .value(shared)
            .and_then(f64_convention::read),
        Some(75.0)
    );
}

fn results(world: &crate::Editor, slots: &[CellId]) -> Vec<f64> {
    use grap::vocabulary::EVALUATE;
    slots
        .iter()
        .filter_map(|key| {
            world
                .sources()
                .resolve_path(&[Step::Key(*key)])
                .and_then(|item| item.as_record()?.get(&EVALUATE))
        })
        .map(|expression| {
            let evaluation = grap::evaluate(expression, &world.sources(), grap::DEFAULT_FUEL);
            assert!(evaluation.completed);
            f64::read(&evaluation.result).expect("numeric lesson result")
        })
        .collect()
}

fn replace_text(world: &mut crate::Editor, path: &[Step], value: &str) {
    let frame = editing_frame(world, false);
    let target = frame
        .descends
        .iter()
        .find(|target| target.path.as_ref() == path)
        .unwrap();
    assert!((target.select)(world, None));
    for (key, modifiers) in [
        (
            Key::Character("a".into()),
            if cfg!(target_os = "macos") {
                Modifiers::META
            } else {
                Modifiers::CONTROL
            },
        ),
        (Key::Character(value.into()), Modifiers::empty()),
    ] {
        assert!(
            editing_frame(world, false)
                .resolve_for_dispatch()
                .dispatch_key(
                    world,
                    &KeyboardEvent {
                        key,
                        modifiers,
                        state: KeyState::Down,
                        ..Default::default()
                    }
                )
        );
    }
}

#[test]
fn website_grap_edits_distinguish_literal_arguments_and_shared_cells() {
    use crate::libraries::{absent, blob, f64, grap as grap_library, name, number};
    use grap::vocabulary::EVALUATE;

    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/grap.gid"
    ))
    .unwrap();
    let slots = [names["first"], names["second"], names["third"]];
    let input_path = [Step::Key(slots[0])];
    let mut world = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            number::ID,
            f64::ID,
            grap_library::ID,
        ])
        .unwrap(),
    );
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(|id| id.simple().to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
    )
    .unwrap();
    assert_eq!(results(&world, &slots), [5.0, 6.0]);

    replace_text(
        &mut world,
        &[
            Step::Key(slots[1]),
            Step::Key(EVALUATE),
            Step::Key(f64::vocabulary::RIGHT),
        ],
        "4",
    );
    assert_eq!(results(&world, &slots), [7.0, 6.0]);
    let root = world.model.doc.root.clone();

    replace_text(
        &mut world,
        &[
            input_path.as_slice(),
            &[Step::Follow(gid::Resolution::Document)],
        ]
        .concat(),
        "5",
    );
    assert_eq!(results(&world, &slots), [9.0, 10.0]);
    replace_text(
        &mut world,
        &[
            Step::Key(slots[2]),
            Step::Key(EVALUATE),
            Step::Key(f64::vocabulary::LEFT),
            Step::Follow(gid::Resolution::Document),
        ],
        "6",
    );
    assert_eq!(results(&world, &slots), [10.0, 12.0]);
    assert_eq!(
        world.model.doc.root, root,
        "edits follow the shared references"
    );

    let result = [
        Step::Key(slots[1]),
        Step::Key(crate::libraries::presentation::vocabulary::RESULT),
    ];
    let frame = editing_frame(&mut world, false);
    let target = frame
        .descends
        .iter()
        .find(|target| target.path.as_ref() == result)
        .unwrap();
    assert!((target.select)(&mut world, None));
    assert!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .source_path()
            .is_none()
    );
    let before = world.model.doc.clone();
    assert!(
        !editing_frame(&mut world, false)
            .resolve_for_dispatch()
            .dispatch_key(
                &mut world,
                &KeyboardEvent {
                    key: Key::Character("9".into()),
                    state: KeyState::Down,
                    ..Default::default()
                }
            )
    );
    assert!(Rc::ptr_eq(&world.model.doc, &before));
}

#[test]
fn website_functions_edit_arguments_body_and_parameter_name() {
    use crate::libraries::{absent, blob, grap as grap_library, number};
    use grap::vocabulary::{BODY, EVALUATE, PARAMS};

    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/functions.gid"
    ))
    .unwrap();
    let slots = [names["first"], names["second"], names["third"]];
    let definition_path = [Step::Key(slots[0])];
    let parameter_position = doc
        .cells
        .value(names["scale"])
        .unwrap()
        .as_record()
        .unwrap()
        .get(&PARAMS)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut world = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            number::ID,
            f64::ID,
            grap_library::ID,
        ])
        .unwrap(),
    );
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(|id| id.simple().to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
    )
    .unwrap();
    assert_eq!(results(&world, &slots), [6.0, 10.0]);
    replace_text(
        &mut world,
        &[
            Step::Key(slots[1]),
            Step::Key(EVALUATE),
            Step::Key(names["x"]),
        ],
        "4",
    );
    assert_eq!(results(&world, &slots), [8.0, 10.0]);
    replace_text(
        &mut world,
        &[
            definition_path.as_slice(),
            &[
                Step::Follow(gid::Resolution::Document),
                Step::Key(BODY),
                Step::Key(f64::vocabulary::RIGHT),
            ],
        ]
        .concat(),
        "3",
    );
    assert_eq!(results(&world, &slots), [12.0, 15.0]);

    let root = world.model.doc.root.clone();
    let definition = world.model.doc.cells.value(names["scale"]).cloned();
    replace_text(
        &mut world,
        &[
            definition_path.as_slice(),
            &[
                Step::Follow(gid::Resolution::Document),
                Step::Key(PARAMS),
                Step::Element(parameter_position),
                Step::Follow(gid::Resolution::Document),
                Step::Key(name::vocabulary::NAME),
            ],
        ]
        .concat(),
        "amount",
    );
    assert_eq!(
        world.model.doc.cells.value(names["x"]).and_then(name::read),
        Some("amount")
    );
    assert_eq!(
        world.model.doc.root, root,
        "call argument labels keep their identities"
    );
    assert_eq!(
        world.model.doc.cells.value(names["scale"]),
        definition.as_ref(),
        "the parameter declaration and body references keep their identities"
    );
    assert_eq!(results(&world, &slots), [12.0, 15.0]);
    let frame = editing_frame(&mut world, false);
    let body_reference = [
        definition_path.as_slice(),
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(BODY),
            Step::Key(f64::vocabulary::LEFT),
        ],
    ]
    .concat();
    assert!(
        frame
            .descends
            .iter()
            .any(|target| target.path.as_ref() == body_reference)
    );
    assert!(
        !frame
            .descends
            .iter()
            .any(|target| target.path.starts_with(&body_reference)
                && target.path.len() > body_reference.len()),
        "the renamed parameter use remains shallow"
    );
}

#[test]
fn tool_profile_click_selects_its_stored_or_computed_occurrence() {
    let tool = crate::libraries::toolpath::cutter::Tool::ball(0.125, 0.22)
        .unwrap()
        .value();
    let owner = new_cell_id();
    let definition = new_cell_id();
    for evaluated in [false, true] {
        let value = if evaluated {
            Value::record([(::grap::vocabulary::EVALUATE, Value::Cell(definition))])
        } else {
            tool.clone()
        };
        let mut cells = Cells::new();
        cells.set_value(definition, tool.clone());
        let doc = Document {
            root: Some(Value::record([(owner, value.clone())])),
            cells,
        };
        let path = vec![Step::Key(owner)];
        let mut bench = BenchContext::new();
        let mut text = TextCtx {
            fonts: &mut bench.fonts,
            layouts: &mut bench.layouts,
            cache: &mut bench.cache,
            scale: 1.0,
        };
        let mut project_tool = || {
            project(
                ProjectDescription {
                    focused: true,
                    computations: None,
                    view: &crate::test_root(),
                    completions: None,
                    sources: Sources {
                        doc: &doc,
                        libraries: &bench.stack.libraries,
                    },
                    root: Some(&value),
                    root_path: &path,
                    selection: None,
                    source_selection: None,
                    annotations: &Annotations::default(),
                    raw: false,
                    styles: &bench.styles,
                    width: 600.0,
                    projection: Some(&bench.stack.projection),
                },
                &mut text,
            )
        };
        let mut selected_path = path.clone();
        if evaluated {
            selected_path.push(Step::Key(
                crate::libraries::presentation::vocabulary::RESULT,
            ));
        }
        let node = project_tool();
        let rect = node.extent.rect_at(Point::ZERO);
        let frame =
            crate::display::widget::frame::place(node, Placement::root(rect), &Default::default());
        let profile = frame
            .descends
            .iter()
            .find(|target| target.path.as_ref() == selected_path)
            .unwrap()
            .rect;
        // Inside the 110 × 180 profile, clear of both its text and delimiters.
        let pointer = Point::new(profile.x0 + 55.0, profile.y0 + 90.0);
        let frame = crate::display::widget::frame::place(
            project_tool(),
            Placement::root(rect),
            &crate::display::widget::HoverInput {
                pointer: Some(pointer),
                ..Default::default()
            },
        );
        let target = Hovered::Tree(Hover::Value(Rc::from(selected_path.clone())));
        assert_eq!(
            frame.claim.as_ref().map(|(_, claim)| claim),
            Some(&Claim::Direct(target.clone()))
        );
        let mut world = crate::test_editor(doc.clone());
        let document = world.model.doc.clone();
        let mut event = press(pointer.x, false);
        event.state.position.y = pointer.y;
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &event,
            &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target)),
        ));
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            selected_path.as_slice()
        );
        assert!(
            Rc::ptr_eq(&world.model.doc, &document),
            "selection must not edit the tool"
        );
    }
}

#[test]
fn sample_text_line_click_mounts_its_own_editor() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/sample.gid"
    )))
    .expect("the sample parses");
    let stack = crate::stack::load();
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let node = project(
        ProjectDescription {
            focused: true,
            computations: None,
            view: &crate::test_root(),
            completions: None,
            sources: Sources {
                doc: &doc,
                libraries: &stack.libraries,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            source_selection: None,
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 852.0,

            projection: Some(&stack.projection),
        },
        &mut tcx,
    );
    let path = vec![
        Step::Key(sample_vocabulary::STYLE),
        Step::Follow(gid::Resolution::Document),
        Step::Key(sample_vocabulary::COLOR),
    ];
    let rect = node.extent.rect_at(Point::new(24.0, 24.0));
    let placed =
        crate::display::widget::frame::place(node, Placement::root(rect), &Default::default());
    let point = placed
        .descends
        .iter()
        .find(|descend| descend.path.as_ref() == &path)
        .expect("color descend")
        .rect
        .center();
    let mut state = ui_events::pointer::PointerState::default();
    state.position.x = point.x;
    state.position.y = point.y;
    let event = ui_events::pointer::PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: ui_events::pointer::PointerInfo {
            pointer_id: Some(ui_events::pointer::PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: ui_events::pointer::PointerType::Mouse,
        },
        state,
    };
    let mut world = crate::test_editor(doc.clone());
    assert!(
        placed
            .resolve_for_dispatch()
            .dispatch_pointer_down(&mut world, &event)
    );
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .map(|selection| selection.path()),
        Some(path.as_slice())
    );

    // The next frame's focused editor still owns pointer-down: a
    // double click selects its word, and a later single click can
    // collapse that selection to a new caret. This is distinct from
    // the first click above, which mounts the editor.
    let mut frame_fonts = parley::FontContext::new();
    let mut frame_layouts = parley::LayoutContext::new();
    let mut frame_cache = puri::text::TextCache::default();
    let mut frame_tcx = TextCtx {
        fonts: &mut frame_fonts,
        layouts: &mut frame_layouts,
        scale: 1.0,
        cache: &mut frame_cache,
    };
    let active = project(
        ProjectDescription {
            focused: true,
            computations: None,
            view: &crate::test_root(),
            completions: None,
            sources: Sources {
                doc: &world.model.doc,
                libraries: &world.stack.libraries,
            },
            root: world.model.doc.root.as_ref(),
            root_path: &[],
            selection: world.model.selection.as_ref(),
            source_selection: world.model.selection.as_ref(),
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 852.0,

            projection: Some(&stack.projection),
        },
        &mut frame_tcx,
    );
    let active =
        crate::display::widget::frame::place(active, Placement::root(rect), &Default::default());
    let line = active
        .descends
        .iter()
        .find(|descend| descend.path.as_ref() == &path)
        .expect("active color descend")
        .rect;
    let mut double = event.clone();
    double.state.position.x = line.center().x;
    double.state.position.y = line.center().y;
    double.state.count = 2;
    let handler = active.resolve_for_dispatch();
    assert!(handler.dispatch_pointer_down(&mut world, &double));
    let selection = world.model.selection.as_ref().unwrap().edit().unwrap();
    let (anchor, focus) = selection.selection_offsets();
    assert_ne!(anchor, focus);

    let mut single = double;
    single.state.position.x = line.x0 + 1.0;
    single.state.count = 1;
    assert!(handler.dispatch_pointer_down(&mut world, &single));
    let selection = world.model.selection.as_ref().unwrap().edit().unwrap();
    let (anchor, focus) = selection.selection_offsets();
    assert_eq!(anchor, focus);
}

fn target() -> Hovered {
    crate::libraries::test_widgets::hover(vec![])
}

fn press(x: f64, pick: bool) -> PointerButtonEvent {
    PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: PointerState {
            position: (x, 5.0).into(),
            modifiers: if pick {
                Modifiers::META | Modifiers::CONTROL
            } else {
                Modifiers::empty()
            },
            ..Default::default()
        },
    }
}

fn gesture_place(
    layout: crate::display::Layout<crate::Editor, Hovered>,
    readonly: bool,
) -> crate::display::widget::HoverCallback<crate::Editor, Hovered> {
    let crate::display::recording::Recorded::Before { before, .. } =
        crate::display::recording::record(&layout)
    else {
        panic!("widget wrapper");
    };
    crate::display::test_support::with_context(
        &crate::display::test_support::NoProject,
        |context| {
            let mut cx = context.inputs.clone();
            if readonly {
                cx.edits = cx.edits.detached(vec![]);
            }
            before(&mut crate::display::widget::Context {
                inputs: &cx,
                project: context.project,
                path: context.path,
                value: context.value,
                text: &mut *context.text,
            })
        },
    )
}

fn drag_frame(
    place: crate::display::widget::HoverCallback<crate::Editor, Hovered>,
    covered: bool,
) -> crate::placed::HoverOutput<crate::Editor> {
    let extent = Extent {
        width: 20.0,
        ascent: 0.0,
        descent: 20.0,
    };
    let node =
        crate::display::widget::before_place(leaf(extent, |_, _| {}), move |placement, output| {
            place(output, placement)
        });
    let placement = Placement::new(
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Rect::new(0.0, 0.0, 10.0, 20.0),
    );
    let placed = crate::display::widget::frame::place(
        placed::in_view(node, crate::test_root()),
        placement,
        &Default::default(),
    );
    if covered {
        measured::Output::over(
            placed,
            crate::display::widget::frame::place(
                leaf(extent, |p, placement| p.occlude(placement)),
                placement,
                &Default::default(),
            ),
        )
    } else {
        placed
    }
}

#[test]
fn state_drag_press_composes_selection_and_start_in_pointer_order() {
    for (accepts, covered) in [(true, false), (false, false), (true, true)] {
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));
        let select_log = log.clone();
        let start_log = log.clone();
        let frame = drag_frame(
            gesture_place(
                crate::display::on_state_drag(
                    crate::display::row(0.0, []),
                    target(),
                    Rc::new(move |world: &mut crate::Editor| {
                        select_log.borrow_mut().push("select");
                        if accepts {
                            crate::editing::select(world, &crate::test_root(), &[]);
                        }
                        accepts
                    }),
                    Rc::new(move || {
                        start_log.borrow_mut().push("start");
                        Box::new(|_, _| Value::record([]))
                    }),
                ),
                false,
            ),
            covered,
        );
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press(5.0, false),
            &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target())),
        );
        assert_eq!(handled, covered || accepts);
        assert_eq!(world.gesture.is_some(), accepts && !covered);
        assert_eq!(world.model.selection.is_some(), accepts && !covered);
        assert_eq!(
            *log.borrow(),
            if covered {
                vec![]
            } else if accepts {
                vec!["select", "start"]
            } else {
                vec!["select"]
            }
        );
    }
}

#[test]
fn state_drag_starts_only_at_a_visible_primary_contact_in_its_own_view() {
    for (x, button, pointer_type, owns_view, expected) in [
        (
            5.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            true,
        ),
        (5.0, None, PointerType::Touch, true, true),
        (
            15.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            25.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            5.0,
            Some(PointerButton::Secondary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            5.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            false,
            false,
        ),
    ] {
        let frame = drag_frame(
            gesture_place(
                crate::display::on_state_drag(
                    crate::display::row(0.0, []),
                    target(),
                    Rc::new(|_| true),
                    Rc::new(|| Box::new(|_, _| Value::record([]))),
                ),
                false,
            ),
            false,
        );
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        let mut event = press(x, false);
        event.button = button;
        event.pointer.pointer_type = pointer_type;
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &event,
            &mut placed::DispatchContext::new(owns_view.then(crate::test_root), Some(target())),
        );
        assert_eq!(handled, expected);
        assert_eq!(world.gesture.is_some(), expected);
    }
}

#[test]
fn scrub_start_respects_pending_selection_and_visible_view_geometry() {
    for (pending, covered, x, pick, owns_view, expected) in [
        (false, false, 5.0, true, true, true),
        (true, false, 5.0, true, true, false),
        (false, true, 5.0, true, true, false),
        (false, false, 15.0, true, true, false),
        (false, false, 5.0, false, true, false),
        (false, false, 5.0, true, false, false),
    ] {
        let frame = drag_frame(
            gesture_place(
                crate::libraries::number::scrub::on_scrub(
                    crate::display::row(0.0, []),
                    target(),
                    Rc::new(|| {
                        Box::new(|_| crate::libraries::number::scrub::ScrubUpdate {
                            value: f64_convention::value(13.0),
                            spelling: Some("13".into()),
                        })
                    }),
                ),
                false,
            ),
            covered,
        );
        let mut world = crate::test_editor(Document {
            root: Some(f64_convention::value(1.0)),
            cells: Cells::new(),
        });
        if pending {
            world.model.selection = Some(pending_value(&crate::test_root(), vec![]));
        }
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press(x, pick),
            &mut placed::DispatchContext::new(owns_view.then(crate::test_root), Some(target())),
        );
        assert_eq!(handled, covered || expected);
        assert_eq!(world.gesture.is_some(), expected);
        if expected {
            world.advance_gesture(&[Point::new(50.0, 5.0)]);
            assert_eq!(world.model.doc.root, Some(f64_convention::value(13.0)));
            assert!(world.model.history.can_undo());
        }
    }
}

#[test]
fn scrub_declines_for_pending_pick_and_raw_contact_takes_precedence() {
    use crate::display::widget::{before_place, interaction::target_action};

    for (pending, raw) in [(false, false), (true, false), (false, true), (true, true)] {
        let field = new_cell_id();
        let value = f64_convention::value(1.0);
        let mut world = crate::test_editor(Document {
            root: Some(value.clone()),
            cells: Cells::new(),
        });
        if pending {
            world.model.selection =
                Some(pending_value(&crate::test_root(), vec![Step::Key(field)]));
        }
        let before = world.model.doc.clone();
        let raw_contacts = Rc::new(std::cell::Cell::new(0));
        let contacts = raw_contacts.clone();
        let scrub = gesture_place(
            crate::libraries::number::scrub::on_scrub(
                crate::display::row(0.0, []),
                target(),
                Rc::new(|| {
                    Box::new(|_| crate::libraries::number::scrub::ScrubUpdate {
                        value: f64_convention::value(13.0),
                        spelling: None,
                    })
                }),
            ),
            false,
        );
        let pick = target_action(
            target(),
            Rc::new(move |world: &mut crate::Editor| world.pick_identity(value.clone())),
            true,
            crate::editing::picking,
            PartialEq::eq,
        );
        let extent = Extent {
            width: 20.0,
            ascent: 0.0,
            descent: 20.0,
        };
        let node = before_place(
            before_place(
                leaf(extent, move |p, _| {
                    p.handler().on_pointer_down(move |_, _| {
                        if raw {
                            contacts.set(contacts.get() + 1);
                        }
                        raw
                    });
                }),
                move |placement, output| scrub(output, placement),
            ),
            move |placement, output| pick(output, placement),
        );
        let frame = crate::display::widget::frame::place(
            placed::in_view(node, crate::test_root()),
            Placement::root(extent.rect_at(Point::ZERO)),
            &Default::default(),
        );
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press(5.0, true),
            &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target())),
        ));
        assert_eq!(raw_contacts.get(), usize::from(raw));
        assert_eq!(world.gesture.is_some(), !pending && !raw);
        if pending && !raw {
            assert_eq!(
                world.sources().resolve_path(&[Step::Key(field)]),
                Some(&f64_convention::value(1.0))
            );
            let selected = world.model.selection.as_ref().unwrap();
            assert_eq!(selected.path(), &[Step::Key(field)]);
            assert_eq!(selected.stage(&world.sources()), Stage::Edge);
            assert!(world.model.history.can_undo());
        } else {
            assert!(Rc::ptr_eq(&before, &world.model.doc));
        }
    }
}

#[test]
fn readonly_gesture_controls_do_not_start_or_construct_edit_runs() {
    for layout in [
        crate::libraries::number::scrub::on_scrub(
            crate::display::row(0.0, []),
            target(),
            Rc::new(|| panic!("read-only scrub")),
        ),
        crate::display::on_point(
            crate::display::row(0.0, []),
            Rc::new(|_| panic!("read-only point control")),
        ),
    ] {
        let frame = drag_frame(gesture_place(layout, true), false);
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        assert!(
            !frame
                .handler
                .is_some_and(|handler| handler.dispatch_pointer_down_with(
                    &mut world,
                    &press(5.0, true),
                    &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target()))
                ))
        );
        assert!(world.gesture.is_none());
    }
}
