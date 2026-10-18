//! The event a Grap handler receives: each toolkit event as a record whose
//! `event kind` says which, with positions local to the handler's placement.
//! The layout library's `on event` decides when a drawing's handler gets one;
//! this is what it gets, in the editor's own vocabulary.

use gid::{CellId, Value};
use puri::handler::{Event, ImeEvent};
use puri::keyboard::CommandModifier;
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyState, KeyboardEvent, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerGesture, PointerGestureEvent, PointerInfo,
    PointerScrollEvent, PointerState, PointerType, PointerUpdate,
};
use uig::Placement;
use vocabulary::*;

#[cfg(test)]
mod tests;

pub mod vocabulary {
    use gid::CellId;

    /// The handler call's argument.
    pub const EVENT: CellId = CellId::from_u128(0xbd232b5fb4450b52e088470776ed2a01);

    // Event kinds.
    pub const EVENT_KIND: CellId = CellId::from_u128(0x9cacae824e9661b167f3ed2b8e187ee0);
    pub const POINTER_DOWN: CellId = CellId::from_u128(0x67a9a626caff2f3568224eafcb338428);
    pub const POINTER_MOVE: CellId = CellId::from_u128(0xbf54bd5b9cd29a7c32fc496d6d59e4ca);
    pub const POINTER_UP: CellId = CellId::from_u128(0x5c2173cf26dfc305ebd89e3cf1d62890);
    pub const POINTER_CANCEL: CellId = CellId::from_u128(0x594eeff3b32941fea937818d89ca2002);
    pub const TOUCH_START: CellId = CellId::from_u128(0x87a2ba9cac6a4805b4e3baf9e9218dd5);
    pub const TOUCH_MOVE: CellId = CellId::from_u128(0x5359b767cb744fb881c8a92b95664c20);
    pub const TOUCH_END: CellId = CellId::from_u128(0x32b27cf0994b45208502d133d0da4eb2);
    pub const TOUCH_CANCEL: CellId = CellId::from_u128(0xf96ee39a16f748cc85b6ea8c99b46394);
    pub const SCROLL: CellId = CellId::from_u128(0x7b0de6ea9b052da4b40e5f97438d537c);
    pub const GESTURE: CellId = CellId::from_u128(0x6043e90ba2b0030ddb6a395929256aa3);
    pub const PINCH: CellId = CellId::from_u128(0xfb30e7915daec8c48748e26b55614cc1);
    pub const ROTATION: CellId = CellId::from_u128(0x77267c709bca4d455769bad3abf39e26);
    pub const KEY: CellId = CellId::from_u128(0xbabfda8d94c4a003ae22faf4a4a2fd01);
    pub const IME: CellId = CellId::from_u128(0x33b7ee93c08863b54d3106802a28d110);
    pub const HOVER_CHANGED: CellId = CellId::from_u128(0xc0776db8cfd8e86b29d854f629717d17);
    pub const NAVIGATE: CellId = CellId::from_u128(0x43699c7ed5d154c4c80e3dfb5353eef0);
    pub const MODIFIERS_CHANGED: CellId = CellId::from_u128(0x5310f0fe0e1d13eddb21c96b61dfb9ac);

    // Fields.
    pub const X: CellId = CellId::from_u128(0x415def0fa0a9ac40dfba5fca4d0f8876);
    pub const Y: CellId = CellId::from_u128(0x4e2dcde5b1ab1480a2f126176dd148c7);
    pub const COUNT: CellId = CellId::from_u128(0xc6b90219f3e63bbbc14b7ebefd1bd443);
    pub const SCALE: CellId = CellId::from_u128(0xa370331d2ba2325f05fe6d621b885bef);
    pub const MODIFIERS: CellId = CellId::from_u128(0x36dc2e12ccce7f3cd48713123170714a);
    pub const SHIFT: CellId = CellId::from_u128(0x3766462666e33f13096d6afd581de3be);
    pub const COMMAND: CellId = CellId::from_u128(0xfaf6451fbf89c75cf7b6a906f27590e2);
    pub const BUTTON: CellId = CellId::from_u128(0x0ec32df180178e9fbc958d3d061481d7);
    pub const PRIMARY: CellId = CellId::from_u128(0x09f73c1a02464b762ae34adc9ec17baa);
    pub const COALESCED: CellId = CellId::from_u128(0x5ac14439cfe0120df29ae87604adf300);
    pub const DELTA: CellId = CellId::from_u128(0x913ddf518aeb0dd706ec0cbee6a03164);
    pub const DELTA_X: CellId = CellId::from_u128(0x798ade16a1f9f7beeb7009c1e183b3c7);
    pub const DELTA_Y: CellId = CellId::from_u128(0x1336f899599217d2535819a51a4c981c);
    pub const CONTENT: CellId = CellId::from_u128(0x3e6b91d4a25f70c8815d29f6c4a30e7b);
    pub const EVENT_STATE: CellId = CellId::from_u128(0x15ab408f66b4286b33f35d95a651b20a);
    pub const DOWN: CellId = CellId::from_u128(0x96a441425ba7048c7fbb1722922e5ffb);
    pub const UP: CellId = CellId::from_u128(0xe47d794f06d1f6fc6166c401aeb17c82);
    pub const REPEAT: CellId = CellId::from_u128(0x5636109fec98530f178de15f65b07372);
    pub const IME_ENABLED: CellId = CellId::from_u128(0xb773d2a984a80a30dec3d0877ff05895);
    pub const IME_DISABLED: CellId = CellId::from_u128(0xa6122f0fea5b0bda9f9a20b44a3e12db);
    pub const IME_PREEDIT: CellId = CellId::from_u128(0xa8dd99527060e9d38b0637fbe842abe4);
    pub const IME_COMMIT: CellId = CellId::from_u128(0xff48152d0c492add92f179b2d719f745);
    pub const START: CellId = CellId::from_u128(0xc68a36ea81944cdbc8e7897ffda5a36c);
    pub const END: CellId = CellId::from_u128(0xabbfd273a978cb520a145eb65568f224);
}

/// `event` as the record a Grap handler drawn at `placement` receives, its
/// positions local to the placement; `command` is the platform's command
/// modifier. Scroll and gesture events arrive in batches, each one record.
pub(crate) fn value(
    event: &Event,
    placement: Placement,
    scale: f64,
    command: CommandModifier,
) -> Value {
    match event {
        Event::PointerDown(event) => {
            pointer_button_value(POINTER_DOWN, TOUCH_START, placement, scale, event, command)
        }
        Event::PointerMove(event) => pointer_move_value(placement, scale, event, command),
        Event::PointerUp(event) => {
            pointer_button_value(POINTER_UP, TOUCH_END, placement, scale, event, command)
        }
        Event::PointerCancel(event) => pointer_cancel_value(event),
        Event::Scroll(events) => scroll(placement, scale, events, command),
        Event::Gesture(events) => gesture(placement, scale, events, command),
        Event::Key(event) => key_value(event, command),
        Event::Navigate(direction) => navigation_value(*direction),
        Event::Ime(event) => ime_value(event),
        Event::HoverChanged => event_value(HOVER_CHANGED, []),
        Event::ModifiersChanged(modifiers) => {
            event_value(MODIFIERS_CHANGED, [modifier_field(modifiers, command)])
        }
    }
}

/// A batch of scroll events, as one record.
pub(crate) fn scroll(
    placement: Placement,
    scale: f64,
    events: &[PointerScrollEvent],
    command: CommandModifier,
) -> Value {
    event_value(
        SCROLL,
        [(
            CONTENT,
            Value::list(
                events
                    .iter()
                    .map(|event| scroll_value(placement, scale, event, command)),
            ),
        )],
    )
}

/// A batch of gesture events, as one record.
pub(crate) fn gesture(
    placement: Placement,
    scale: f64,
    events: &[PointerGestureEvent],
    command: CommandModifier,
) -> Value {
    event_value(
        GESTURE,
        [(
            CONTENT,
            Value::list(
                events
                    .iter()
                    .map(|event| gesture_value(placement, scale, event, command)),
            ),
        )],
    )
}

fn event_value(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record([(EVENT_KIND, Value::Cell(kind))].into_iter().chain(fields))
}

fn navigation_value(direction: puri::handler::NavigationDirection) -> Value {
    use puri::handler::NavigationDirection::*;
    event_value(
        NAVIGATE,
        [(
            CONTENT,
            grap::text::value(match direction {
                Left => "left",
                Right => "right",
                Up => "up",
                Down => "down",
            }),
        )],
    )
}

fn marker() -> Value {
    Value::record([])
}

fn modifier_field(modifiers: &Modifiers, command: CommandModifier) -> (CellId, Value) {
    (
        MODIFIERS,
        Value::list(
            [
                modifiers.shift().then_some(Value::Cell(SHIFT)),
                command.pressed(modifiers).then_some(Value::Cell(COMMAND)),
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
    command: CommandModifier,
) -> Vec<(CellId, Value)> {
    let mut fields = vec![
        (X, grap::f64::value(state.position.x - placement.rect.x0)),
        (Y, grap::f64::value(state.position.y - placement.rect.y0)),
        (COUNT, grap::f64::value(f64::from(state.count))),
        (SCALE, grap::f64::value(scale)),
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
    command: CommandModifier,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state, command);
    let touch = event.pointer.pointer_type == PointerType::Touch;
    if !touch && event.button == Some(PointerButton::Primary) {
        fields.push((BUTTON, Value::Cell(PRIMARY)));
    }
    event_value(if touch { touch_kind } else { pointer_kind }, fields)
}

fn pointer_motion_fields(
    placement: Placement,
    scale: f64,
    touch: bool,
    state: &PointerState,
    command: CommandModifier,
) -> Vec<(CellId, Value)> {
    let mut fields = pointer_fields(placement, scale, state, command);
    if !touch && state.buttons.contains(PointerButton::Primary) {
        fields.push((BUTTON, Value::Cell(PRIMARY)));
    }
    fields
}

fn pointer_move_value(
    placement: Placement,
    scale: f64,
    event: &PointerUpdate,
    command: CommandModifier,
) -> Value {
    let touch = event.pointer.pointer_type == PointerType::Touch;
    let mut fields = pointer_motion_fields(placement, scale, touch, &event.current, command);
    fields.push((
        COALESCED,
        Value::list(event.coalesced.iter().map(|sample| {
            Value::record(pointer_motion_fields(
                placement, scale, touch, sample, command,
            ))
        })),
    ));
    event_value(if touch { TOUCH_MOVE } else { POINTER_MOVE }, fields)
}

fn pointer_cancel_value(event: &PointerInfo) -> Value {
    event_value(
        if event.pointer_type == PointerType::Touch {
            TOUCH_CANCEL
        } else {
            POINTER_CANCEL
        },
        [],
    )
}

fn scroll_value(
    placement: Placement,
    scale: f64,
    event: &PointerScrollEvent,
    command: CommandModifier,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state, command);
    let (x, y) = match event.delta {
        ScrollDelta::PageDelta(x, y) | ScrollDelta::LineDelta(x, y) => (f64::from(x), f64::from(y)),
        ScrollDelta::PixelDelta(delta) => (delta.x, delta.y),
    };
    fields.extend([
        (DELTA_X, grap::f64::value(x)),
        (DELTA_Y, grap::f64::value(y)),
    ]);
    event_value(SCROLL, fields)
}

fn gesture_value(
    placement: Placement,
    scale: f64,
    event: &PointerGestureEvent,
    command: CommandModifier,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state, command);
    let (kind, delta) = match event.gesture {
        PointerGesture::Pinch(delta) => (PINCH, delta),
        PointerGesture::Rotate(delta) => (ROTATION, delta),
    };
    fields.push((DELTA, grap::f64::value(f64::from(delta))));
    event_value(kind, fields)
}

fn key_value(event: &KeyboardEvent, command: CommandModifier) -> Value {
    let mut fields = vec![
        (
            EVENT_STATE,
            Value::Cell(match event.state {
                KeyState::Down => DOWN,
                KeyState::Up => UP,
            }),
        ),
        (CONTENT, grap::text::value(event.key.to_string())),
    ];
    if event.repeat {
        fields.push((REPEAT, marker()));
    }
    fields.push(modifier_field(&event.modifiers, command));
    event_value(KEY, fields)
}

fn ime_value(event: &ImeEvent) -> Value {
    let mut fields = Vec::new();
    let state = match event {
        ImeEvent::Enabled => IME_ENABLED,
        ImeEvent::Disabled => IME_DISABLED,
        ImeEvent::Preedit(content, cursor) => {
            fields.push((CONTENT, grap::text::value(content)));
            if let Some((start, end)) = cursor {
                fields.extend([
                    (START, grap::f64::value(*start as f64)),
                    (END, grap::f64::value(*end as f64)),
                ]);
            }
            IME_PREEDIT
        }
        ImeEvent::Commit(content) => {
            fields.push((CONTENT, grap::text::value(content)));
            IME_COMMIT
        }
    };
    fields.push((EVENT_STATE, Value::Cell(state)));
    event_value(IME, fields)
}
