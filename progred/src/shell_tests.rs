use super::*;
use crate::frame::Hovered;
use crate::input::{
    PendingPointer, PendingScroll, continuous_input, pointer_position, window_pointer,
};
use crate::workspace;
use kurbo::{Point, Size};
use ui_events::ScrollDelta;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{
    PointerEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState, PointerType,
    PointerUpdate,
};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use winit::dpi::PhysicalPosition;
use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};
use winit::window::CursorIcon;

fn translate_pointer(reducer: &mut WindowEventReducer, event: WindowEvent) -> PointerEvent {
    match translate_window_event(reducer, 1.0, &event) {
        Some(WindowEventTranslation::Pointer(pointer)) => pointer,
        _ => panic!("expected pointer input"),
    }
}

#[test]
fn window_departure_preserves_pressed_motion_and_release_outside() {
    let mut reducer = WindowEventReducer::default();
    let device_id = DeviceId::dummy();
    assert!(matches!(
        translate_pointer(
            &mut reducer,
            WindowEvent::MouseInput {
                device_id,
                state: ElementState::Pressed,
                button: MouseButton::Left,
            }
        ),
        PointerEvent::Down(_)
    ));
    assert!(matches!(
        translate_pointer(&mut reducer, WindowEvent::CursorLeft { device_id }),
        PointerEvent::Leave(_)
    ));
    let PointerEvent::Move(motion) = translate_pointer(
        &mut reducer,
        WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(-40.0, 700.0),
        },
    ) else {
        panic!("expected motion")
    };
    assert!(puri::interact::is_primary_contact_move(&motion));
    assert_eq!(
        pointer_position(&PointerEvent::Move(motion)),
        Some(Point::new(-40.0, 700.0))
    );
    let PointerEvent::Up(release) = translate_pointer(
        &mut reducer,
        WindowEvent::MouseInput {
            device_id,
            state: ElementState::Released,
            button: MouseButton::Left,
        },
    ) else {
        panic!("expected release")
    };
    assert!(release.state.buttons.is_empty());
    assert_eq!(
        pointer_position(&PointerEvent::Up(release)),
        Some(Point::new(-40.0, 700.0))
    );
}

#[test]
fn focus_loss_cancels_and_forgets_pressed_mouse_state() {
    let mut reducer = WindowEventReducer::default();
    let device_id = DeviceId::dummy();
    translate_pointer(
        &mut reducer,
        WindowEvent::MouseInput {
            device_id,
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
    );
    assert!(matches!(
        translate_pointer(&mut reducer, WindowEvent::Focused(false)),
        PointerEvent::Cancel(_)
    ));
    let PointerEvent::Move(motion) = translate_pointer(
        &mut reducer,
        WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(10.0, 20.0),
        },
    ) else {
        panic!("expected motion")
    };
    assert!(!puri::interact::is_primary_contact_move(&motion));
}

#[test]
fn outside_drag_positions_do_not_become_hover_positions() {
    let size = Size::new(400.0, 300.0);
    assert_eq!(
        window_pointer(Point::new(10.0, 20.0), size),
        Some(Point::new(10.0, 20.0))
    );
    for point in [
        Point::new(-1.0, 20.0),
        Point::new(401.0, 20.0),
        Point::new(10.0, -1.0),
        Point::new(10.0, 301.0),
    ] {
        assert_eq!(window_pointer(point, size), None);
    }
}

fn pending(delta: ScrollDelta, x: f64) -> PendingScroll {
    let mut state = PointerState::default();
    state.position.x = x;
    PendingScroll {
        events: vec![PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            delta,
            state,
        }],
        scale: 2.0,
        viewport: Size::new(1800.0, 1280.0),
    }
}

#[test]
fn pending_scrolls_preserve_packets_in_order_including_mixed_units() {
    let mut accumulated = pending(
        ScrollDelta::PixelDelta(PhysicalPosition::new(2.0, 3.0)),
        10.0,
    );
    assert!(
        accumulated
            .merge(pending(
                ScrollDelta::PixelDelta(PhysicalPosition::new(5.0, 7.0)),
                20.0,
            ))
            .is_ok()
    );
    assert_eq!(
        accumulated
            .events
            .iter()
            .map(|event| (event.delta, event.state.position.x))
            .collect::<Vec<_>>(),
        [
            (
                ScrollDelta::PixelDelta(PhysicalPosition::new(2.0, 3.0)),
                10.0
            ),
            (
                ScrollDelta::PixelDelta(PhysicalPosition::new(5.0, 7.0)),
                20.0
            ),
        ]
    );
    assert!(
        accumulated
            .merge(pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0))
            .is_ok()
    );
    assert_eq!(
        accumulated.events.last().unwrap().delta,
        ScrollDelta::LineDelta(0.0, 1.0)
    );
    let mut resized = pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0);
    resized.viewport.width += 1.0;
    assert!(accumulated.merge(resized).is_err());
    let mut rescaled = pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0);
    rescaled.scale += 1.0;
    assert!(accumulated.merge(rescaled).is_err());
}

fn pointer(x: f64) -> PendingPointer {
    let scroll = pending(ScrollDelta::LineDelta(0.0, 0.0), x);
    let event = scroll.events.into_iter().next().unwrap();
    PendingPointer {
        event: PointerUpdate {
            pointer: event.pointer,
            current: event.state,
            coalesced: vec![],
            predicted: vec![],
        },
        start: Point::ZERO,
        scale: scroll.scale,
        viewport: scroll.viewport,
    }
}

#[test]
fn pointer_batches_preserve_observed_samples_and_replace_predictions() {
    let mut batch = pointer(2.0);
    batch.event.coalesced.push(pointer(1.0).event.current);
    batch.event.predicted.push(pointer(100.0).event.current);
    let mut next = pointer(4.0);
    next.event.coalesced.push(pointer(3.0).event.current);
    next.event.predicted.push(pointer(5.0).event.current);
    next.start = Point::new(2.0, 0.0);
    assert!(batch.merge(next).is_ok());
    assert_eq!(batch.start, Point::ZERO);
    assert_eq!(batch.event.current.position.x, 4.0);
    assert_eq!(
        puri::interact::pointer_samples(&batch.event)
            .map(|sample| sample.position.x)
            .collect::<Vec<_>>(),
        [1.0, 2.0, 3.0, 4.0],
    );
    assert_eq!(batch.event.predicted, vec![pointer(5.0).event.current]);
}

#[test]
fn pointer_batches_do_not_mix_contacts_buttons_or_coordinate_systems() {
    let mut other_contact = pointer(1.0);
    other_contact.event.pointer.pointer_id = PointerId::new(2);
    let mut pressed = pointer(1.0);
    pressed
        .event
        .current
        .buttons
        .insert(ui_events::pointer::PointerButton::Primary);
    let mut modified = pointer(1.0);
    modified.event.current.modifiers = Modifiers::SHIFT;
    let mut resized = pointer(1.0);
    resized.viewport.width += 1.0;
    let mut rescaled = pointer(1.0);
    rescaled.scale = 1.0;
    for next in [other_contact, pressed, modified, resized, rescaled] {
        let mut batch = pointer(0.0);
        let before = batch.event.clone();
        assert!(batch.merge(next).is_err());
        assert_eq!(batch.event, before);
    }
    let mut pressed = pointer(1.0);
    pressed
        .event
        .current
        .buttons
        .insert(ui_events::pointer::PointerButton::Primary);
    let mut next = pointer(2.0);
    next.event.current.buttons = pressed.event.current.buttons;
    assert!(pressed.merge(next).is_ok(), "pressed motion batches too");
}

#[test]
fn redraw_release_and_cancellation_flush_motion_before_dispatch() {
    let device_id = DeviceId::dummy();
    assert!(continuous_input(&WindowEvent::CursorMoved {
        device_id,
        position: PhysicalPosition::new(-20.0, 30.0),
    }));
    for event in [
        WindowEvent::RedrawRequested,
        WindowEvent::MouseInput {
            device_id,
            state: ElementState::Released,
            button: MouseButton::Left,
        },
        WindowEvent::Focused(false),
        WindowEvent::CursorLeft { device_id },
    ] {
        assert!(!continuous_input(&event));
    }
    for (phase, continuous) in [
        (winit::event::TouchPhase::Started, false),
        (winit::event::TouchPhase::Moved, true),
        (winit::event::TouchPhase::Ended, false),
        (winit::event::TouchPhase::Cancelled, false),
    ] {
        assert_eq!(
            continuous_input(&WindowEvent::Touch(winit::event::Touch {
                device_id,
                phase,
                location: PhysicalPosition::new(10.0, 20.0),
                force: None,
                id: 1,
            })),
            continuous
        );
    }
}

#[test]
fn divider_hover_uses_the_cursor_for_its_resize_axis() {
    assert_eq!(cursor_icon(None), CursorIcon::Default);
    assert_eq!(
        cursor_icon(Some(&Hovered::Divider(workspace::Divider::Columns(
            workspace::Side::Left,
        )))),
        CursorIcon::ColResize
    );
    assert_eq!(
        cursor_icon(Some(&Hovered::Divider(workspace::Divider::Panes {
            side: workspace::Side::Left,
            before: workspace::Root::document(),
            after: workspace::Root::document(),
        }))),
        CursorIcon::RowResize
    );
}
