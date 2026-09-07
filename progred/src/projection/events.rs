//! Bind projection handlers to Puri input and encode Grap event values.

use crate::frame::Hovered;
use crate::hover::Hover;
use crate::placed::{Placed, before};
use gid::{CellId, Path, Value};
use measured::Measured;
use progred_libraries::{f64 as f64_convention, layout as layout_data, text};
use puri::handler::{HasHandler, ImeEvent, ScrollOutcome};
use puri::interact::is_primary_contact;
use puri::{Canvas, Placement, Point};
use std::rc::Rc;
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyState, KeyboardEvent};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerScrollEvent, PointerType, PointerUpdate,
};

pub(super) fn realize_click<C: 'static, Cv: Canvas + 'static>(
    handler: progred_display::ActionHandler<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            is_primary_contact(event)
                && !crate::modifiers::pick(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && handler(world)
        });
    })
}

pub(super) fn realize_activate<C: 'static, Cv: Canvas + 'static>(
    target: Hover,
    handler: progred_display::ActionHandler<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, _| {
        p.activate(Hovered::Tree(target), move |world| handler(world));
    })
}

pub(super) fn realize_event_with<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    function: Value,
    apply: Rc<dyn Fn(&mut C, Path, Value, Value) -> bool>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_down(move |world, event| {
                placement.contains(Point::new(event.state.position.x, event.state.position.y))
                    && apply(
                        world,
                        path.clone(),
                        function.clone(),
                        pointer_button_value(
                            layout_data::vocabulary::POINTER_DOWN,
                            layout_data::vocabulary::TOUCH_START,
                            placement,
                            scale,
                            event,
                        ),
                    )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_cancel(move |world, event| {
                apply(
                    world,
                    path.clone(),
                    function.clone(),
                    pointer_cancel_value(event),
                )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_move(move |world, event| {
                apply(
                    world,
                    path.clone(),
                    function.clone(),
                    pointer_move_value(placement, scale, event),
                )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_up(move |world, event| {
                apply(
                    world,
                    path.clone(),
                    function.clone(),
                    pointer_button_value(
                        layout_data::vocabulary::POINTER_UP,
                        layout_data::vocabulary::TOUCH_END,
                        placement,
                        scale,
                        event,
                    ),
                )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_scroll(move |world, event| {
                if placement.contains(Point::new(event.state.position.x, event.state.position.y))
                    && apply(
                        world,
                        path.clone(),
                        function.clone(),
                        scroll_value(placement, scale, event),
                    )
                {
                    ScrollOutcome::consume(event)
                } else {
                    ScrollOutcome::pass(event)
                }
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_key(move |world, event| {
                apply(world, path.clone(), function.clone(), key_value(event))
            });
        }
        {
            p.handler().on_ime(move |world, event| {
                apply(world, path.clone(), function.clone(), ime_value(event))
            });
        }
    })
}

pub(super) fn realize_scrub<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    target: Hover,
    handler: progred_display::ScrubHandler,
    start: Rc<dyn Fn(&mut C, Path, progred_display::ScrubHandler, Point, f64) -> bool>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.pick_with(Hovered::Tree(target), move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            placement.contains(point) && start(world, path.clone(), handler.clone(), point, scale)
        });
    })
}

pub(super) fn realize_state_drag<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    target: Hover,
    on_press: progred_display::ActionHandler<C>,
    handler: progred_display::StateDragHandler,
    start: Rc<dyn Fn(&mut C, Path, progred_display::StateDragHandler, Point, f64)>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.activate_with(Hovered::Tree(target), move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            if placement.contains(point) && on_press(world) {
                start(world, path.clone(), handler.clone(), point, scale);
                true
            } else {
                false
            }
        });
    })
}

pub(super) fn realize_state_scroll<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    handler: progred_display::StateScrollHandler,
    update_state: Rc<dyn Fn(&mut C, Path, Value) -> bool>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_scroll(move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            if !placement.contains(point) {
                return ScrollOutcome::pass(event);
            }
            let (delta_x, delta_y, units_x, units_y) = match event.delta {
                ScrollDelta::PageDelta(x, y) => (
                    f64::from(x),
                    f64::from(y),
                    placement.rect.width() / scale,
                    placement.rect.height() / scale,
                ),
                ScrollDelta::LineDelta(x, y) => (f64::from(x), f64::from(y), 40.0, 40.0),
                ScrollDelta::PixelDelta(delta) => (delta.x, delta.y, 1.0 / scale, 1.0 / scale),
            };
            let (state, outcome) = handler(progred_display::StateScrollEvent {
                delta_x: delta_x * units_x,
                delta_y: delta_y * units_y,
            });
            if let Some(state) = state {
                update_state(world, path.clone(), state);
            }
            outcome.map(|remaining| match event.delta {
                ScrollDelta::PageDelta(..) => ScrollDelta::PageDelta(
                    (remaining.delta_x / units_x) as f32,
                    (remaining.delta_y / units_y) as f32,
                ),
                ScrollDelta::LineDelta(..) => ScrollDelta::LineDelta(
                    (remaining.delta_x / units_x) as f32,
                    (remaining.delta_y / units_y) as f32,
                ),
                ScrollDelta::PixelDelta(mut delta) => {
                    delta.x = remaining.delta_x / units_x;
                    delta.y = remaining.delta_y / units_y;
                    ScrollDelta::PixelDelta(delta)
                }
            })
        });
    })
}

pub(super) fn realize_point<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    handler: progred_display::PointHandler,
    start: Rc<dyn Fn(&mut C, Path, Placement, progred_display::PointHandler, Point) -> bool>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            is_primary_contact(event)
                && placement.contains(point)
                && start(world, path.clone(), placement, handler.clone(), point)
        });
    })
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
    state: &ui_events::pointer::PointerState,
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
                crate::modifiers::command(&state.modifiers)
                    .then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
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
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state);
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
    state: &ui_events::pointer::PointerState,
) -> Vec<(CellId, Value)> {
    let mut fields = pointer_fields(placement, scale, state);
    if !touch && state.buttons.contains(PointerButton::Primary) {
        fields.push((
            layout_data::vocabulary::BUTTON,
            Value::Cell(layout_data::vocabulary::PRIMARY),
        ));
    }
    fields
}

fn pointer_move_value(placement: Placement, scale: f64, event: &PointerUpdate) -> Value {
    let touch = event.pointer.pointer_type == PointerType::Touch;
    let mut fields = pointer_motion_fields(placement, scale, touch, &event.current);
    fields.push((
        layout_data::vocabulary::COALESCED,
        Value::list(
            event.coalesced.iter().map(|sample| {
                Value::record(pointer_motion_fields(placement, scale, touch, sample))
            }),
        ),
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

fn pointer_cancel_value(event: &ui_events::pointer::PointerInfo) -> Value {
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
    use ui_events::pointer::{PointerId, PointerInfo, PointerState};

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
            let value = pointer_move_value(placement, 2.0, &event);
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
                let mut fields = pointer_fields(placement, 2.0, &sample(x + 10.0));
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

fn scroll_value(placement: Placement, scale: f64, event: &PointerScrollEvent) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state);
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

fn key_value(event: &KeyboardEvent) -> Value {
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
                crate::modifiers::command(&event.modifiers)
                    .then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
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
