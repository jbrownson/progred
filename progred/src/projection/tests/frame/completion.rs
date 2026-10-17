use super::*;

#[test]
fn new_color_completion_opens_without_editing_and_commits_at_the_missing_location() {
    use crate::libraries::color;
    let function = new_cell_id();
    let argument = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(function, grap::lambda([argument], argument.into()));
    // A Grap argument's hole offers the libraries' values, a new color among
    // them; Raw's holes don't.
    for (value, path) in [(
        Some(grap::call(function.into(), [])),
        vec![Step::Key(argument)],
    )] {
        let mut world = crate::test_editor(Document {
            root: value.clone(),
            cells: cells.clone(),
        });
        world.model.selection = Some(crate::selection::pending_with_query(
            &crate::test_root(),
            path.clone(),
            "new color",
        ));
        let frame = editing_frame(&mut world, false);
        let entry = frame
            .completion
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .find(|e| e.display == "new color")
            .unwrap();
        let document = world.model.doc.clone();
        (entry.activate)(&mut world);
        assert!(Rc::ptr_eq(&document, &world.model.doc));
        assert!(!world.model.history.can_undo());
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
        world.model.selection = None;
        assert!(
            Rc::ptr_eq(&document, &world.model.doc),
            "leaving does not insert the seed"
        );
        assert!(!world.model.history.can_undo());
        world.model.selection = Some(pending_value(&crate::test_root(), path.clone()));
        (entry.activate)(&mut world);
        let frame = editing_frame(&mut world, false);
        assert!(frame.completion.is_none(), "picker replaces completion");
        assert!(frame.resolve_for_dispatch().dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Named(NamedKey::Escape),
                state: KeyState::Down,
                ..Default::default()
            }
        ));
        assert!(Rc::ptr_eq(&document, &world.model.doc));
        assert!(editing_frame(&mut world, false).completion.is_some());

        // Re-open the retained action, then click the actual projected color plane.
        (entry.activate)(&mut world);
        let drawing = settle(editing_frame(&mut world, false)).list;
        let plane = drawing
            .0
            .iter()
            .find_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::Rect(rect),
                    brush: peniko::Brush::Gradient(_),
                    transform,
                    ..
                } if rect.width() == puri_widgets::color_picker::WIDTH
                    && rect.height() == puri_widgets::color_picker::PLANE_HEIGHT =>
                {
                    Some(transform.transform_rect_bbox(*rect))
                }
                _ => None,
            })
            .expect("visible color plane");
        let point = plane.center();
        let frame = editing_frame(&mut world, false);
        let mut event = PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: crate::display::test_support::mouse(),
            state: Default::default(),
        };
        event.state.position = (point.x, point.y).into();
        assert!(
            frame
                .resolve_for_dispatch()
                .dispatch_pointer_down(&mut world, &event)
        );
        world.advance_gesture(&[Point::new(plane.x0 + plane.width() * 0.75, point.y)]);
        world.finish_gesture();
        assert!(color::read(world.sources().resolve_path(&path).unwrap()).is_some());
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .stage(&world.sources()),
            Stage::Edge
        );
        assert!(editing_frame(&mut world, false).completion.is_none());
        assert!(
            world.model.step_history(true),
            "one undo for the whole drag"
        );
        assert_eq!(world.model.doc.root, value);
        assert!(!world.model.history.can_undo());
    }
}

#[test]
fn constructor_keys_make_their_offer_from_an_empty_query() {
    let root = crate::test_root();
    let press = |key: &str, modifiers| KeyboardEvent {
        key: Key::Character(key.into()),
        state: KeyState::Down,
        modifiers,
        ..Default::default()
    };
    for labels in [false, true] {
        for key in ["[", "(", "{"] {
            let mut world = crate::test_editor(Document {
                root: labels.then(|| Value::record([])),
                cells: Cells::new(),
            });
            world.model.selection = Some(if labels {
                pending_edge(&root, &world.sources(), vec![]).unwrap()
            } else {
                pending_value(&root, vec![])
            });
            let frame = editing_frame(&mut world, false);
            assert!(
                frame
                    .resolve_for_dispatch()
                    .dispatch_key(&mut world, &press(key, Modifiers::SHIFT))
            );
            if labels && key != "(" {
                assert_eq!(world.model.doc.root, Some(Value::record([])));
                assert_eq!(
                    world
                        .model
                        .selection
                        .as_ref()
                        .unwrap()
                        .edit()
                        .unwrap()
                        .text(),
                    key
                );
            } else {
                let value = world.model.doc.root.as_ref().unwrap();
                if labels {
                    assert_eq!(value.as_record().unwrap().len(), 0);
                    assert!(matches!(
                        world.model.selection.as_ref().unwrap().path(),
                        [Step::Key(_)]
                    ));
                } else {
                    match key {
                        "[" => assert_eq!(value, &Value::list([])),
                        "{" => assert_eq!(value, &Value::record([])),
                        _ => assert!(value.as_cell().is_some()),
                    };
                }
            }
        }
    }
    for modifiers in [Modifiers::CONTROL, Modifiers::META] {
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        world.model.selection = Some(pending_value(&root, vec![]));
        assert!(
            !editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_key(&mut world, &press("[", modifiers))
        );
        assert!(world.model.doc.root.is_none());
    }
    for query in ["\"", "search"] {
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        world.model.selection = Some(crate::selection::pending_with_query(&root, vec![], query));
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_key(&mut world, &press("[", Modifiers::empty()))
        );
        assert!(world.model.doc.root.is_none());
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .edit()
                .unwrap()
                .text(),
            format!("{query}[")
        );
    }
    let mut world = crate::test_editor(Document {
        root: None,
        cells: Cells::new(),
    });
    world.model.selection = Some(pending_value(&root, vec![]));
    let mut release = press("[", Modifiers::empty());
    release.state = KeyState::Up;
    assert!(
        !editing_frame(&mut world, false)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &release)
    );
    world
        .model
        .selection
        .as_mut()
        .unwrap()
        .edit_mut()
        .unwrap()
        .handle_ime(&puri::handler::ImeEvent::Preedit("[".into(), Some((1, 1))));
    assert!(
        !editing_frame(&mut world, false)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &press("[", Modifiers::empty()))
    );
    assert!(
        editing_frame(&mut world, false)
            .resolve_for_dispatch()
            .dispatch_ime(&mut world, &puri::handler::ImeEvent::Commit("[".into()))
    );
    assert!(world.model.doc.root.is_none());
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .text(),
        "["
    );
}

#[test]
fn delimiters_make_constructors_in_raw_s_holes_and_are_text_in_grap_s() {
    let root = crate::test_root();
    let function = Document {
        root: Some(Value::record([(
            grap::vocabulary::PARAMS,
            Value::list([new_cell_id().into()]),
        )])),
        cells: Cells::new(),
    };
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    let body = vec![Step::Key(grap::vocabulary::BODY)];
    let press = |world: &mut crate::Editor, key: &str| {
        editing_frame(world, false)
            .resolve_for_dispatch()
            .dispatch_key(
                world,
                &KeyboardEvent {
                    key: Key::Character(key.into()),
                    state: KeyState::Down,
                    ..Default::default()
                },
            )
    };
    for key in ["(", "[", "{"] {
        // Raw draws a plain document's root: the key makes what it does and
        // opens it, its picker showing.
        let mut world = crate::test_editor(empty.clone());
        world.model.selection = Some(pending_value(&root, vec![]));
        assert!(press(&mut world, key));
        assert!(world.model.doc.root.is_some(), "{key} makes a value");
        let opened = world.model.selection.as_ref().unwrap();
        assert!(
            opened.stage(&world.sources()) != Stage::Edge,
            "{key} opens what it made: {:?}",
            opened.path()
        );
        assert!(
            editing_frame(&mut world, false).completion.is_some(),
            "{key} shows its picker"
        );
        // Grap draws a function's body, and its hole takes the key as text.
        let mut world = crate::test_editor(function.clone());
        world.model.selection = Some(pending_value(&root, body.clone()));
        assert!(press(&mut world, key));
        assert_eq!(world.model.doc.root, function.root, "{key} makes nothing");
        let typing = world.model.selection.as_ref().unwrap();
        assert_eq!(typing.path(), body);
        assert_eq!(typing.edit().unwrap().text(), key);
    }
}

#[test]
fn completion_activation_precedes_the_real_editor_it_covers() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/sample.gid"
    )))
    .expect("the sample parses");
    let stack = crate::stack::load();
    let styles = crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0);
    let mut fonts = crate::fonts::bundled_font_context();
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
        .landmark(&path)
        .expect("color descend")
        .rect
        .center();
    let activated = Rc::new(std::cell::Cell::new(false));
    let clicked = activated.clone();
    let card = crate::display::widget::completion::card::<crate::Editor>(
        &mut tcx,
        &styles,
        &[Entry {
            display: "completion offer".into(),
            detail: None,
            matches: Vec::new(),
            face: crate::display::Face::String,
            source: None,
            activate: Rc::new(move |_| clicked.set(true)),
        }],
        Default::default(),
        |_| Some(0.0),
        |_, _| {},
    );
    let card_rect = card
        .extent
        .rect_at(Point::new(point.x - 8.0, point.y - 8.0));
    let card = crate::display::widget::frame::place(
        card,
        Placement::root(card_rect),
        &placed::HoverInput {
            pointer: Some(point),
            ..Default::default()
        },
    );
    let placed = measured::Output::over(placed, card);
    let Some(Claim::Direct(target)) = placed.claim.clone().map(|(_, claim)| claim) else {
        panic!("direct hover")
    };
    assert_eq!(target, Hovered::Tree(Hover::Entry(0)));
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
    let mut world = crate::test_editor(doc);
    let mut pointer = placed::DispatchContext::new(None, Some(target));
    assert!(placed.resolve_for_dispatch().dispatch_pointer_down_with(
        &mut world,
        &event,
        &mut pointer
    ));
    assert!(world.model.selection.is_none());
    assert!(activated.get());
}
