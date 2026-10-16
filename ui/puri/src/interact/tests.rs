use super::*;
use crate::handler::Handler;
use kurbo::Rect;
use ui_events::pointer::{PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType};

fn down_at(x: f64, y: f64) -> PointerButtonEvent {
    let mut state = PointerState::default();
    state.position.x = x;
    state.position.y = y;
    PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state,
    }
}

fn down_at_count(x: f64, y: f64, count: u8) -> PointerButtonEvent {
    let mut event = down_at(x, y);
    event.state.count = count;
    event
}

fn touch_at(x: f64, y: f64) -> PointerButtonEvent {
    let mut event = down_at(x, y);
    event.button = None;
    event.pointer.pointer_type = PointerType::Touch;
    event
}

struct Frame {
    handler: Handler<u32>,
}

impl HasHandler<u32> for Frame {
    type Input = ();

    fn handler(&mut self) -> &mut Handler<u32> {
        &mut self.handler
    }
}

#[test]
fn pinch_is_bounded_by_both_widget_and_clip() {
    use crate::handler::{Event, PointerGestureEvent};
    let mut frame = Frame {
        handler: Handler::new(),
    };
    on_pinch(
        &mut frame,
        Placement::new(
            kurbo::Rect::new(0.0, 0.0, 20.0, 20.0),
            kurbo::Rect::new(0.0, 0.0, 10.0, 20.0),
        ),
        |count, delta| {
            assert_eq!(delta, 0.25);
            *count += 1;
            true
        },
    );
    let mut count = 0;
    for (x, delta, accepted) in [
        (5.0, 0.25, true),
        (15.0, 0.25, false),
        (25.0, 0.25, false),
        (5.0, f32::NAN, false),
    ] {
        let pointer = down_at(x, 5.0);
        let samples = [PointerGestureEvent {
            pointer: pointer.pointer,
            state: pointer.state,
            gesture: PointerGesture::Pinch(delta),
        }];
        assert_eq!(
            frame
                .handler
                .dispatch(
                    &mut count,
                    Event::Gesture(std::borrow::Cow::Borrowed(&samples)),
                    &mut ()
                )
                .handled(),
            accepted
        );
    }
    assert_eq!(count, 1);
}

/// A 10x10 clickable at the origin that sets the selected id to 7.
fn placed(viewport: Option<kurbo::Rect>) -> Handler<u32> {
    let mut frame = Frame {
        handler: Handler::new(),
    };
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    let placement = match viewport {
        Some(clip_rect) => Placement::new(rect, clip_rect),
        None => Placement::root(rect),
    };
    clickable(&mut frame, placement, |sel: &mut u32| *sel = 7);
    frame.handler
}

#[test]
fn press_inside_fires() {
    let frame = placed(None);
    let mut selected = 0;
    assert!(frame.dispatch_pointer_down(&mut selected, &down_at(5.0, 5.0)));
    assert_eq!(selected, 7);
}

#[test]
fn touch_contact_fires_without_inventing_a_button() {
    let event = touch_at(5.0, 5.0);
    assert_eq!(event.button, None);
    let frame = placed(None);
    let mut selected = 0;
    assert!(frame.dispatch_pointer_down(&mut selected, &event));
    assert_eq!(selected, 7);
}

#[test]
fn touch_move_is_an_active_contact_without_inventing_a_button() {
    let event = ui_events::pointer::PointerUpdate {
        pointer: touch_at(5.0, 5.0).pointer,
        current: PointerState::default(),
        coalesced: Vec::new(),
        predicted: Vec::new(),
    };
    assert!(event.current.buttons.is_empty());
    assert!(is_primary_contact_move(&event));
}

#[test]
fn press_outside_falls_through() {
    let frame = placed(None);
    let mut selected = 0;
    assert!(!frame.dispatch_pointer_down(&mut selected, &down_at(50.0, 50.0)));
    assert_eq!(selected, 0);
}

#[test]
fn press_in_the_clipped_part_falls_through() {
    let frame = placed(Some(kurbo::Rect::new(0.0, 0.0, 4.0, 10.0)));
    let mut selected = 0;
    assert!(!frame.dispatch_pointer_down(&mut selected, &down_at(7.0, 5.0)));
    assert_eq!(selected, 0);
    assert!(frame.dispatch_pointer_down(&mut selected, &down_at(3.0, 5.0)));
    assert_eq!(selected, 7);
}

#[test]
fn double_click_overrides_the_ordinary_click() {
    let mut frame = Frame {
        handler: Handler::new(),
    };
    let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
    clickable(&mut frame, placement, |value| *value += 1);
    double_clickable(&mut frame, placement, |value| *value += 10);

    let mut value = 0;
    assert!(
        frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at_count(5.0, 5.0, 2))
    );
    assert_eq!(value, 10);
}

#[test]
fn placed_predicate_can_subdivide_the_clip_rect() {
    let mut frame = Frame {
        handler: Handler::new(),
    };
    on_primary_pointer_down_where(
        &mut frame,
        Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
        |placement, event| event.state.position.x >= placement.rect.center().x,
        |value, _, _| {
            *value = 7;
            true
        },
    );

    let mut value = 0;
    assert!(
        !frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at(4.0, 5.0))
    );
    assert!(
        frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at(6.0, 5.0))
    );
    assert_eq!(value, 7);
}
