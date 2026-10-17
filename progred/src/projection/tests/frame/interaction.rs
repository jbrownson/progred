use super::*;
use crate::libraries::test_widgets::press;

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
    // `[` opens the list it makes, so 7 goes inside it.
    for (slot, typed) in [("first", "42"), ("second", "\"hello\""), ("third", "[7")] {
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
                Key::Character(character.to_string()),
                Modifiers::empty(),
            );
        }
        key(&mut world, Key::Named(NamedKey::Enter), Modifiers::empty());
    }
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

/// A document whose left pane renders `render`, beside `program`.
fn rendered_in_a_pane(program: CellId, render: Value, cells: Cells) -> crate::driver::Driver {
    use crate::libraries::{presentation, workspace};
    crate::driver::Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([
                (crate::libraries::grap::vocabulary::GRAP, program.into()),
                (
                    workspace::vocabulary::PANES,
                    Value::record([(
                        workspace::vocabulary::LEFT,
                        Value::list([Value::record([(presentation::vocabulary::RENDER, render)])]),
                    )]),
                ),
            ])),
            cells,
        }),
        kurbo::Size::new(900.0, 600.0),
    )
}

/// The pane's result, and `rest` within it.
fn pane_result(driver: &crate::driver::Driver, rest: &[Step]) -> Vec<Step> {
    use crate::libraries::{presentation, workspace};
    driver.element(
        &[
            Step::Key(workspace::vocabulary::PANES),
            Step::Key(workspace::vocabulary::LEFT),
        ],
        0,
        &[
            [Step::Key(presentation::vocabulary::RESULT)].as_slice(),
            rest,
        ]
        .concat(),
    )
}

#[test]
fn a_missing_cell_named_in_an_error_is_defined_from_the_error() {
    let program = new_cell_id();
    let mut driver = rendered_in_a_pane(program, program.into(), Cells::new());
    let hole = pane_result(
        &driver,
        &[
            Step::Key(crate::libraries::absent::vocabulary::CELL),
            Step::Follow(gid::Resolution::Document),
        ],
    );
    driver.click(&hole);
    {
        let selection = driver.editor().model.selection.as_ref().unwrap();
        assert_eq!(selection.path(), hole);
        assert_eq!(selection.stage(&driver.editor().sources()), Stage::Pending);
        assert_eq!(
            selection.location(),
            Some(Location::in_cell(
                program,
                gid::Resolution::Document,
                Vec::new()
            ))
        );
    }
    driver.type_text("\"drawn");
    driver.press(NamedKey::Enter);
    assert_eq!(
        driver.editor().model.doc.cells.value(program),
        Some(&text::value("drawn"))
    );
    assert!(driver.drawn(&pane_result(&driver, &[])));
    assert!(
        !driver.drawn(&hole),
        "the error gave way to the program's result"
    );
}

#[test]
fn a_quoted_reference_in_a_result_edits_its_cell() {
    let (program, quoted) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(quoted, text::value("before"));
    let render = grap::call(
        crate::libraries::control::vocabulary::QUOTE.into(),
        [(grap::vocabulary::EXPRESSION, quoted.into())],
    );
    let mut driver = rendered_in_a_pane(program, render, cells);
    let definition = pane_result(&driver, &[Step::Follow(gid::Resolution::Document)]);
    driver.double_click(&definition);
    driver.type_text("after");
    assert_eq!(
        driver.editor().model.doc.cells.value(quoted),
        Some(&text::value("after"))
    );
    assert_eq!(
        driver.editor().model.doc.cells.value(program),
        None,
        "nothing else was written"
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
    let target = frame.descends.landmark(path).unwrap();
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
fn grap_results_take_no_typing() {
    use crate::libraries::{absent, blob, grap as grap_library, number};
    let (mut world, names) = lesson_world(
        include_str!("../../../../fixtures/grap.gid"),
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
    assert!(world.model.selection.as_ref().unwrap().location().is_none());
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
        let profile = frame.descends.landmark(&selected_path).unwrap().rect;
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
    let mut frame_fonts = crate::fonts::bundled_font_context();
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
        .landmark(&path)
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

fn lesson_world(
    file: &str,
    libraries: &[CellId],
    slots: [&str; 3],
) -> (crate::Editor, crate::gid_text::Binders) {
    let (doc, names) = crate::gid_text::parse(file).unwrap();
    let mut world =
        crate::test_editor_with_stack(doc, crate::stack::load_selected(libraries).unwrap());
    tutorial_slots(&mut world, &slots.map(|slot| names[slot]));
    (world, names)
}

fn select_occurrence(world: &mut crate::Editor, path: &[Step]) {
    let frame = editing_frame(world, false);
    let target = frame.descends.landmark(path).unwrap();
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
    let (doc, names) =
        crate::gid_text::parse(include_str!("../../../../fixtures/model.gid")).unwrap();
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
    let (doc, names) =
        crate::gid_text::parse(include_str!("../../../../fixtures/functions.gid")).unwrap();
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
        press_key(&mut runner, Key::Named(key));
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
        press_key(&mut runner, Key::Named(key));
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
fn a_new_cell_list_or_record_opens_for_what_goes_inside_it() {
    let made = |key: &str| {
        let (mut runner, paths) = three_lists();
        press_key(&mut runner, Key::Named(NamedKey::Backspace));
        press_key(&mut runner, Key::Character(key.into()));
        let place = paths[1].clone();
        (runner, place)
    };
    let typed = |runner: &mut crate::EditorRunner, text: &str| {
        for key in text.chars() {
            press_key(runner, Key::Character(key.to_string().into()));
        }
        press_key(runner, Key::Named(NamedKey::Enter));
    };
    let value = |runner: &crate::EditorRunner, path: &[Step]| {
        runner.editor.sources().resolve_path(path).cloned()
    };
    let cell = [Step::Follow(gid::Resolution::Document)];
    for key in ["(", "[", "{"] {
        let (mut runner, place) = made(key);
        {
            let sources = runner.editor.sources();
            let inside = runner.editor.model.selection.as_ref().unwrap();
            let (path, stage) = match key {
                "(" => (&[&place[..], &cell].concat()[..], Stage::Pending),
                "[" => (inside.path(), Stage::Pending),
                _ => (&place[..], Stage::Label),
            };
            assert_eq!(inside.path(), path, "{key} selects inside what it made");
            assert_eq!(inside.stage(&sources), stage, "{key}");
            if key == "[" {
                assert!(matches!(
                    inside.path().split_last(),
                    Some((Step::Element(_), list)) if list == place
                ));
            }
        }
        assert!(
            editing_frame(&mut runner.editor, false)
                .completion
                .is_some(),
            "{key} shows its picker"
        );
        press_key(&mut runner, Key::Named(NamedKey::Escape));
        let back = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(back.path(), place, "Escape steps back out of {key}");
        assert_eq!(back.stage(&runner.editor.sources()), Stage::Edge);
    }

    let (mut runner, place) = made("(");
    typed(&mut runner, "5");
    assert!(value(&runner, &[&place[..], &cell].concat()).is_some());
    let (mut runner, place) = made("[");
    typed(&mut runner, "5");
    assert_eq!(value(&runner, &place).unwrap().as_list().unwrap().len(), 1);
    let (mut runner, place) = made("{");
    typed(&mut runner, "width");
    typed(&mut runner, "5");
    assert_eq!(
        value(&runner, &place).unwrap().as_record().unwrap().len(),
        1
    );
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
fn enter_ends_typing_in_a_value_before_it_adds_anything_beside() {
    let size = kurbo::Size::new(600.0, 400.0);
    let x = new_cell_id();
    let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
        root: Some(Value::record([(x, f64::value(2.0))])),
        cells: Cells::new(),
    }));
    runner.editor.model.selection = Some(make_selection(vec![Step::Key(x)]));
    runner.refresh_frame(1.0, size);
    let press = |runner: &mut crate::EditorRunner, key: Key| {
        runner.keyboard_event(
            &KeyboardEvent {
                key,
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            size,
        );
    };
    // "25e" is still becoming a number, so 25 is what was written.
    for typed in ["5", "e"] {
        press(&mut runner, Key::Character(typed.into()));
    }
    let selection = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selection.edit().unwrap().text(), "25e");
    press(&mut runner, Key::Named(NamedKey::Enter));
    let selection = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selection.path(), [Step::Key(x)]);
    assert_eq!(selection.stage(&runner.editor.sources()), Stage::Edge);
    assert!(selection.edit().is_none(), "the typing ended");
    assert_eq!(
        runner.editor.model.doc.root,
        Some(Value::record([(x, f64::value(25.0))])),
        "and nothing was added beside it"
    );
    // Enter on the value itself continues beside it, as before.
    press(&mut runner, Key::Named(NamedKey::Enter));
    let selection = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selection.stage(&runner.editor.sources()), Stage::Label);
}

#[test]
fn tab_goes_from_empty_slot_to_empty_slot_keeping_what_was_typed() {
    let size = kurbo::Size::new(600.0, 400.0);
    use crate::libraries::name;
    let function = new_cell_id();
    let [x, height] = [new_cell_id(), new_cell_id()];
    let mut cells = Cells::new();
    let tree = grap::lambda([x, height], Value::record([]));
    let tree = tree
        .as_record()
        .unwrap()
        .clone()
        .update(name::vocabulary::NAME, text::value("tree"));
    cells.set_value(function, Value::record(tree));
    cells.set_value(x, name::record("x", []));
    cells.set_value(height, name::record("height", []));
    let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
        root: Some(grap::call(function.into(), [])),
        cells,
    }));
    runner.refresh_frame(1.0, size);
    let press = |runner: &mut crate::EditorRunner, key: Key, modifiers: Modifiers| {
        runner.keyboard_event(
            &KeyboardEvent {
                key,
                state: KeyState::Down,
                modifiers,
                ..Default::default()
            },
            1.0,
            size,
        )
    };
    let tab = |runner: &mut crate::EditorRunner, modifiers| {
        press(runner, Key::Named(NamedKey::Tab), modifiers)
    };
    let selected = |runner: &crate::EditorRunner| {
        let selection = runner.editor.model.selection.as_ref().unwrap();
        (
            selection.path().to_vec(),
            selection.stage(&runner.editor.sources()),
        )
    };
    let typing = |runner: &mut crate::EditorRunner, text: &str| {
        for typed in text.chars() {
            press(
                runner,
                Key::Character(typed.to_string().into()),
                Modifiers::empty(),
            );
        }
    };
    let cleared = |runner: &crate::EditorRunner| runner.editor.model.selection.is_none();
    // From nothing, Tab starts at the first empty slot. There's none before
    // it, so Shift+Tab clears the selection, and the next starts at the end.
    assert!(tab(&mut runner, Modifiers::empty()));
    assert_eq!(selected(&runner), (vec![Step::Key(x)], Stage::Pending));
    assert!(tab(&mut runner, Modifiers::SHIFT));
    assert!(cleared(&runner));
    assert!(tab(&mut runner, Modifiers::SHIFT));
    assert_eq!(selected(&runner), (vec![Step::Key(height)], Stage::Pending));
    assert!(tab(&mut runner, Modifiers::SHIFT));
    assert_eq!(selected(&runner), (vec![Step::Key(x)], Stage::Pending));
    // What was typed goes in, and Tab moves on to the next slot.
    typing(&mut runner, "172");
    assert!(tab(&mut runner, Modifiers::empty()));
    let root = runner.editor.model.doc.root.clone().unwrap();
    assert!(root.as_record().unwrap().get(&x).is_some(), "x keeps 172");
    assert_eq!(selected(&runner), (vec![Step::Key(height)], Stage::Pending));
    // Past the last slot, a window's Tab clears the selection, and the next
    // starts over; a browser moves focus on to its page instead.
    assert!(tab(&mut runner, Modifiers::empty()));
    assert!(cleared(&runner));
    assert!(tab(&mut runner, Modifiers::empty()));
    assert_eq!(selected(&runner), (vec![Step::Key(height)], Stage::Pending));
    // What's typed in the last slot still goes in, and stays selected.
    typing(&mut runner, "80");
    assert!(tab(&mut runner, Modifiers::empty()));
    let root = runner.editor.model.doc.root.clone().unwrap();
    assert!(
        root.as_record().unwrap().get(&height).is_some(),
        "height keeps 80"
    );
    assert_eq!(selected(&runner), (vec![Step::Key(height)], Stage::Edge));
    // With every slot filled, Tab clears the selection, then has nowhere to go.
    assert!(tab(&mut runner, Modifiers::empty()));
    assert!(cleared(&runner));
    assert!(!tab(&mut runner, Modifiers::empty()));
}

#[test]
fn a_numbers_other_fields_follow_it_but_no_key_adds_one() {
    let size = kurbo::Size::new(600.0, 400.0);
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
    // Drawn as one piece, the number takes no field from Enter, within it or
    // beside one of the others it has; a rawer view of it can add one.
    let before = runner.editor.model.doc.root.clone();
    for (path, modifiers) in [
        (vec![Step::Key(x)], crate::modifiers::command()),
        (vec![Step::Key(x), Step::Key(note)], Modifiers::empty()),
    ] {
        runner.editor.model.selection = Some(make_selection(path.clone()));
        runner.refresh_frame(1.0, size);
        assert!(!runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(NamedKey::Enter),
                modifiers,
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            size,
        ));
        let selection = runner.editor.model.selection.as_ref().unwrap();
        assert_eq!(
            (
                selection.path().to_vec(),
                selection.stage(&runner.editor.sources())
            ),
            (path, Stage::Edge)
        );
    }
    assert_eq!(runner.editor.model.doc.root, before);
    assert!(drawn(&mut runner, note));
}

#[test]
fn enter_beside_text_in_a_list_adds_an_element_but_nothing_within_the_text() {
    let size = kurbo::Size::new(600.0, 400.0);
    let doc = Document {
        root: Some(Value::list([text::value("a"), text::value("b")])),
        cells: Cells::new(),
    };
    let mut runner = crate::EditorRunner::new(crate::test_editor(doc));
    let first = runner
        .editor
        .sources()
        .resolve_path(&[])
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let enter = |runner: &mut crate::EditorRunner, modifiers| {
        runner.editor.model.selection = Some(make_selection(vec![Step::Element(first.clone())]));
        runner.refresh_frame(1.0, size);
        runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(NamedKey::Enter),
                modifiers,
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            size,
        )
    };
    // The list takes a new element after the text.
    assert!(enter(&mut runner, Modifiers::empty()));
    let selection = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selection.stage(&runner.editor.sources()), Stage::Pending);
    assert!(matches!(
        selection.path(),
        [Step::Element(position)] if *position > first
    ));
    // The text, drawn as one piece, takes nothing within it.
    assert!(!enter(&mut runner, crate::modifiers::command()));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        [Step::Element(first.clone())]
    );
}

#[test]
fn enter_adds_no_field_within_a_number_unless_its_view_shows_a_record() {
    let size = kurbo::Size::new(600.0, 400.0);
    let doc = Document {
        root: Some(f64::value(5.0)),
        cells: Cells::new(),
    };
    let enter = |editor: crate::Editor, command: bool| {
        let mut runner = crate::EditorRunner::new(editor);
        runner.editor.model.selection = Some(make_selection(Vec::new()));
        runner.refresh_frame(1.0, size);
        let modifiers = if command {
            crate::modifiers::command()
        } else {
            Modifiers::empty()
        };
        runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(NamedKey::Enter),
                modifiers,
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            size,
        );
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&runner.editor.sources())
    };
    // A number at the root has nothing beside it, and nothing within.
    for command in [false, true] {
        assert_eq!(enter(crate::test_editor(doc.clone()), command), Stage::Edge);
    }
    // Without the number library it reads as the record it is, which takes one.
    let raw =
        || crate::test_editor_with_stack(doc.clone(), crate::stack::load_selected(&[]).unwrap());
    for command in [false, true] {
        assert_eq!(enter(raw(), command), Stage::Label);
    }
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
    let rect = frame.descends.landmark(&result).unwrap().rect;
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
