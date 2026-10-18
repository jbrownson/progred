use super::*;
use crate::display::{Layout, partial, widget};
use crate::libraries::{f64, text};
use crate::projection::Projection;
use gid::{Cells, Document};
use puri::handler::{HasHandler, PointerButtonEvent, PointerInfo, PointerState, ScrollOutcome};
use std::cell::RefCell;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyState};
use ui_events::pointer::PointerUpdate;

const VIEWPORT: Size = Size::new(500.0, 400.0);
type Log = Rc<RefCell<Vec<(&'static str, f64)>>>;

fn instrumented_runner(log: &Log) -> EditorRunner {
    let mut editor = crate::test_editor(Document {
        root: Some(f64::value(0.0)),
        cells: Cells::new(),
    });
    editor.stack.projection = Projection::new([partial({
        let log = log.clone();
        move |input| {
            let value = f64::read(input.value?)?;
            log.borrow_mut().push(("project", value));
            let log = log.clone();
            Some(Layout::widget(Rc::new(move |_| {
                let log = log.clone();
                widget::leaf(
                    widget::Extent {
                        width: 100.0,
                        ascent: 30.0,
                        descent: 0.0,
                    },
                    move |output, _| {
                        log.borrow_mut().push(("hover", value));
                        output.after_hover(move |_, effects| {
                            log.borrow_mut().push(("bind", value));
                            effects.handler().on_key({
                                let log = log.clone();
                                move |editor: &mut Editor, event| {
                                    if event.key == Key::Character("x".into()) {
                                        log.borrow_mut().push(("key", value));
                                        Rc::make_mut(&mut editor.model.doc).root =
                                            Some(f64::value(value + 1.0));
                                        true
                                    } else {
                                        false
                                    }
                                }
                            });
                            effects.handler().on_scroll({
                                let log = log.clone();
                                move |editor, event| {
                                    log.borrow_mut().push(("scroll", value));
                                    Rc::make_mut(&mut editor.model.doc).root =
                                        Some(f64::value(value + 1.0));
                                    ScrollOutcome::consume(event)
                                }
                            });
                            effects.handler().on_pointer_move({
                                let log = log.clone();
                                move |_, _| {
                                    log.borrow_mut().push(("move", value));
                                    false
                                }
                            });
                            effects.renders.push(Box::new(move |_| {
                                log.borrow_mut().push(("paint", value));
                            }));
                        });
                    },
                )
            })))
        }
    })]);
    EditorRunner::new(editor)
}

fn key() -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Character("x".into()),
        state: KeyState::Down,
        ..Default::default()
    }
}

#[test]
fn selected_cell_enters_with_right_and_backs_out_through_each_container() {
    use crate::libraries::name;
    use gid::{Step, Value};
    use ui_events::keyboard::NamedKey;
    let cell = gid::new_cell_id();
    let list = Value::list([Value::Cell(cell)]);
    let cell_path = vec![Step::Element(
        list.as_list().unwrap().keys().next().unwrap().clone(),
    )];
    let name_path = cell_path
        .iter()
        .cloned()
        .chain([
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ])
        .collect::<Vec<_>>();
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        name::record("tilt", f64::value(45.0).as_record().unwrap().clone()),
    );
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(list),
        cells,
    }));
    runner.refresh_frame(1.0, VIEWPORT);
    let press = |key| KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        ..Default::default()
    };
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, cell_path.clone()));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(!runner.keyboard_event(&press(NamedKey::ArrowDown), 1.0, VIEWPORT));
    assert!(runner.keyboard_event(&press(NamedKey::ArrowRight), 1.0, VIEWPORT));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        name_path
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
    // Backward traversal visits the cell, then its enclosing list.
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        cell_path
    );
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    assert!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .path()
            .is_empty()
    );
    assert!(!runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
}

#[test]
fn arrows_edit_text_before_routing_and_arrive_at_the_near_text_edge() {
    use ui_events::keyboard::NamedKey;
    let list = gid::Value::list([text::value("ab"), text::value("cd")]);
    let paths: Vec<_> = list
        .as_list()
        .unwrap()
        .keys()
        .map(|position| vec![gid::Step::Element(position.clone())])
        .collect();
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(list),
        cells: Cells::new(),
    }));
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, paths[0].clone()));
    runner.refresh_frame(1.0, VIEWPORT);
    let press = |key| KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        ..Default::default()
    };
    // Plain selection defaults to the end; Left first moves within "ab".
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    let selected = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), paths[0]);
    assert_eq!(selected.edit().unwrap().selection_offsets(), (1, 1));
    assert!(runner.keyboard_event(&press(NamedKey::ArrowRight), 1.0, VIEWPORT));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        paths[0]
    );
    // Only at the boundary does Right hand off to the list's next item.
    assert!(runner.keyboard_event(&press(NamedKey::ArrowRight), 1.0, VIEWPORT));
    let selected = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), paths[1]);
    assert_eq!(selected.edit().unwrap().selection_offsets(), (0, 0));
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    let selected = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), paths[0]);
    assert_eq!(selected.edit().unwrap().selection_offsets(), (2, 2));
}

#[test]
fn modified_arrows_edit_text_then_navigate_at_its_boundary() {
    use puri::keyboard::CommandModifier;
    use ui_events::keyboard::NamedKey;
    for (command, modifiers) in [
        (CommandModifier::Meta, Modifiers::ALT),
        (CommandModifier::Meta, Modifiers::META),
        (CommandModifier::Control, Modifiers::CONTROL),
    ] {
        let list = gid::Value::list([text::value("ab"), text::value("cd")]);
        let paths: Vec<_> = list
            .as_list()
            .unwrap()
            .keys()
            .map(|p| vec![gid::Step::Element(p.clone())])
            .collect();
        let mut editor = crate::test_editor(Document {
            root: Some(list),
            cells: Cells::new(),
        });
        editor.command_modifier = command;
        let root = editor.model.workspace.document_root().clone();
        editor.model.selection = Some(selection::Selection::edge(&root, paths[1].clone()));
        let mut runner = EditorRunner::new(editor);
        runner.refresh_frame(1.0, VIEWPORT);
        for (key, index, offset) in [
            (NamedKey::ArrowLeft, 1, 0), // Word/line movement takes precedence.
            (NamedKey::ArrowLeft, 0, 2), // At the boundary, navigate instead.
            (NamedKey::ArrowRight, 1, 0),
            (NamedKey::ArrowRight, 1, 2),
        ] {
            assert!(runner.keyboard_event(
                &KeyboardEvent {
                    key: Key::Named(key),
                    state: KeyState::Down,
                    modifiers,
                    ..Default::default()
                },
                1.0,
                VIEWPORT
            ));
            let selected = runner.editor.model.selection.as_ref().unwrap();
            assert_eq!(selected.path(), paths[index]);
            assert_eq!(
                selected.edit().unwrap().selection_offsets(),
                (offset, offset)
            );
        }
    }
}

#[test]
fn arrows_route_between_a_numbers_name_and_digits_and_out_to_siblings() {
    use crate::libraries::name;
    use ui_events::keyboard::NamedKey;
    let list = gid::Value::list([
        text::value("before"),
        name::record("tilt", f64::value(45.0).as_record().unwrap().clone()),
        text::value("after"),
    ]);
    let paths: Vec<_> = list
        .as_list()
        .unwrap()
        .keys()
        .map(|position| vec![gid::Step::Element(position.clone())])
        .collect();
    let name_path: Vec<_> = paths[1]
        .iter()
        .cloned()
        .chain([gid::Step::Key(name::vocabulary::NAME)])
        .collect();
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(list),
        cells: Cells::new(),
    }));
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, name_path.clone()));
    runner.refresh_frame(1.0, VIEWPORT);
    let press = |key| KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        ..Default::default()
    };
    assert!(runner.keyboard_event(&press(NamedKey::ArrowRight), 1.0, VIEWPORT));
    let selected = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), paths[1]);
    assert_eq!(selected.edit().unwrap().selection_offsets(), (0, 0));
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    let selected = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), name_path);
    assert_eq!(selected.edit().unwrap().selection_offsets(), (4, 4));

    for _ in 0.."tilt".len() {
        assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    }
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        paths[0]
    );
    assert!(runner.keyboard_event(&press(NamedKey::ArrowRight), 1.0, VIEWPORT));
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        name_path
    );
    for _ in 0.."tilt".len() + 1 + "45".len() + 1 {
        assert!(runner.keyboard_event(&press(NamedKey::ArrowRight), 1.0, VIEWPORT));
    }
    assert_eq!(
        runner.editor.model.selection.as_ref().unwrap().path(),
        paths[2]
    );
    assert!(runner.keyboard_event(&press(NamedKey::ArrowLeft), 1.0, VIEWPORT));
    let selected = runner.editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), paths[1]);
    assert_eq!(selected.edit().unwrap().selection_offsets(), (2, 2));
}

#[test]
fn focus_changes_clear_selection_cancel_gestures_and_gate_keyboard_input() {
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    }));
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, vec![]));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.keyboard_event(&key(), 1.0, VIEWPORT));
    let document = runner.editor.model.doc.clone();
    runner.editor.pressed = true;
    runner.editor.pointer = Some(Point::new(20.0, 20.0));
    let cancelled = Rc::new(std::cell::Cell::new(false));
    runner.frame.dispatch.handler_mut().on_pointer_cancel({
        let cancelled = cancelled.clone();
        move |_, _| {
            cancelled.set(true);
            true
        }
    });
    assert!(runner.focus_changed(false, 1.0, VIEWPORT));
    assert!(cancelled.get());
    assert!(!runner.editor.pressed);
    assert!(runner.editor.pointer.is_none());
    assert!(!runner.focus_changed(false, 1.0, VIEWPORT));
    assert!(!runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert!(!runner.ime_event(&ImeEvent::Commit("ignored".into()), 1.0, VIEWPORT));
    assert!(Rc::ptr_eq(&document, &runner.editor.model.doc));
    assert!(runner.editor.model.selection.is_none());
    assert!(runner.focus_changed(true, 1.0, VIEWPORT));
    assert!(!runner.focus_changed(true, 1.0, VIEWPORT));
    assert!(runner.editor.model.selection.is_none());
    assert!(!runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert_eq!(
        text::read(runner.editor.model.doc.root.as_ref().unwrap()),
        Some("hellox")
    );
}

#[test]
fn only_a_focused_text_edit_offers_a_composition_area() {
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    }));
    runner.refresh_frame(1.0, VIEWPORT);
    assert_eq!(runner.frame.input_area, None);
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, vec![]));
    runner.refresh_frame(1.0, VIEWPORT);
    let area = runner
        .frame
        .input_area
        .expect("editing text offers a composition area");
    let viewport = Rect::from_origin_size(Point::ZERO, VIEWPORT);
    assert!(area.height() > 0.0 && area.intersect(viewport) == area);
    assert!(runner.focus_changed(false, 1.0, VIEWPORT));
    assert_eq!(runner.frame.input_area, None);
}

#[test]
fn a_picker_query_offers_a_composition_area() {
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: None,
        cells: Cells::new(),
    }));
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, vec![]));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.frame.input_area.is_some());
}

#[test]
fn focus_loss_ends_ime_composition_without_changing_committed_text() {
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    }));
    let root = runner.editor.model.workspace.document_root().clone();
    runner.editor.model.selection = Some(selection::Selection::edge(&root, vec![]));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.ime_event(
        &ImeEvent::Preedit("draft".into(), Some((5, 5))),
        1.0,
        VIEWPORT
    ));
    let composing = |runner: &EditorRunner| {
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .preedit_parts()
            .is_some()
    };
    assert!(composing(&runner));
    let document = runner.editor.model.doc.clone();
    assert!(runner.focus_changed(false, 1.0, VIEWPORT));
    assert!(runner.editor.model.selection.is_none());
    assert!(Rc::ptr_eq(&document, &runner.editor.model.doc));
    assert_eq!(text::read(document.root.as_ref().unwrap()), Some("hello"));
    assert!(runner.focus_changed(true, 1.0, VIEWPORT));
    assert!(runner.editor.model.selection.is_none());
    assert!(!runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert_eq!(
        text::read(runner.editor.model.doc.root.as_ref().unwrap()),
        Some("hello")
    );
}

#[test]
fn input_runs_the_previous_handler_then_builds_the_successor_without_painting() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert_eq!(
        log.take(),
        [
            ("project", 0.0),
            ("hover", 0.0),
            ("bind", 0.0),
            ("key", 0.0),
            ("project", 1.0),
            ("hover", 1.0),
            ("bind", 1.0),
        ]
    );
    assert!(runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert_eq!(
        log.take(),
        [
            ("key", 1.0),
            ("project", 2.0),
            ("hover", 2.0),
            ("bind", 2.0)
        ]
    );
    puri::frame::render(
        runner.prepare_paint(1.0, VIEWPORT).renders,
        &mut puri::draw::DrawList::default(),
    );
    assert_eq!(log.take(), [("paint", 2.0)]);
}

#[test]
fn no_op_dispatch_declines_without_building_a_frame() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    assert!(!runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert!(!runner.ime_event(&ImeEvent::Commit("x".into()), 1.0, VIEWPORT));
    assert!(log.borrow().is_empty());
    assert!(runner.frame.pending_paint.is_none());
    assert!(runner.frame.dispatch.landmarks.is_empty());
    assert!(runner.frame.dispatch.view_regions.is_empty());
    assert!(runner.frame.dispatch.pointer_root.is_none());
}

#[test]
fn stationary_modifier_changes_dispatch_with_the_installed_hover_without_focus() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.editor.focused = false;
    runner.refresh_frame(1.0, VIEWPORT);
    log.borrow_mut().clear();
    let hover = crate::frame::Hovered::Blocked;
    let root = runner.editor.model.workspace.document_root().clone();
    runner.frame.hover = Some(hover.clone());
    runner.frame.dispatch.pointer_root = Some(root.clone());
    runner.frame.dispatch.handler_mut().on({
        let log = log.clone();
        move |editor, event, input| {
            if let Event::ModifiersChanged(modifiers) = event {
                assert_eq!(editor.modifiers, *modifiers);
                assert_eq!(input.hovered(), Some(&hover));
                assert_eq!(input.root.as_ref(), Some(&root));
                log.borrow_mut().push(("modifiers", 0.0));
                Rc::make_mut(&mut editor.model.doc).root = Some(f64::value(5.0));
                puri::handler::EventOutcome::accept()
            } else {
                puri::handler::EventOutcome::decline(event)
            }
        }
    });
    runner.modifiers_changed(Modifiers::META, 1.0, VIEWPORT);
    assert!(!runner.editor.focused);
    assert_eq!(
        log.take(),
        [
            ("modifiers", 0.0),
            ("project", 5.0),
            ("hover", 5.0),
            ("bind", 5.0)
        ]
    );
}

#[test]
fn every_input_receives_the_installed_dispatch_context() {
    use puri::handler::EventOutcome;
    for kind in [
        "down", "up", "cancel", "move", "scroll", "gesture", "key", "ime",
    ] {
        let log = Log::default();
        let mut runner = instrumented_runner(&log);
        runner.refresh_frame(1.0, VIEWPORT);
        let root = runner.editor.model.workspace.document_root().clone();
        let hover = crate::frame::Hovered::Blocked;
        let mut pass = crate::display::widget::HoverPass::<(), _>::new(&Default::default());
        pass.in_view(root.clone(), |pass| {
            pass.visit(|output| {
                output.claim(crate::display::widget::frame::Probe::exact(
                    puri::Placement::root(Rect::new(0.0, 0.0, 100.0, 100.0)),
                    hover.clone(),
                ));
            })
        });
        runner.frame.dispatch.hover_geometry =
            pass.finish().bind(Default::default()).hover_geometry;
        runner.frame.hover = Some(hover.clone());
        runner.frame.dispatch.pointer_root = Some(root.clone());
        let landmarks = runner.frame.dispatch.landmarks.clone();
        let regions = runner.frame.dispatch.view_regions.clone();
        let calls = Rc::new(std::cell::Cell::new(0));
        *runner.frame.dispatch.handler_mut() = puri::handler::Handler::from_function({
            let calls = calls.clone();
            move |_, event, input: &mut crate::frame::DispatchContext<Editor>| {
                if matches!(event, Event::HoverChanged) {
                    return EventOutcome::decline(event);
                }
                assert_eq!(input.hovered(), Some(&hover), "{kind}");
                assert_eq!(input.root.as_ref(), Some(&root), "{kind}");
                assert!(Rc::ptr_eq(&input.landmarks, &landmarks), "{kind}");
                assert!(Rc::ptr_eq(&input.view_regions, &regions), "{kind}");
                calls.set(calls.get() + 1);
                EventOutcome::accept()
            }
        });
        let button = PointerButtonEvent {
            button: Some(puri::handler::PointerButton::Primary),
            pointer: crate::display::test_support::mouse(),
            state: PointerState {
                position: (20.0, 20.0).into(),
                ..Default::default()
            },
        };
        match kind {
            "key" => {
                runner.keyboard_event(&key(), 1.0, VIEWPORT);
            }
            "ime" => {
                runner.ime_event(&ImeEvent::Commit("x".into()), 1.0, VIEWPORT);
            }
            _ => {
                let event = match kind {
                    "down" => PointerEvent::Down(button),
                    "up" => PointerEvent::Up(button),
                    "cancel" => PointerEvent::Cancel(button.pointer),
                    "move" => PointerEvent::Move(PointerUpdate {
                        pointer: button.pointer,
                        current: button.state,
                        coalesced: vec![],
                        predicted: vec![],
                    }),
                    "gesture" => PointerEvent::Gesture(ui_events::pointer::PointerGestureEvent {
                        pointer: button.pointer,
                        state: button.state,
                        gesture: ui_events::pointer::PointerGesture::Pinch(0.1),
                    }),
                    "scroll" => PointerEvent::Scroll(PointerScrollEvent {
                        pointer: button.pointer,
                        state: button.state,
                        delta: ScrollDelta::LineDelta(0.0, -1.0),
                    }),
                    _ => unreachable!(),
                };
                runner.pointer_event(&event, 1.0, VIEWPORT);
                runner.flush_pending_continuous();
            }
        }
        assert_eq!(calls.get(), 1, "{kind}");
    }
}

#[test]
fn pinch_packets_keep_their_context_and_build_one_successor() {
    use puri::handler::{EventOutcome, PointerGesture};
    use winit::event::{DeviceId, TouchPhase, WindowEvent};
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.refresh_frame(2.0, VIEWPORT);
    log.take();
    let device_id = DeviceId::dummy();
    let motion = WindowEvent::CursorMoved {
        device_id,
        position: (25.0, 20.0).into(),
    };
    // Winit refreshes the pointer before every macOS magnification packet.
    runner
        .frame
        .dispatch
        .handler_mut()
        .on_gesture_batch(|editor, events| {
            assert_eq!(events.len(), 2);
            let mut zoom = 1.0;
            for event in events.iter() {
                assert_eq!(event.state.position.x, 25.0);
                let PointerGesture::Pinch(delta) = event.gesture else {
                    panic!("expected pinch")
                };
                zoom *= 1.0 + f64::from(delta);
            }
            Rc::make_mut(&mut editor.model.doc).root = Some(f64::value(zoom));
            EventOutcome::accept()
        });
    for _ in 0..2 {
        let pinch = WindowEvent::PinchGesture {
            device_id,
            delta: 0.25,
            phase: TouchPhase::Moved,
        };
        for event in [&motion, &pinch] {
            assert!(!runner.flush_before_window_event(event));
            let Some(crate::WindowEventTranslation::Pointer(pointer)) =
                crate::translate_window_event(&mut runner.editor.reducer, 2.0, event)
            else {
                panic!("expected pointer event")
            };
            runner.pointer_event(&pointer, 2.0, VIEWPORT);
        }
    }
    assert!(log.borrow().is_empty());
    let end = WindowEvent::PinchGesture {
        device_id,
        delta: 0.0,
        phase: TouchPhase::Ended,
    };
    assert!(runner.flush_before_window_event(&end));
    assert!(runner.pending_gesture.is_none());
    let log = log.take();
    assert_eq!(
        log.iter().filter(|(event, _)| *event == "project").count(),
        1
    );
    assert_eq!(
        runner.editor.model.doc.root.as_ref().and_then(f64::read),
        Some(1.5625)
    );
}

#[test]
fn scrolling_delivers_one_batch_and_builds_one_successor() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.refresh_frame(1.0, VIEWPORT);
    log.take();
    let events = [10.0, -10.0].map(|y| PointerScrollEvent {
        pointer: crate::display::test_support::mouse(),
        state: PointerState {
            position: (20.0, 20.0).into(),
            ..Default::default()
        },
        delta: ScrollDelta::PixelDelta((0.0, y).into()),
    });
    *runner.frame.dispatch.handler_mut() = puri::handler::Handler::new();
    runner
        .frame
        .dispatch
        .handler_mut()
        .on_scroll_batch(|editor, events| {
            assert_eq!(events.len(), 2);
            puri::scroll::each(events, |event| {
                let (next, outcome) = crate::display::widget::scroll::offset(
                    editor.model.workspace.document.scroll,
                    event,
                    1.0,
                    VIEWPORT,
                    kurbo::Vec2::new(0.0, 100.0),
                );
                editor.model.workspace.document.scroll = next;
                outcome
            })
        });
    for event in events {
        runner.pointer_event(&PointerEvent::Scroll(event), 1.0, VIEWPORT);
    }
    assert!(log.borrow().is_empty());
    assert!(runner.flush_pending_continuous());
    assert_eq!(runner.editor.model.workspace.document.scroll.y, 10.0);
    assert_eq!(
        log.take(),
        [("project", 0.0), ("hover", 0.0), ("bind", 0.0)]
    );
}

#[test]
fn replacing_a_document_drops_the_old_dispatch_before_building_its_successor() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.refresh_frame(1.0, VIEWPORT);
    log.borrow_mut().clear();
    let pointer = crate::display::test_support::mouse();
    let current = PointerState {
        position: (20.0, 20.0).into(),
        ..Default::default()
    };
    runner.pointer_event(
        &PointerEvent::Move(PointerUpdate {
            pointer,
            current: current.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        }),
        1.0,
        VIEWPORT,
    );
    runner.pointer_event(
        &PointerEvent::Scroll(PointerScrollEvent {
            pointer,
            state: current,
            delta: ScrollDelta::LineDelta(0.0, 1.0),
        }),
        1.0,
        VIEWPORT,
    );
    assert!(runner.pending_pointer.is_some());
    assert!(runner.pending_scroll.is_some());
    runner.frame.hover = Some(crate::frame::Hovered::Blocked);
    // Without a window, replacement leaves the no-op until setup supplies geometry.
    runner.adopt_model(
        Document {
            root: Some(f64::value(10.0)),
            cells: Cells::new(),
        },
        None,
        Default::default(),
    );
    assert!(!runner.flush_pending_continuous());
    assert!(runner.pending_pointer.is_none());
    assert!(runner.pending_scroll.is_none());
    assert!(runner.frame.hover.is_none());
    assert!(!runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert!(log.borrow().is_empty());
    assert!(runner.frame.pending_paint.is_none());
    assert!(runner.frame.dispatch.landmarks.is_empty());
    assert!(
        runner
            .frame
            .dispatch
            .hover_geometry
            .probe(Some(Point::new(20.0, 20.0)), None, 0.0)
            .is_none()
    );
    assert_eq!(runner.editor.model.doc.root, Some(f64::value(10.0)));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.keyboard_event(&key(), 1.0, VIEWPORT));
    assert_eq!(
        log.take(),
        [
            ("project", 10.0),
            ("hover", 10.0),
            ("bind", 10.0),
            ("key", 10.0),
            ("project", 11.0),
            ("hover", 11.0),
            ("bind", 11.0),
        ]
    );
}

#[test]
fn an_unhandled_key_retains_dispatch_and_pending_paint() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.refresh_frame(1.0, VIEWPORT);
    log.borrow_mut().clear();
    assert!(!runner.keyboard_event(&KeyboardEvent::default(), 1.0, VIEWPORT));
    assert!(log.borrow().is_empty());
    puri::frame::render(
        runner.prepare_paint(1.0, VIEWPORT).renders,
        &mut puri::draw::DrawList::default(),
    );
    assert_eq!(log.take(), [("paint", 0.0)]);
}

#[test]
fn scroll_settles_before_paired_motion_without_an_extra_frame() {
    let log = Log::default();
    let mut runner = instrumented_runner(&log);
    runner.refresh_frame(1.0, VIEWPORT);
    log.borrow_mut().clear();
    let state = PointerState {
        position: (20.0, 20.0).into(),
        ..Default::default()
    };
    let pointer = crate::display::test_support::mouse();
    let motion = PointerEvent::Move(PointerUpdate {
        pointer,
        current: state.clone(),
        coalesced: Vec::new(),
        predicted: Vec::new(),
    });
    runner.pointer_event(
        &PointerEvent::Scroll(PointerScrollEvent {
            pointer,
            delta: ScrollDelta::PixelDelta((0.0, -10.0).into()),
            state,
        }),
        1.0,
        VIEWPORT,
    );
    runner.pointer_event(&motion, 1.0, VIEWPORT);
    assert!(log.borrow().is_empty());
    assert!(runner.flush_pending_continuous());
    assert_eq!(
        log.take(),
        [
            ("scroll", 0.0),
            ("project", 1.0),
            ("hover", 1.0),
            ("bind", 1.0),
            ("move", 1.0)
        ]
    );
    runner.pointer_event(&motion, 1.0, VIEWPORT);
    assert!(runner.flush_pending_continuous());
    assert_eq!(
        log.take(),
        [
            ("move", 1.0),
            ("project", 1.0),
            ("hover", 1.0),
            ("bind", 1.0)
        ]
    );
}

#[test]
fn paint_flushes_motion_without_requesting_another_paint_but_release_does_request_one() {
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};

    let device_id = DeviceId::dummy();
    for (event, request_redraw) in [
        (WindowEvent::RedrawRequested, false),
        (
            WindowEvent::MouseInput {
                device_id,
                state: ElementState::Released,
                button: MouseButton::Left,
            },
            true,
        ),
    ] {
        let log = Log::default();
        let mut runner = instrumented_runner(&log);
        runner.refresh_frame(1.0, VIEWPORT);
        puri::frame::render(
            runner.prepare_paint(1.0, VIEWPORT).renders,
            &mut puri::draw::DrawList::default(),
        );
        assert!(!runner.frame_presented());
        log.take();

        let motion = WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(25.0, 20.0),
        };
        let Some(crate::WindowEventTranslation::Pointer(pointer)) =
            crate::translate_window_event(&mut runner.editor.reducer, 1.0, &motion)
        else {
            panic!("expected pointer motion");
        };
        runner.pointer_event(&pointer, 1.0, VIEWPORT);
        assert!(!runner.flush_before_window_event(&motion));
        assert!(runner.pending_pointer.is_some());
        assert!(log.borrow().is_empty());

        assert_eq!(runner.flush_before_window_event(&event), request_redraw);
        assert!(runner.pending_pointer.is_none());
        assert_eq!(
            log.take(),
            [
                ("move", 0.0),
                ("project", 0.0),
                ("hover", 0.0),
                ("bind", 0.0)
            ]
        );
        puri::frame::render(
            runner.prepare_paint(1.0, VIEWPORT).renders,
            &mut puri::draw::DrawList::default(),
        );
        assert_eq!(log.take(), [("paint", 0.0)]);
        assert!(!runner.frame_presented());
        assert!(!runner.flush_before_window_event(&event));
    }
}

#[test]
fn pointer_release_and_cancellation_update_the_computation_input() {
    let mut runner = instrumented_runner(&Log::default());
    runner.refresh_frame(1.0, VIEWPORT);
    let pressed = runner.editor.computations.runtime.memo({
        let pressed = runner.editor.computations.pointer_pressed.clone();
        move |read| Ok(*pressed.read(read))
    });
    let button = PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: crate::display::test_support::mouse(),
        state: PointerState::default(),
    };
    for release in [
        PointerEvent::Up(button.clone()),
        PointerEvent::Cancel(button.pointer),
    ] {
        runner.pointer_event(&PointerEvent::Down(button.clone()), 1.0, VIEWPORT);
        assert!(*runner.editor.computations.runtime.read(&pressed).unwrap());
        runner.pointer_event(&release, 1.0, VIEWPORT);
        assert!(!*runner.editor.computations.runtime.read(&pressed).unwrap());
    }
}

#[test]
fn touch_without_prior_motion_selects_the_contact_and_text_input_uses_its_handler() {
    let mut runner = EditorRunner::new(crate::test_editor(Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    }));
    let contact = PointerButtonEvent {
        button: None,
        pointer: PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: PointerType::Touch,
        },
        state: PointerState {
            position: (25.0, 20.0).into(),
            ..Default::default()
        },
    };
    assert!(runner.pointer_event(&PointerEvent::Down(contact.clone()), 1.0, VIEWPORT));
    assert!(runner.editor.pressed);
    assert!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .is_some_and(|selection| selection.path().is_empty())
    );
    assert!(runner.pointer_event(&PointerEvent::Up(contact), 1.0, VIEWPORT));
    assert!(!runner.editor.pressed);
    assert!(runner.ime_event(&ImeEvent::Commit("!".into()), 1.0, VIEWPORT));
    let value = text::read(runner.editor.model.doc.root.as_ref().unwrap()).unwrap();
    assert_eq!(value.len(), 6);
    assert!(value.contains('!'));
}
