//! Grap's ordinary event adapter. Native widgets use Puri handlers directly.

use super as layout_data;
use crate::display::{Layout, widget};
use crate::libraries::{f64 as f64_convention, text};
use gid::{CellId, Value};
use puri::handler::{
    Event, EventOutcome, HasHandler, ImeEvent, KeyState, KeyboardEvent, Modifiers, PointerButton,
    PointerButtonEvent, PointerGesture, PointerGestureEvent, PointerInfo, PointerScrollEvent,
    PointerState, PointerType, PointerUpdate, ScrollDelta,
};
use puri::{Placement, Point};
use std::rc::Rc;

pub fn on_event(
    child: Layout<crate::Editor, crate::frame::Hovered>,
    function: Value,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    widget::before(
        child,
        Rc::new(move |context| {
            let root = context.inputs.view.clone();
            let path = context.path.to_vec();
            let edits = context.inputs.edits.clone();
            let command = context.inputs.command_modifier.predicate();
            let scale = context.inputs.styles.scale;
            let function = function.clone();
            Box::new(move |output, placement| {
                output.handler().on(move |world, event, _| {
                    if let Event::Scroll(events) = event {
                        return puri::scroll::filter(
                            events,
                            |event| {
                                placement.contains(Point::new(
                                    event.state.position.x,
                                    event.state.position.y,
                                ))
                            },
                            |events| {
                                let value = event_value(
                                    layout_data::vocabulary::SCROLL,
                                    [(
                                        layout_data::vocabulary::CONTENT,
                                        Value::list(events.iter().map(|event| {
                                            scroll_value(placement, scale, event, command)
                                        })),
                                    )],
                                );
                                let handled = edits.open(crate::editing::Access::new(world)).grap(
                                    root.clone(),
                                    path.clone(),
                                    function.clone(),
                                    value,
                                );
                                EventOutcome::from_handled(Event::Scroll(events), handled)
                            },
                        );
                    }
                    if let Event::Gesture(events) = event {
                        return puri::gesture::filter(
                            events,
                            |event| {
                                placement.contains(Point::new(
                                    event.state.position.x,
                                    event.state.position.y,
                                ))
                            },
                            |events| {
                                let value = event_value(
                                    layout_data::vocabulary::GESTURE,
                                    [(
                                        layout_data::vocabulary::CONTENT,
                                        Value::list(events.iter().map(|event| {
                                            gesture_value(placement, scale, event, command)
                                        })),
                                    )],
                                );
                                let handled = edits.open(crate::editing::Access::new(world)).grap(
                                    root.clone(),
                                    path.clone(),
                                    function.clone(),
                                    value,
                                );
                                EventOutcome::from_handled(Event::Gesture(events), handled)
                            },
                        );
                    }
                    let point = match &event {
                        Event::PointerDown(event) => Some(event.state.position),
                        _ => None,
                    };
                    let handled = if point
                        .is_some_and(|point| !placement.contains(Point::new(point.x, point.y)))
                    {
                        false
                    } else {
                        let value = match &event {
                            Event::PointerDown(event) => pointer_button_value(
                                layout_data::vocabulary::POINTER_DOWN,
                                layout_data::vocabulary::TOUCH_START,
                                placement,
                                scale,
                                event,
                                command,
                            ),
                            Event::PointerMove(event) => {
                                pointer_move_value(placement, scale, event, command)
                            }
                            Event::PointerUp(event) => pointer_button_value(
                                layout_data::vocabulary::POINTER_UP,
                                layout_data::vocabulary::TOUCH_END,
                                placement,
                                scale,
                                event,
                                command,
                            ),
                            Event::PointerCancel(event) => pointer_cancel_value(event),
                            Event::Scroll(_) | Event::Gesture(_) => unreachable!(),
                            Event::Key(event) => key_value(event, command),
                            Event::Ime(event) => ime_value(event),
                            Event::HoverChanged => {
                                event_value(layout_data::vocabulary::HOVER_CHANGED, [])
                            }
                            Event::ModifiersChanged(modifiers) => event_value(
                                layout_data::vocabulary::MODIFIERS_CHANGED,
                                [modifier_field(modifiers, command)],
                            ),
                        };
                        edits.open(crate::editing::Access::new(world)).grap(
                            root.clone(),
                            path.clone(),
                            function.clone(),
                            value,
                        )
                    };
                    EventOutcome::from_handled(event, handled)
                });
            })
        }),
    )
}

fn event_value(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(
        [(layout_data::vocabulary::EVENT_KIND, Value::Cell(kind))]
            .into_iter()
            .chain(fields),
    )
}

fn marker() -> Value {
    Value::record([])
}

fn modifier_field(modifiers: &Modifiers, command: fn(&Modifiers) -> bool) -> (CellId, Value) {
    (
        layout_data::vocabulary::MODIFIERS,
        Value::list(
            [
                modifiers
                    .shift()
                    .then_some(Value::Cell(layout_data::vocabulary::SHIFT)),
                command(modifiers).then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
            ]
            .into_iter()
            .flatten(),
        ),
    )
}

fn pointer_fields(
    placement: Placement,
    scale: f64,
    state: &PointerState,
    command: fn(&Modifiers) -> bool,
) -> Vec<(CellId, Value)> {
    let mut fields = vec![
        (
            layout_data::vocabulary::X,
            f64_convention::value(state.position.x - placement.rect.x0),
        ),
        (
            layout_data::vocabulary::Y,
            f64_convention::value(state.position.y - placement.rect.y0),
        ),
        (
            layout_data::vocabulary::COUNT,
            f64_convention::value(f64::from(state.count)),
        ),
        (layout_data::vocabulary::SCALE, f64_convention::value(scale)),
    ];
    fields.push(modifier_field(&state.modifiers, command));
    fields
}

fn pointer_button_value(
    pointer_kind: CellId,
    touch_kind: CellId,
    placement: Placement,
    scale: f64,
    event: &PointerButtonEvent,
    command: fn(&Modifiers) -> bool,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state, command);
    let touch = event.pointer.pointer_type == PointerType::Touch;
    if !touch && event.button == Some(PointerButton::Primary) {
        fields.push((
            layout_data::vocabulary::BUTTON,
            Value::Cell(layout_data::vocabulary::PRIMARY),
        ));
    }
    event_value(if touch { touch_kind } else { pointer_kind }, fields)
}

fn pointer_motion_fields(
    placement: Placement,
    scale: f64,
    touch: bool,
    state: &PointerState,
    command: fn(&Modifiers) -> bool,
) -> Vec<(CellId, Value)> {
    let mut fields = pointer_fields(placement, scale, state, command);
    if !touch && state.buttons.contains(PointerButton::Primary) {
        fields.push((
            layout_data::vocabulary::BUTTON,
            Value::Cell(layout_data::vocabulary::PRIMARY),
        ));
    }
    fields
}

fn pointer_move_value(
    placement: Placement,
    scale: f64,
    event: &PointerUpdate,
    command: fn(&Modifiers) -> bool,
) -> Value {
    let touch = event.pointer.pointer_type == PointerType::Touch;
    let mut fields = pointer_motion_fields(placement, scale, touch, &event.current, command);
    fields.push((
        layout_data::vocabulary::COALESCED,
        Value::list(event.coalesced.iter().map(|sample| {
            Value::record(pointer_motion_fields(
                placement, scale, touch, sample, command,
            ))
        })),
    ));
    event_value(
        if touch {
            layout_data::vocabulary::TOUCH_MOVE
        } else {
            layout_data::vocabulary::POINTER_MOVE
        },
        fields,
    )
}

fn pointer_cancel_value(event: &PointerInfo) -> Value {
    event_value(
        if event.pointer_type == PointerType::Touch {
            layout_data::vocabulary::TOUCH_CANCEL
        } else {
            layout_data::vocabulary::POINTER_CANCEL
        },
        [],
    )
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    use puri::handler::PointerId;

    #[test]
    fn grap_pinch_delta_is_dimensionless_and_position_is_local() {
        let event = PointerGestureEvent {
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (25.0, 40.0).into(),
                ..Default::default()
            },
            gesture: PointerGesture::Pinch(0.25),
        };
        let placement = Placement::root(puri::Rect::new(10.0, 20.0, 110.0, 120.0));
        for scale in [1.0, 2.0] {
            let value = gesture_value(placement, scale, &event, |m| m.meta());
            let fields = value.as_record().unwrap();
            assert_eq!(
                fields.get(&layout_data::vocabulary::EVENT_KIND),
                Some(&Value::Cell(layout_data::vocabulary::PINCH))
            );
            assert_eq!(
                fields
                    .get(&layout_data::vocabulary::DELTA)
                    .and_then(f64_convention::read),
                Some(0.25)
            );
            assert_eq!(
                fields
                    .get(&layout_data::vocabulary::X)
                    .and_then(f64_convention::read),
                Some(15.0)
            );
        }
    }

    #[test]
    fn grap_motion_preserves_the_batch_in_local_coordinates() {
        let placement = Placement::root(puri::Rect::new(10.0, 20.0, 110.0, 120.0));
        let sample = |x: f64| {
            let mut state = PointerState::default();
            state.position.x = x;
            state.position.y = 30.0;
            state.buttons.insert(PointerButton::Primary);
            state
        };
        let mut event = PointerUpdate {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            current: sample(40.0),
            coalesced: vec![sample(20.0), sample(30.0)],
            predicted: vec![sample(50.0)],
        };
        for (pointer_type, kind) in [
            (PointerType::Mouse, layout_data::vocabulary::POINTER_MOVE),
            (PointerType::Touch, layout_data::vocabulary::TOUCH_MOVE),
        ] {
            event.pointer.pointer_type = pointer_type;
            let value = pointer_move_value(placement, 2.0, &event, |m| m.ctrl());
            let fields = value.as_record().unwrap();
            assert_eq!(
                fields.get(&layout_data::vocabulary::EVENT_KIND),
                Some(&Value::Cell(kind))
            );
            assert_eq!(
                fields.get(&layout_data::vocabulary::X),
                Some(&f64_convention::value(30.0))
            );
            let earlier = fields.get(&layout_data::vocabulary::COALESCED).unwrap();
            let expected = [10.0, 20.0].map(|x| {
                let mut fields = pointer_fields(placement, 2.0, &sample(x + 10.0), |m| m.ctrl());
                if pointer_type == PointerType::Mouse {
                    fields.push((
                        layout_data::vocabulary::BUTTON,
                        Value::Cell(layout_data::vocabulary::PRIMARY),
                    ));
                }
                Value::record(fields)
            });
            assert_eq!(earlier, &Value::list(expected));
        }
    }
}

fn scroll_value(
    placement: Placement,
    scale: f64,
    event: &PointerScrollEvent,
    command: fn(&Modifiers) -> bool,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state, command);
    let (x, y) = match event.delta {
        ScrollDelta::PageDelta(x, y) | ScrollDelta::LineDelta(x, y) => (f64::from(x), f64::from(y)),
        ScrollDelta::PixelDelta(delta) => (delta.x, delta.y),
    };
    fields.extend([
        (layout_data::vocabulary::DELTA_X, f64_convention::value(x)),
        (layout_data::vocabulary::DELTA_Y, f64_convention::value(y)),
    ]);
    event_value(layout_data::vocabulary::SCROLL, fields)
}

fn gesture_value(
    placement: Placement,
    scale: f64,
    event: &PointerGestureEvent,
    command: fn(&Modifiers) -> bool,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state, command);
    let (kind, delta) = match event.gesture {
        PointerGesture::Pinch(delta) => (layout_data::vocabulary::PINCH, delta),
        PointerGesture::Rotate(delta) => (layout_data::vocabulary::ROTATION, delta),
    };
    fields.push((
        layout_data::vocabulary::DELTA,
        f64_convention::value(f64::from(delta)),
    ));
    event_value(kind, fields)
}

fn key_value(event: &KeyboardEvent, command: fn(&Modifiers) -> bool) -> Value {
    let mut fields = vec![
        (
            layout_data::vocabulary::EVENT_STATE,
            Value::Cell(match event.state {
                KeyState::Down => layout_data::vocabulary::DOWN,
                KeyState::Up => layout_data::vocabulary::UP,
            }),
        ),
        (
            layout_data::vocabulary::CONTENT,
            text::value(event.key.to_string()),
        ),
    ];
    if event.repeat {
        fields.push((layout_data::vocabulary::REPEAT, marker()));
    }
    fields.push(modifier_field(&event.modifiers, command));
    event_value(layout_data::vocabulary::KEY, fields)
}

fn ime_value(event: &ImeEvent) -> Value {
    let mut fields = Vec::new();
    let state = match event {
        ImeEvent::Enabled => layout_data::vocabulary::IME_ENABLED,
        ImeEvent::Disabled => layout_data::vocabulary::IME_DISABLED,
        ImeEvent::Preedit(content, cursor) => {
            fields.push((layout_data::vocabulary::CONTENT, text::value(content)));
            if let Some((start, end)) = cursor {
                fields.extend([
                    (
                        layout_data::vocabulary::START,
                        f64_convention::value(*start as f64),
                    ),
                    (
                        layout_data::vocabulary::END,
                        f64_convention::value(*end as f64),
                    ),
                ]);
            }
            layout_data::vocabulary::IME_PREEDIT
        }
        ImeEvent::Commit(content) => {
            fields.push((layout_data::vocabulary::CONTENT, text::value(content)));
            layout_data::vocabulary::IME_COMMIT
        }
    };
    fields.push((layout_data::vocabulary::EVENT_STATE, Value::Cell(state)));
    event_value(layout_data::vocabulary::IME, fields)
}
