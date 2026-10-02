use super::*;
use crate::libraries::f64 as f64_convention;

#[test]
fn website_command_modifier_is_a_host_input_for_editing_and_source_picking() {
    use puri::keyboard::CommandModifier;
    for (command, modifiers, other) in [
        (CommandModifier::Meta, Modifiers::META, Modifiers::CONTROL),
        (
            CommandModifier::Control,
            Modifiers::CONTROL,
            Modifiers::META,
        ),
    ] {
        let mut world = crate::test_editor(Document {
            root: Some(text::value("hello")),
            cells: Cells::new(),
        });
        world.command_modifier = command;
        replace_text(&mut world, &[], "changed");
        assert_eq!(
            text::read(world.model.doc.root.as_ref().unwrap()),
            Some("changed")
        );
        for (pressed, expected) in [(modifiers, true), (other, false)] {
            let mut event = press(0.0, false);
            event.state.modifiers = pressed;
            assert_eq!(crate::modifiers::picking(command)(&event), expected);
            assert_eq!(crate::modifiers::primary_edit(command)(&event), !expected);
            let undo = KeyboardEvent {
                key: Key::Character("z".into()),
                modifiers: pressed,
                state: KeyState::Down,
                ..Default::default()
            };
            assert_eq!(
                crate::menu::shortcut(&undo, command),
                expected.then_some(crate::command::Command::Doc(
                    crate::command::DocCommand::Undo
                ))
            );
        }
    }
}

#[test]
fn empty_slots_make_values_through_the_picker() {
    let (doc, names) = crate::gid_text::parse(EMPTY_SLOTS).unwrap();
    let mut world = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            crate::libraries::blob::ID,
            crate::libraries::number::ID,
            f64::ID,
        ])
        .unwrap(),
    );
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &["first", "second", "third"]
                .map(|key| names[key].to_string())
                .join(","),
        ),
        world.stack.projection,
        &world.stack.libraries,
    )
    .unwrap();
    let key = |world: &mut crate::Editor, key, modifiers| {
        let event = KeyboardEvent {
            key,
            modifiers,
            state: KeyState::Down,
            ..Default::default()
        };
        assert!(
            editing_frame(world, false)
                .resolve_for_dispatch()
                .dispatch_key(world, &event)
                || world.insert_key(Default::default(), &event),
            "unhandled {:?} at {:?}",
            event.key,
            world.model.selection.as_ref().map(Selection::path)
        );
    };
    for (slot, typed) in [("first", "42"), ("second", "\"hello\""), ("third", "[")] {
        let frame = editing_frame(&mut world, false);
        let target = frame
            .descends
            .iter()
            .find(|target| target.path.as_ref() == [Step::Key(names[slot])])
            .unwrap();
        assert!((target.select)(&mut world, None));
        for character in typed.chars() {
            key(
                &mut world,
                Key::Character(character.to_string().into()),
                Modifiers::empty(),
            );
        }
        if slot != "third" {
            key(&mut world, Key::Named(NamedKey::Enter), Modifiers::empty());
        }
    }
    let command = match world.command_modifier {
        puri::keyboard::CommandModifier::Meta => Modifiers::META,
        puri::keyboard::CommandModifier::Control => Modifiers::CONTROL,
    };
    key(&mut world, Key::Named(NamedKey::Enter), command);
    key(&mut world, Key::Character("7".into()), Modifiers::empty());
    key(&mut world, Key::Named(NamedKey::Enter), Modifiers::empty());
    let values = world.model.doc.root.as_ref().unwrap().as_record().unwrap();
    assert_eq!(values.get(&names["first"]).and_then(f64::read), Some(42.0));
    assert_eq!(
        values.get(&names["second"]).and_then(text::read),
        Some("hello")
    );
    assert_eq!(
        values.get(&names["third"]),
        Some(&Value::list([f64::value(7.0)]))
    );
}

#[test]
fn enter_on_a_value_that_fills_a_cell_continues_beside_its_reference() {
    let shared = gid::new_cell_id();
    let items = gid::new_cell_id();
    let program = gid::new_cell_id();
    let mut cells = gid::Cells::new();
    cells.set_value(shared, f64::value(7.0));
    cells.set_value(program, f64::value(1.0));
    let doc = gid::Document {
        root: Some(Value::record([
            (items, Value::list([shared.into(), shared.into()])),
            (crate::libraries::grap::vocabulary::GRAP, program.into()),
        ])),
        cells,
    };
    let mut world = crate::test_editor_with_stack(doc, crate::stack::load());
    let first = world
        .model
        .doc
        .root
        .as_ref()
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&items))
        .and_then(Value::as_list)
        .and_then(|elements| elements.keys().next().cloned())
        .unwrap();
    let follow = Step::Follow(gid::Resolution::Document);
    let enter = KeyboardEvent {
        key: Key::Named(NamedKey::Enter),
        state: KeyState::Down,
        ..Default::default()
    };
    // A new element pends beside the reference in its list; a new field
    // pends on the record holding the reference.
    for (value, expected, pending_at) in [
        (
            vec![Step::Key(items), Step::Element(first), follow.clone()],
            Stage::Pending,
            2,
        ),
        (
            vec![Step::Key(crate::libraries::grap::vocabulary::GRAP), follow],
            Stage::Label,
            0,
        ),
    ] {
        let frame = editing_frame(&mut world, false);
        let target = frame
            .descends
            .iter()
            .find(|target| target.path.as_ref() == value.as_slice())
            .expect("the cell's value is selectable");
        assert!((target.select)(&mut world, None));
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_key(&mut world, &enter)
                || world.insert_key(Default::default(), &enter)
        );
        let selection = world.model.selection.as_ref().unwrap();
        assert_eq!(selection.stage(&world.sources()), expected);
        assert_eq!(selection.path().len(), pending_at);
        world.model.selection = None;
    }
}

#[test]
fn list_insertion_through_a_comma_and_whole_list_selection() {
    let (doc, _) = crate::gid_text::parse(FRUIT).unwrap();
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
            if world.command_modifier == puri::keyboard::CommandModifier::Meta {
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
fn website_grap_results_take_no_typing() {
    use crate::libraries::{absent, blob, grap as grap_library, number};
    let (mut world, names) = lesson_world(
        include_str!("../../../../../website/public/lessons/grap.gid"),
        &[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            number::ID,
            f64::ID,
            grap_library::ID,
        ],
        ["first", "second", "third"],
    );
    select_occurrence(
        &mut world,
        &[
            Step::Key(names["second"]),
            Step::Key(crate::libraries::presentation::vocabulary::RESULT),
        ],
    );
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
                    command_modifier: crate::modifiers::native(),
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
    let styles = crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0);
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
            command_modifier: crate::modifiers::native(),
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
            command_modifier: crate::modifiers::native(),
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
            crate::modifiers::picking(crate::modifiers::native()),
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

fn lesson_world(
    file: &str,
    libraries: &[CellId],
    slots: [&str; 3],
) -> (crate::Editor, crate::gid_text::Binders) {
    let (doc, names) = crate::gid_text::parse(file).unwrap();
    let mut world =
        crate::test_editor_with_stack(doc, crate::stack::load_selected(libraries).unwrap());
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(&slots.map(|slot| names[slot].to_string()).join(",")),
        world.stack.projection.clone(),
        &world.stack.libraries,
    )
    .unwrap();
    (world, names)
}

fn select_occurrence(world: &mut crate::Editor, path: &[Step]) {
    let frame = editing_frame(world, false);
    let target = frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == path)
        .unwrap();
    assert!((target.select)(world, None));
}

#[test]
fn tutorial_plain_and_base_slots_show_a_call_without_its_callee() {
    use crate::libraries::{absent, blob, grap as grap_library, number};
    use grap::vocabulary::FUNCTION;
    let (mut world, names) = lesson_world(
        r#"{
          "binders": {
            "name": 02e562654d6d0828d3a7559e6f75fffe,
            "function": 751fca4373debdd0b7e6eb73e08d684b,
            "params": 195b378d0d31d90ab0d7366c15346b70,
            "body": 986143866eda2e2fbf9ab8484357a0c9,
            "f64": ed11fde03b7c2c1ba2fccc3cdba5d561,
            "tree": 7f46dd29d8450afe5b1ef07d72098f91,
            "x": 9aa131305c35532eeb45a876bc8fdc22,
            "call": d3ff654ad635e00b1a21b6ff849231b2,
            "first": 9940ece27410c72a5308a544890ccc71,
            "second": f717b766d250a7b86c5eb842885c4417,
            "third": 5e716c07490849f072b4e9017dd6230d,
          },
          "cells": {
            x: {name: "x"},
            tree: {name: "tree", params: [x], body: x},
            call: {function: tree, x: {f64: 0x0000000000004e40}},
          },
          "root": {first: call, second: call, third: tree},
        }"#,
        &[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            number::ID,
            f64::ID,
            grap_library::ID,
        ],
        ["first", "second", "third"],
    );
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(&format!(
            "{}:plain,{}:raw,{}",
            names["first"], names["second"], names["third"]
        )),
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            number::ID,
            f64::ID,
            grap_library::ID,
        ])
        .unwrap()
        .projection,
        &world.stack.libraries,
    )
    .unwrap();
    let frame = editing_frame(&mut world, false);
    let placed = |path: &[Step]| frame.descends.iter().any(|d| d.path.as_ref() == path);
    let opened = |path: &[Step]| {
        frame
            .descends
            .iter()
            .any(|d| d.path.len() > path.len() && d.path.starts_with(path))
    };
    for slot in ["first", "second"] {
        let callee = [
            Step::Key(names[slot]),
            Step::Follow(gid::Resolution::Document),
            Step::Key(FUNCTION),
        ];
        assert!(placed(&callee), "{slot} shows its call's function");
        assert!(!opened(&callee), "{slot} doesn't inline the definition");
        assert!(placed(&[
            Step::Key(names[slot]),
            Step::Follow(gid::Resolution::Document),
            Step::Key(names["x"]),
        ]));
    }
    // A slot showing the definition itself still opens it.
    assert!(opened(&[Step::Key(names["third"])]));
}

#[test]
fn swapping_libraries_keeps_the_document_and_selection() {
    use crate::libraries::{blob, number};
    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/model.gid"
    ))
    .unwrap();
    let slots = format!("{}", names["first"]);
    let stack_of = |libraries: &[CellId]| {
        let mut stack = crate::stack::load_selected(libraries).unwrap();
        stack.projection =
            crate::web_embed::tutorial_slots(Some(&slots), stack.projection, &stack.libraries)
                .unwrap();
        stack
    };
    let mut world = crate::test_editor_with_stack(
        doc,
        stack_of(&[name::ID, text::ID, blob::ID, number::ID, f64::ID]),
    );
    let planet = [
        Step::Key(names["first"]),
        Step::Follow(gid::Resolution::Document),
        Step::Key(names["planet"]),
    ];
    let bytes: Vec<Step> = [&planet[..], &[Step::Key(text::vocabulary::UTF8)]].concat();
    select_occurrence(&mut world, &planet);
    assert!(
        !editing_frame(&mut world, false)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == bytes.as_slice()),
        "the text library draws the planet as a line of text"
    );
    let document = world.model.doc.clone();
    let mut runner = crate::EditorRunner::new(world);
    runner.stack_changed(stack_of(&[blob::ID]), 1.0, kurbo::Size::new(620.0, 400.0));
    let world = &mut runner.editor;
    assert!(Rc::ptr_eq(&world.model.doc, &document));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), planet);
    assert!(
        editing_frame(world, false)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == bytes.as_slice()),
        "without it, the same value is a record holding bytes"
    );
}

#[test]
fn a_tutorial_slot_without_grap_shows_a_call_by_its_functions_name() {
    use crate::libraries::{blob, number};
    use grap::vocabulary::FUNCTION;
    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/functions.gid"
    ))
    .unwrap();
    let mut stack =
        crate::stack::load_selected(&[name::ID, text::ID, blob::ID, number::ID, f64::ID]).unwrap();
    stack.projection = crate::web_embed::tutorial_slots(
        Some(&names["second"].to_string()),
        stack.projection,
        &stack.libraries,
    )
    .unwrap();
    let mut world = crate::test_editor_with_stack(doc, stack);
    let frame = editing_frame(&mut world, false);
    let callee = [
        Step::Key(names["second"]),
        Step::Key(grap::vocabulary::EVALUATE),
        Step::Key(FUNCTION),
    ];
    assert!(frame.descends.iter().any(|d| d.path.as_ref() == callee));
    assert!(
        !frame
            .descends
            .iter()
            .any(|d| d.path.len() > callee.len() && d.path.starts_with(&callee)),
        "the call shows (scale), not scale's whole definition"
    );
}

/// A document of three empty lists, the middle one selected, driven through
/// the shell's own key handling.
fn three_lists() -> (crate::EditorRunner, [Path; 3]) {
    let doc = Document {
        root: Some(Value::list([
            Value::list([]),
            Value::list([]),
            Value::list([]),
        ])),
        cells: Cells::new(),
    };
    let paths = positions(doc.root.as_ref().unwrap())
        .into_iter()
        .map(|position| vec![Step::Element(position)])
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let mut runner = crate::EditorRunner::new(crate::test_editor(doc));
    let [_, middle, _]: &[Path; 3] = &paths;
    runner.editor.model.selection = Some(make_selection(middle.clone()));
    runner.refresh_frame(1.0, kurbo::Size::new(600.0, 400.0));
    (runner, paths)
}

fn press_key(runner: &mut crate::EditorRunner, key: Key) {
    assert!(runner.keyboard_event(
        &KeyboardEvent {
            key,
            state: KeyState::Down,
            ..Default::default()
        },
        1.0,
        kurbo::Size::new(600.0, 400.0),
    ));
}

#[test]
fn deleting_empties_a_place_then_removing_it_moves_toward_the_key() {
    for (key, lands) in [(NamedKey::Backspace, 0), (NamedKey::Delete, 2)] {
        let (mut runner, paths) = three_lists();
        press_key(&mut runner, Key::Named(key.clone()));
        let hole = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(hole.path(), paths[1], "{key:?} keeps the place selected");
        assert_eq!(hole.stage(&runner.editor.sources()), Stage::Pending);
        assert_eq!(
            runner
                .editor
                .model
                .doc
                .root
                .as_ref()
                .and_then(Value::as_list)
                .map(|list| list.len()),
            Some(2)
        );
        press_key(&mut runner, Key::Named(key.clone()));
        let landed = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(landed.path(), paths[lands], "{key:?} removes the hole");
        assert_eq!(landed.stage(&runner.editor.sources()), Stage::Edge);
    }
}

#[test]
fn escape_returns_a_picker_to_where_it_was_opened() {
    let (mut runner, paths) = three_lists();
    let document = runner.editor.model.doc.clone();
    press_key(&mut runner, Key::Named(NamedKey::Enter));
    assert_eq!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&runner.editor.sources()),
        Stage::Pending
    );
    press_key(&mut runner, Key::Named(NamedKey::Escape));
    let back = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(back.path(), paths[1]);
    assert_eq!(back.stage(&runner.editor.sources()), Stage::Edge);
    assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));
    press_key(&mut runner, Key::Named(NamedKey::Escape));
    assert!(runner.editor.model.selection.is_none());
}

#[test]
fn typing_over_a_value_replaces_it_and_escape_brings_it_back() {
    let (mut runner, paths) = three_lists();
    let document = runner.editor.model.doc.clone();
    press_key(&mut runner, Key::Character("x".into()));
    let replacing = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(replacing.path(), paths[1]);
    assert_eq!(replacing.stage(&runner.editor.sources()), Stage::Pending);
    assert!(
        Rc::ptr_eq(&runner.editor.model.doc, &document),
        "nothing changes until a commit"
    );
    let frame = editing_frame(&mut runner.editor, false);
    assert!(frame.completion.is_some(), "the hole shows its picker");
    assert!(
        !frame
            .descends
            .iter()
            .any(|target| target.path.as_ref() == paths[1].as_slice()
                && target.path.len() > paths[1].len()),
        "the old value is not drawn"
    );
    press_key(&mut runner, Key::Named(NamedKey::Escape));
    let back = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(back.path(), paths[1]);
    assert_eq!(back.stage(&runner.editor.sources()), Stage::Edge);
    assert!(Rc::ptr_eq(&runner.editor.model.doc, &document));

    press_key(&mut runner, Key::Character("{".into()));
    if runner
        .editor
        .model
        .selection
        .as_ref()
        .unwrap()
        .stage(&runner.editor.sources())
        == Stage::Pending
    {
        press_key(&mut runner, Key::Named(NamedKey::Enter));
    }
    assert_eq!(
        runner.editor.sources().resolve_path(&paths[1]),
        Some(&Value::record([]))
    );
    assert!(runner.editor.model.step_history(true));
    assert_eq!(
        runner.editor.sources().resolve_path(&paths[1]),
        Some(&Value::list([])),
        "one undo step restores the replaced value"
    );
}

#[test]
fn typing_an_operator_after_a_number_makes_it_the_left_operand() {
    use crate::libraries::{number::vocabulary as number, u64};
    let size = kurbo::Size::new(600.0, 400.0);
    let typed = |root: Value, keys: &[&str]| {
        let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
            root: Some(root),
            cells: Cells::new(),
        }));
        runner.editor.model.selection = Some(make_selection(vec![]));
        runner.refresh_frame(1.0, size);
        for key in keys {
            runner.keyboard_event(
                &KeyboardEvent {
                    key: Key::Character((*key).into()),
                    state: KeyState::Down,
                    ..Default::default()
                },
                1.0,
                size,
            );
        }
        runner
    };
    for (number, operator, symbol) in [
        (f64::value(2.0), f64::vocabulary::MULTIPLY, "*"),
        (f64::value(2.0), f64::vocabulary::SUBTRACT, "-"),
        (u64::value(6), u64::vocabulary::DIVIDE, "/"),
    ] {
        let runner = typed(number.clone(), &[symbol]);
        assert_eq!(
            runner.editor.model.doc.root,
            Some(grap::call(operator.into(), [(number::LEFT, number)]))
        );
        let selection = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(selection.path(), [Step::Key(number::RIGHT)]);
        assert_eq!(selection.stage(&runner.editor.sources()), Stage::Pending);
    }
    // An exponent's sign belongs to the number it is still becoming.
    let runner = typed(f64::value(2.0), &["e", "-"]);
    assert_eq!(runner.editor.model.doc.root, Some(f64::value(2.0)));
    assert_eq!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .text(),
        "2e-"
    );
}

#[test]
fn a_numbers_other_fields_follow_it_including_one_being_added() {
    let (x, note) = (new_cell_id(), new_cell_id());
    let number = f64::value(1.0)
        .as_record()
        .unwrap()
        .clone()
        .update(note, text::value("hi"));
    let doc = Document {
        root: Some(Value::record([(x, Value::record(number))])),
        cells: Cells::new(),
    };
    let mut runner = crate::EditorRunner::new(crate::test_editor(doc));
    // The number view leaves its own field undrawn, unlike the record view.
    let drawn = |runner: &mut crate::EditorRunner, field| {
        editing_frame(&mut runner.editor, false)
            .descends
            .iter()
            .any(|descend| *descend.path == [Step::Key(x), Step::Key(field)])
    };
    assert!(drawn(&mut runner, note));
    assert!(!drawn(&mut runner, f64::vocabulary::F64));
    runner.editor.model.selection = Some(make_selection(vec![Step::Key(x)]));
    runner.refresh_frame(1.0, kurbo::Size::new(600.0, 400.0));
    let command = match runner.editor.command_modifier {
        puri::keyboard::CommandModifier::Meta => Modifiers::META,
        puri::keyboard::CommandModifier::Control => Modifiers::CONTROL,
    };
    assert!(runner.keyboard_event(
        &KeyboardEvent {
            key: Key::Named(NamedKey::Enter),
            modifiers: command,
            state: KeyState::Down,
            ..Default::default()
        },
        1.0,
        kurbo::Size::new(600.0, 400.0),
    ));
    assert_eq!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&runner.editor.sources()),
        Stage::Label
    );
    assert!(
        editing_frame(&mut runner.editor, false)
            .completion
            .is_some(),
        "the new field's picker is drawn"
    );
    assert!(drawn(&mut runner, note));
    assert!(!drawn(&mut runner, f64::vocabulary::F64));
}

#[test]
fn a_read_only_swatch_selects_its_color() {
    use crate::libraries::presentation::vocabulary::RESULT;
    let color = crate::libraries::color::value(puri::Color::from_rgb8(0x50, 0x96, 0xdc));
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([(grap::vocabulary::EVALUATE, color)])),
        cells: Cells::new(),
    });
    // The evaluated result is computed, so it's read-only.
    let result = vec![Step::Key(RESULT)];
    let frame = settle(editing_frame(&mut world, false));
    let rect = frame
        .descends
        .iter()
        .find(|landmark| landmark.path.as_ref() == result)
        .unwrap()
        .rect;
    assert!(click_at(&mut world, Point::new(rect.x0 + 7.0, rect.center().y)).is_some());
    let selection = world.model.selection.as_ref().unwrap();
    assert_eq!(selection.path(), result);
    assert!(
        selection
            .payload()
            .as_record()
            .is_none_or(|fields| !fields.contains_key(&crate::libraries::color::vocabulary::PICKER)),
        "no picker opens on a read-only color"
    );
}
