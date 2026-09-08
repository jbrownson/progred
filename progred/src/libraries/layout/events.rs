//! Grap's ordinary event adapter. Native widgets use Puri handlers directly.

use super as layout_data;
use crate::display::{Layout, widget};
use crate::libraries::{f64 as f64_convention, text};
use gid::{CellId, Value};
use puri::handler::{
    Event, EventOutcome, HasHandler, ImeEvent, KeyState, KeyboardEvent, Modifiers, PointerButton,
    PointerButtonEvent, PointerInfo, PointerScrollEvent, PointerState, PointerType, PointerUpdate,
    ScrollDelta, ScrollOutcome,
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
            let command = crate::modifiers::command;
            let scale = context.inputs.styles.scale;
            let function = function.clone();
            Box::new(move |output, placement| {
                output.handler().on(move |world, event, _| {
                    let point = match &event {
                        Event::PointerDown(event) => Some(event.state.position),
                        Event::Scroll(event) => Some(event.state.position),
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
                            Event::Scroll(event) => scroll_value(placement, scale, event, command),
                            Event::Key(event) => key_value(event, command),
                            Event::Ime(event) => ime_value(event),
                        };
                        crate::site::apply_event(
                            world,
                            root.clone(),
                            path.clone(),
                            function.clone(),
                            value,
                        )
                    };
                    match event {
                        Event::Scroll(scroll) => {
                            let outcome = if handled {
                                ScrollOutcome::consume(&scroll)
                            } else {
                                ScrollOutcome::pass(&scroll)
                            };
                            outcome.into_event(scroll)
                        }
                        other => EventOutcome::from_handled(other, handled),
                    }
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
    fields.push((
        layout_data::vocabulary::MODIFIERS,
        Value::list(
            [
                state
                    .modifiers
                    .shift()
                    .then_some(Value::Cell(layout_data::vocabulary::SHIFT)),
                command(&state.modifiers).then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
            ]
            .into_iter()
            .flatten(),
        ),
    ));
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
    fields.push((
        layout_data::vocabulary::MODIFIERS,
        Value::list(
            [
                event
                    .modifiers
                    .shift()
                    .then_some(Value::Cell(layout_data::vocabulary::SHIFT)),
                command(&event.modifiers).then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
            ]
            .into_iter()
            .flatten(),
        ),
    ));
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
