//! Event-scoped host capabilities used by the Grap stock line editor.
//! The functions expose Puri's text-editing transitions without
//! exposing document paths or making line editing a display variant.

use crate::SystemTextClipboard;
use crate::selection::payload;
use gid::{CellId, Value};
use grap::{Context, Environment, ForeignFunction, ForeignFunctions, Halt};
use parley::{FontContext, LayoutContext};
use progred_libraries::{absent, f64, layout, line_edit, text};
use puri::edit::{
    EditStyle, LineEditDescription, LineEditPointerDown, LineEditPresentation,
};
use puri::text::{TextCache, TextCtx};
use puri::handler::ImeEvent;
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers};
use vello::kurbo::Point;
use vello::peniko::{Brush, Color};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transition {
    PointerDown,
    PointerMove,
    PointerUp,
    Key,
    Ime,
}

pub(crate) fn drawing_functions() -> ForeignFunctions {
    let fonts = Rc::new(RefCell::new(FontContext::new()));
    let layouts = Rc::new(RefCell::new(LayoutContext::<Brush>::new()));
    let cache = Rc::new(RefCell::new(TextCache::default()));
    ForeignFunctions::default().register(
        line_edit::vocabulary::GEOMETRY,
        ForeignFunction::new(move |context, call, environment| {
            geometry(context, call, environment, &fonts, &layouts, &cache)
        }),
    )
}

fn geometry(
    context: &mut Context,
    call: &Value,
    environment: &Environment,
    fonts: &Rc<RefCell<FontContext>>,
    layouts: &Rc<RefCell<LayoutContext<Brush>>>,
    cache: &Rc<RefCell<TextCache>>,
) -> Result<Value, Halt> {
    macro_rules! argument {
        ($field:expr) => {{
            let Some(expression) = context.field(call, $field) else {
                return Ok(context.missing_argument($field));
            };
            context.eval(expression, environment)?
        }};
    }
    let content = argument!(layout::vocabulary::CONTENT);
    let prefix = argument!(line_edit::vocabulary::PREFIX);
    let suffix = argument!(line_edit::vocabulary::SUFFIX);
    let selected = argument!(layout::vocabulary::SELECTION);
    let (Some(content), Some(prefix), Some(suffix)) = (
        text::read(&content),
        text::read(&prefix),
        text::read(&suffix),
    ) else {
        return Ok(absent::value());
    };
    let focused = !absent::is_absent(&selected);
    if !focused {
        return Ok(Value::record([
            (
                layout::vocabulary::CONTENT,
                text::value(format!("{prefix}{content}{suffix}")),
            ),
            (layout::vocabulary::WIDTH, f64::value(0.0)),
            (layout::vocabulary::ASCENT, f64::value(0.0)),
            (layout::vocabulary::DESCENT, f64::value(0.0)),
            (
                line_edit::vocabulary::SELECTION_COMMANDS,
                Value::list([]),
            ),
            (
                line_edit::vocabulary::CURSOR_COMMANDS,
                Value::list([]),
            ),
        ]));
    }
    let state = payload::editor_line(&selected, content);
    let presentation = LineEditPresentation::new(
        14.0,
        Brush::from(Color::new([0.55, 0.33, 0.28, 1.0])),
    )
    .with_affixes(prefix, suffix);
    let style = EditStyle {
        selection: Brush::from(Color::new([0.0, 0.48, 1.0, 0.30])),
        cursor: Brush::from(Color::new([0.13, 0.14, 0.16, 1.0])),
    };
    let mut fonts = fonts.borrow_mut();
    let mut layouts = layouts.borrow_mut();
    let mut cache = cache.borrow_mut();
    let mut tcx = TextCtx {
        fonts: &mut *fonts,
        layouts: &mut *layouts,
        scale: 1.0,
        cache: &mut *cache,
    };
    let geometry = puri::edit::text_edit(
        LineEditDescription {
            state: &state,
            focused,
            presentation,
            style: &style,
            placeholder: None,
        },
        &mut tcx,
    )
    .geometry();
    let command = |rect: vello::kurbo::Rect, face| {
        layout::fill_rounded_rect(
            rect.x0,
            rect.y0,
            rect.width(),
            rect.height(),
            0.0,
            face,
        )
    };
    Ok(Value::record([
        (layout::vocabulary::CONTENT, text::value(geometry.text)),
        (
            layout::vocabulary::WIDTH,
            f64::value(geometry.metrics.width),
        ),
        (
            layout::vocabulary::ASCENT,
            f64::value(geometry.metrics.ascent),
        ),
        (
            layout::vocabulary::DESCENT,
            f64::value(geometry.metrics.descent),
        ),
        (
            line_edit::vocabulary::SELECTION_COMMANDS,
            Value::list(geometry.selection.into_iter().map(|rect| {
                command(rect, layout::vocabulary::ACCENT_WASH_FACE)
            })),
        ),
        (
            line_edit::vocabulary::CURSOR_COMMANDS,
            Value::list(geometry.cursor.into_iter().map(|rect| {
                command(rect, layout::vocabulary::INK_FACE)
            })),
        ),
    ]))
}

pub(crate) fn functions(
    fonts: Rc<RefCell<FontContext>>,
    layouts: Rc<RefCell<LayoutContext<Brush>>>,
) -> ForeignFunctions {
    [
        (line_edit::vocabulary::POINTER_DOWN, Transition::PointerDown),
        (line_edit::vocabulary::POINTER_MOVE, Transition::PointerMove),
        (line_edit::vocabulary::POINTER_UP, Transition::PointerUp),
        (line_edit::vocabulary::KEY, Transition::Key),
        (line_edit::vocabulary::IME, Transition::Ime),
    ]
    .into_iter()
    .fold(ForeignFunctions::default(), |functions, (cell, transition)| {
        let fonts = fonts.clone();
        let layouts = layouts.clone();
        functions.register(
            cell,
            ForeignFunction::new(move |context, call, environment| {
                apply(
                    transition,
                    context,
                    call,
                    environment,
                    &fonts,
                    &layouts,
                )
            }),
        )
    })
}

fn apply(
    transition: Transition,
    context: &mut Context,
    call: &Value,
    environment: &Environment,
    fonts: &Rc<RefCell<FontContext>>,
    layouts: &Rc<RefCell<LayoutContext<Brush>>>,
) -> Result<Value, Halt> {
    macro_rules! argument {
        ($field:expr) => {{
            let Some(expression) = context.field(call, $field) else {
                return Ok(context.missing_argument($field));
            };
            context.eval(expression, environment)?
        }};
    }

    let event = argument!(layout::vocabulary::EVENT);
    let content = argument!(layout::vocabulary::CONTENT);
    let prefix = argument!(line_edit::vocabulary::PREFIX);
    let suffix = argument!(line_edit::vocabulary::SUFFIX);
    let update = argument!(line_edit::vocabulary::UPDATE);
    let selected = argument!(layout::vocabulary::SELECTION);

    let Some(content) = text::read(&content) else {
        return Ok(absent::value());
    };
    let Some(prefix) = text::read(&prefix) else {
        return Ok(absent::value());
    };
    let Some(suffix) = text::read(&suffix) else {
        return Ok(absent::value());
    };
    let active = !absent::is_absent(&selected);
    if transition != Transition::PointerDown && !active {
        return Ok(absent::value());
    }

    let mut selected = if active { selected } else { payload::edge() };
    selected = payload::with_update(&selected, &update);
    let mut state = payload::editor_line(&selected, content);
    let presentation = LineEditPresentation::new(
        14.0,
        Brush::from(Color::new([0.0, 0.0, 0.0, 1.0])),
    )
    .with_affixes(prefix, suffix);

    let handled = match transition {
        Transition::PointerDown => {
            let Some(fields) = event.as_record() else {
                return Ok(absent::value());
            };
            if fields.get(&layout::vocabulary::BUTTON).and_then(Value::as_cell)
                != Some(layout::vocabulary::PRIMARY)
                || has_modifier(&event, layout::vocabulary::COMMAND)
            {
                return Ok(absent::value());
            }
            let Some(point) = point(&event) else {
                return Ok(absent::value());
            };
            let scale = number(&event, layout::vocabulary::SCALE).unwrap_or(1.0) as f32;
            let count = if active {
                number(&event, layout::vocabulary::COUNT)
                    .unwrap_or(1.0)
                    .clamp(1.0, f64::from(u8::MAX)) as u8
            } else {
                1
            };
            state.pointer_down(
                &presentation,
                &mut fonts.borrow_mut(),
                &mut layouts.borrow_mut(),
                scale,
                LineEditPointerDown {
                    point,
                    shift: has_modifier(&event, layout::vocabulary::SHIFT),
                    count,
                },
            );
            true
        }
        Transition::PointerMove => {
            if event
                .as_record()
                .and_then(|fields| fields.get(&layout::vocabulary::BUTTON))
                .and_then(Value::as_cell)
                != Some(layout::vocabulary::PRIMARY)
            {
                return Ok(absent::value());
            }
            let Some(point) = point(&event) else {
                return Ok(absent::value());
            };
            let scale = number(&event, layout::vocabulary::SCALE).unwrap_or(1.0) as f32;
            state.pointer_move(
                &presentation,
                &mut fonts.borrow_mut(),
                &mut layouts.borrow_mut(),
                scale,
                point,
            )
        }
        Transition::PointerUp => state.pointer_up(),
        Transition::Key => {
            let Some(event) = keyboard_event(&event) else {
                return Ok(absent::value());
            };
            state.handle_key(
                &presentation,
                &mut fonts.borrow_mut(),
                &mut layouts.borrow_mut(),
                &mut SystemTextClipboard,
                &event,
            )
        }
        Transition::Ime => {
            let Some(event) = ime_event(&event) else {
                return Ok(absent::value());
            };
            state.handle_ime(&event)
        }
    };
    if !handled {
        return Ok(absent::value());
    }
    let own_text = payload::stage(&selected).is_some_and(|stage| stage != payload::vocabulary::EDGE);
    Ok(payload::with_editor(&selected, &state, own_text))
}

fn number(value: &Value, field: CellId) -> Option<f64> {
    f64::read(value.as_record()?.get(&field)?)
}

fn point(value: &Value) -> Option<Point> {
    Some(Point::new(
        number(value, layout::vocabulary::X)?,
        number(value, layout::vocabulary::Y)?,
    ))
}

fn has_marker(value: &Value, field: CellId) -> bool {
    value
        .as_record()
        .is_some_and(|fields| fields.contains_key(&field))
}

fn has_modifier(value: &Value, modifier: CellId) -> bool {
    value
        .as_record()
        .and_then(|fields| fields.get(&layout::vocabulary::MODIFIERS))
        .and_then(Value::as_list)
        .is_some_and(|modifiers| {
            modifiers
                .values()
                .any(|value| value.as_cell() == Some(modifier))
        })
}

fn modifiers(value: &Value) -> Modifiers {
    let mut modifiers = Modifiers::empty();
    if has_modifier(value, layout::vocabulary::SHIFT) {
        modifiers |= Modifiers::SHIFT;
    }
    if has_modifier(value, layout::vocabulary::COMMAND) {
        modifiers |= if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };
    }
    modifiers
}

fn keyboard_event(value: &Value) -> Option<KeyboardEvent> {
    let fields = value.as_record()?;
    let key = Key::from_str(text::read(fields.get(&layout::vocabulary::CONTENT)?)?).ok()?;
    let state = match fields
        .get(&layout::vocabulary::EVENT_STATE)?
        .as_cell()?
    {
        cell if cell == layout::vocabulary::DOWN => KeyState::Down,
        cell if cell == layout::vocabulary::UP => KeyState::Up,
        _ => return None,
    };
    Some(KeyboardEvent {
        key,
        state,
        modifiers: modifiers(value),
        repeat: has_marker(value, layout::vocabulary::REPEAT),
        ..Default::default()
    })
}

fn index(value: &Value, field: CellId) -> Option<usize> {
    let value = number(value, field)?;
    (value >= 0.0 && value.fract() == 0.0).then_some(value as usize)
}

fn ime_event(value: &Value) -> Option<ImeEvent> {
    let fields = value.as_record()?;
    let state = fields
        .get(&layout::vocabulary::EVENT_STATE)?
        .as_cell()?;
    let content = || text::read(fields.get(&layout::vocabulary::CONTENT)?).map(str::to_owned);
    Some(match state {
        cell if cell == layout::vocabulary::IME_ENABLED => ImeEvent::Enabled,
        cell if cell == layout::vocabulary::IME_DISABLED => ImeEvent::Disabled,
        cell if cell == layout::vocabulary::IME_COMMIT => ImeEvent::Commit(content()?),
        cell if cell == layout::vocabulary::IME_PREEDIT => ImeEvent::Preedit(
            content()?,
            match (
                index(value, layout::vocabulary::START),
                index(value, layout::vocabulary::END),
            ) {
                (Some(start), Some(end)) => Some((start, end)),
                _ => None,
            },
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_libraries::selection as selection_capability;

    fn resources() -> (
        Rc<RefCell<FontContext>>,
        Rc<RefCell<LayoutContext<Brush>>>,
    ) {
        (
            Rc::new(RefCell::new(FontContext::new())),
            Rc::new(RefCell::new(LayoutContext::new())),
        )
    }

    fn invoke(function: CellId, selection: Value, event: Value) -> Value {
        let (fonts, layouts) = resources();
        let call = grap::call(
            Value::from(function),
            [
                (layout::vocabulary::EVENT, event),
                (layout::vocabulary::CONTENT, text::value("hi")),
                (line_edit::vocabulary::PREFIX, text::value("\"")),
                (line_edit::vocabulary::SUFFIX, text::value("\"")),
                (line_edit::vocabulary::UPDATE, grap::ffi(text::vocabulary::UPDATE)),
                (layout::vocabulary::SELECTION, selection),
            ],
        );
        grap::evaluate(&call, |_| None, &functions(fonts, layouts), 100).result
    }

    #[test]
    fn pointer_down_mounts_and_positions_an_editor() {
        let event = Value::record([
            (layout::vocabulary::BUTTON, Value::from(layout::vocabulary::PRIMARY)),
            (layout::vocabulary::X, f64::value(0.0)),
            (layout::vocabulary::Y, f64::value(0.0)),
            (layout::vocabulary::SCALE, f64::value(1.0)),
            (layout::vocabulary::COUNT, f64::value(1.0)),
            (layout::vocabulary::MODIFIERS, Value::list([])),
        ]);
        let result = invoke(
            line_edit::vocabulary::POINTER_DOWN,
            absent::value(),
            event,
        );
        assert_eq!(payload::editor_text(&result), Some("hi"));
        assert_eq!(payload::stage(&result), Some(payload::vocabulary::EDGE));
    }

    #[test]
    fn key_edits_the_payload_text_and_unselected_keys_decline() {
        let event = Value::record([
            (layout::vocabulary::EVENT_STATE, Value::from(layout::vocabulary::DOWN)),
            (layout::vocabulary::CONTENT, text::value("!")),
            (layout::vocabulary::MODIFIERS, Value::list([])),
        ]);
        let selected = payload::with_editor(
            &payload::edge(),
            &crate::selection::line_edit("hi"),
            false,
        );
        let result = invoke(line_edit::vocabulary::KEY, selected, event.clone());
        assert_eq!(payload::editor_text(&result), Some("hi!"));
        assert!(absent::is_absent(&invoke(
            line_edit::vocabulary::KEY,
            absent::value(),
            event,
        )));
    }

    #[test]
    fn the_grap_composed_pointer_handler_replaces_the_site_selection() {
        let stack = crate::stack::load::<()>();
        let expression = line_edit::call(
            text::value("hi"),
            grap::ffi(text::vocabulary::UPDATE),
            text::value("\""),
            text::value("\""),
            absent::value(),
        );
        let display = grap::evaluate(
            &expression,
            |cell| stack.library.value(cell).cloned(),
            &stack.foreign,
            500,
        );
        assert!(display.diagnostics.is_empty());
        let handler = display
            .result
            .as_record()
            .and_then(|fields| fields.get(&layout::vocabulary::ON_EVENT))
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&layout::vocabulary::HANDLER))
            .cloned()
            .expect("inactive line editor has a pointer handler");

        let selected = Rc::new(RefCell::new(None));
        let selection_functions = selection_capability::at(
            {
                let selected = selected.clone();
                move || selected.borrow().clone()
            },
            {
                let selected = selected.clone();
                move |value| *selected.borrow_mut() = value
            },
        );
        let (fonts, layouts) = resources();
        let event = Value::record([
            (
                layout::vocabulary::BUTTON,
                Value::from(layout::vocabulary::PRIMARY),
            ),
            (layout::vocabulary::X, f64::value(0.0)),
            (layout::vocabulary::Y, f64::value(0.0)),
            (layout::vocabulary::SCALE, f64::value(1.0)),
            (layout::vocabulary::COUNT, f64::value(1.0)),
            (layout::vocabulary::MODIFIERS, Value::list([])),
        ]);
        let evaluation = grap::apply(
            &handler,
            [(layout::vocabulary::EVENT, event)],
            |cell| stack.library.value(cell).cloned(),
            &stack
                .foreign
                .merge(functions(fonts, layouts))
                .merge(selection_functions),
            500,
        );
        assert!(evaluation.diagnostics.is_empty(), "{:?}", evaluation.diagnostics);
        assert!(!absent::is_absent(&evaluation.result));
        assert_eq!(
            selected
                .borrow()
                .as_ref()
                .and_then(payload::editor_text),
            Some("hi")
        );
    }
}
