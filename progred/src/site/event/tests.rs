use super::*;
use crate::display::test_support::mouse;

#[test]
fn grap_navigation_events_express_direction_without_a_physical_key() {
    use puri::handler::NavigationDirection::*;
    for (direction, spelling) in [(Left, "left"), (Right, "right"), (Up, "up"), (Down, "down")] {
        assert_eq!(
            navigation_value(direction),
            Value::record([
                (EVENT_KIND, Value::Cell(NAVIGATE)),
                (CONTENT, grap::text::value(spelling)),
            ])
        );
    }
}

#[test]
fn grap_pinch_delta_is_dimensionless_and_position_is_local() {
    let event = PointerGestureEvent {
        pointer: mouse(),
        state: PointerState {
            position: (25.0, 40.0).into(),
            ..Default::default()
        },
        gesture: PointerGesture::Pinch(0.25),
    };
    let placement = Placement::root(kurbo::Rect::new(10.0, 20.0, 110.0, 120.0));
    for scale in [1.0, 2.0] {
        let value = gesture_value(placement, scale, &event, CommandModifier::Meta);
        let fields = value.as_record().unwrap();
        assert_eq!(fields.get(&EVENT_KIND), Some(&Value::Cell(PINCH)));
        assert_eq!(fields.get(&DELTA).and_then(grap::f64::read), Some(0.25));
        assert_eq!(fields.get(&X).and_then(grap::f64::read), Some(15.0));
    }
}

#[test]
fn grap_motion_preserves_the_batch_in_local_coordinates() {
    let placement = Placement::root(kurbo::Rect::new(10.0, 20.0, 110.0, 120.0));
    let sample = |x: f64| {
        let mut state = PointerState::default();
        state.position.x = x;
        state.position.y = 30.0;
        state.buttons.insert(PointerButton::Primary);
        state
    };
    let mut event = PointerUpdate {
        pointer: mouse(),
        current: sample(40.0),
        coalesced: vec![sample(20.0), sample(30.0)],
        predicted: vec![sample(50.0)],
    };
    for (pointer_type, kind) in [
        (PointerType::Mouse, POINTER_MOVE),
        (PointerType::Touch, TOUCH_MOVE),
    ] {
        event.pointer.pointer_type = pointer_type;
        let value = pointer_move_value(placement, 2.0, &event, CommandModifier::Control);
        let fields = value.as_record().unwrap();
        assert_eq!(fields.get(&EVENT_KIND), Some(&Value::Cell(kind)));
        assert_eq!(fields.get(&X), Some(&grap::f64::value(30.0)));
        let earlier = fields.get(&COALESCED).unwrap();
        let expected = [10.0, 20.0].map(|x| {
            Value::record(
                pointer_fields(placement, 2.0, &sample(x + 10.0), CommandModifier::Control)
                    .into_iter()
                    .chain(
                        (pointer_type == PointerType::Mouse)
                            .then(|| (BUTTON, Value::Cell(PRIMARY))),
                    ),
            )
        });
        assert_eq!(earlier, &Value::list(expected));
    }
}
