//! The stock line editor as Grap composition. The function returns
//! ordinary text display data wrapped in generic event handlers.
//! Puri's geometry and editing transitions enter only as scoped FFIs
//! supplied by the host while an event is being dispatched.

use crate::{Library, absent, control, layout, name, selection};
use gid::{CellId, Cells, Value};
#[cfg(test)]
use grap_runtime::{ForeignFunction, ForeignFunctions};

pub mod vocabulary {
    use gid::CellId;

    /// Grap function building the stock editable-text display.
    pub const LINE_EDIT: CellId = CellId::from_u128(0x9a310ea0c1669f01be6f2db9e8783454);

    /// Pure Puri operation returning shaped text metrics and ordinary
    /// vector commands for the selection and caret.
    pub const GEOMETRY: CellId = CellId::from_u128(0xf411f215e61f37ab7589e154dde6ca44);
    pub const SELECTION_COMMANDS: CellId =
        CellId::from_u128(0x2c6bd379ed22eadd1c83a2ba6bd5de1a);
    pub const CURSOR_COMMANDS: CellId =
        CellId::from_u128(0x266731c97496c012dc7288b6292b2b62);

    /// The line update call's arguments.
    pub const CURRENT: CellId = CellId::from_u128(0x0e6a49d1c78325bfa9231c05e84d67fb);
    pub const INPUT: CellId = CellId::from_u128(0xd58c17f3402b96ea6f0e4a2b91c738d5);
    pub const UPDATE: CellId = CellId::from_u128(0xd6293e85f0b7c41a374b8f0e29d1a6c5);
    pub const PREFIX: CellId = CellId::from_u128(0x30c581b6e9f2d74a92a5c3f7e14608bd);
    pub const SUFFIX: CellId = CellId::from_u128(0xac47f2d90b6e83156e93a1b4d5270c8f);

    // Private binders used while assembling drawing data.
    pub const DRAW_TEXT: CellId = CellId::from_u128(0xff0fdd8e8d443c6c786a29a4b0fab24d);
    pub const DRAW_WIDTH: CellId = CellId::from_u128(0x3662283a16b26f5943508e10796833e5);
    pub const DRAW_ASCENT: CellId = CellId::from_u128(0x6833059c9062a948e920f83d35dbcf94);
    pub const DRAW_DESCENT: CellId = CellId::from_u128(0xd03aa0d1a3bfc00e9a62652537c9f0a9);
    pub const DRAW_SELECTION: CellId =
        CellId::from_u128(0x59d3f77d6669394ee9cb500a1f94a979);
    pub const DRAW_CURSOR: CellId = CellId::from_u128(0x5ed6b931c9cdd4ca8bf9a1d3ddd82f1e);

    /// Host capabilities implementing one Puri line-edit transition.
    /// They exist only during dispatch at a concrete projection site.
    pub const POINTER_DOWN: CellId =
        CellId::from_u128(0x14cf6aea16c232a2c74e9af4f58092f5);
    pub const POINTER_MOVE: CellId =
        CellId::from_u128(0xc6063f258a27937a9504a6a0476ca9a3);
    pub const POINTER_UP: CellId =
        CellId::from_u128(0xe03aae1889fa94d9b0057e0c37a6d053);
    pub const KEY: CellId = CellId::from_u128(0x0839cb84ce3c54c253a63d82b9e8fea7);
    pub const IME: CellId = CellId::from_u128(0x80ac560f6af712fdd7fbe5ac1ee8b135);

    /// Private binder for a transition's next selection payload.
    pub const NEXT: CellId = CellId::from_u128(0x845a5827e6f686b9a11a9f4673f1967e);
}

fn unquote(expression: Value) -> Value {
    Value::record([(control::vocabulary::UNQUOTE, expression)])
}

fn bind(cell: CellId) -> Value {
    Value::record([(control::vocabulary::BIND, Value::from(cell))])
}

fn alternative(pattern: Value, expression: Value) -> Value {
    Value::record([
        (control::vocabulary::PATTERN, pattern),
        (grap_runtime::vocabulary::EXPRESSION, expression),
    ])
}

fn transition(function: CellId) -> Value {
    grap_runtime::call(
        Value::from(function),
        [
            (layout::vocabulary::EVENT, Value::from(layout::vocabulary::EVENT)),
            (
                layout::vocabulary::CONTENT,
                Value::from(layout::vocabulary::CONTENT),
            ),
            (
                vocabulary::PREFIX,
                Value::from(vocabulary::PREFIX),
            ),
            (
                vocabulary::SUFFIX,
                Value::from(vocabulary::SUFFIX),
            ),
            (
                vocabulary::UPDATE,
                Value::from(vocabulary::UPDATE),
            ),
            (
                layout::vocabulary::SELECTION,
                grap_runtime::call(Value::from(selection::vocabulary::GET), []),
            ),
        ],
    )
}

fn handle(function: CellId) -> Value {
    let next = grap_runtime::call(
        Value::from(selection::vocabulary::SET),
        [(selection::vocabulary::VALUE, Value::from(vocabulary::NEXT))],
    );
    let choose = grap_runtime::call(
        Value::from(control::vocabulary::CASE),
        [
            (control::vocabulary::VALUE, transition(function)),
            (
                control::vocabulary::ALTERNATIVES,
                Value::list([
                    alternative(absent::value(), absent::value()),
                    alternative(bind(vocabulary::NEXT), next),
                ]),
            ),
            (control::vocabulary::DEFAULT, absent::value()),
        ],
    );
    choose
}

/// One generic event handler dispatching the event data to a host
/// transition, then installing its returned selection payload.
fn handler(handlers: impl IntoIterator<Item = (CellId, CellId)>) -> Value {
    let alternatives = handlers.into_iter().map(|(kind, function)| {
        alternative(
            Value::record([(layout::vocabulary::EVENT_KIND, Value::from(kind))]),
            handle(function),
        )
    });
    grap_runtime::lambda(
        [layout::vocabulary::EVENT],
        grap_runtime::call(
            Value::from(control::vocabulary::CASE),
            [
                (
                    control::vocabulary::VALUE,
                    Value::from(layout::vocabulary::EVENT),
                ),
                (
                    control::vocabulary::ALTERNATIVES,
                    Value::list(alternatives),
                ),
                (control::vocabulary::DEFAULT, absent::value()),
            ],
        ),
    )
}

fn on_events(
    child: Value,
    handlers: impl IntoIterator<Item = (CellId, CellId)>,
) -> Value {
    let handlers = handlers.into_iter().collect::<Vec<_>>();
    layout::on(child, unquote(handler(handlers)))
}

fn definition() -> Value {
    let geometry = grap_runtime::call(
        Value::from(vocabulary::GEOMETRY),
        [
            (
                layout::vocabulary::CONTENT,
                Value::from(layout::vocabulary::CONTENT),
            ),
            (
                vocabulary::PREFIX,
                Value::from(vocabulary::PREFIX),
            ),
            (
                vocabulary::SUFFIX,
                Value::from(vocabulary::SUFFIX),
            ),
            (
                layout::vocabulary::SELECTION,
                Value::from(layout::vocabulary::SELECTION),
            ),
        ],
    );
    let geometry_pattern = Value::record([
        (layout::vocabulary::CONTENT, bind(vocabulary::DRAW_TEXT)),
        (layout::vocabulary::WIDTH, bind(vocabulary::DRAW_WIDTH)),
        (layout::vocabulary::ASCENT, bind(vocabulary::DRAW_ASCENT)),
        (
            layout::vocabulary::DESCENT,
            bind(vocabulary::DRAW_DESCENT),
        ),
        (
            vocabulary::SELECTION_COMMANDS,
            bind(vocabulary::DRAW_SELECTION),
        ),
        (
            vocabulary::CURSOR_COMMANDS,
            bind(vocabulary::DRAW_CURSOR),
        ),
    ]);
    let vector = |commands: CellId| {
        Value::record([(
            layout::vocabulary::VECTOR,
            Value::record([
                (
                    layout::vocabulary::WIDTH,
                    unquote(Value::from(vocabulary::DRAW_WIDTH)),
                ),
                (
                    layout::vocabulary::ASCENT,
                    unquote(Value::from(vocabulary::DRAW_ASCENT)),
                ),
                (
                    layout::vocabulary::DESCENT,
                    unquote(Value::from(vocabulary::DRAW_DESCENT)),
                ),
                (
                    layout::vocabulary::COMMANDS,
                    unquote(Value::from(commands)),
                ),
            ]),
        )])
    };
    let drawing = layout::overlay([
        vector(vocabulary::DRAW_SELECTION),
        Value::record([(
            layout::vocabulary::TEXT,
            Value::record([
                (
                    layout::vocabulary::CONTENT,
                    unquote(Value::from(vocabulary::DRAW_TEXT)),
                ),
                (
                    layout::vocabulary::FACE,
                    Value::from(layout::vocabulary::STRING_FACE),
                ),
            ]),
        )]),
        vector(vocabulary::DRAW_CURSOR),
    ]);
    let drawing = grap_runtime::call(
        Value::from(control::vocabulary::CASE),
        [
            (control::vocabulary::VALUE, geometry),
            (
                control::vocabulary::ALTERNATIVES,
                Value::list([alternative(
                    geometry_pattern,
                    grap_runtime::call(
                        Value::from(control::vocabulary::QUOTE),
                        [(grap_runtime::vocabulary::EXPRESSION, drawing)],
                    ),
                )]),
            ),
            (control::vocabulary::DEFAULT, absent::value()),
        ],
    );

    let drawing = layout::hoverable(unquote(drawing));
    let inactive = on_events(
        drawing.clone(),
        [(
            layout::vocabulary::POINTER_DOWN,
            vocabulary::POINTER_DOWN,
        )],
    );
    let active = on_events(
        drawing,
        [
            (layout::vocabulary::POINTER_DOWN, vocabulary::POINTER_DOWN),
            (layout::vocabulary::POINTER_MOVE, vocabulary::POINTER_MOVE),
            (layout::vocabulary::POINTER_UP, vocabulary::POINTER_UP),
            (layout::vocabulary::KEY, vocabulary::KEY),
            (layout::vocabulary::IME, vocabulary::IME),
        ],
    );

    let quote = |display| {
        grap_runtime::call(
            Value::from(control::vocabulary::QUOTE),
            [(grap_runtime::vocabulary::EXPRESSION, display)],
        )
    };
    let body = grap_runtime::call(
        Value::from(control::vocabulary::CASE),
        [
            (
                control::vocabulary::VALUE,
                Value::from(layout::vocabulary::SELECTION),
            ),
            (
                control::vocabulary::ALTERNATIVES,
                Value::list([
                    alternative(absent::value(), quote(inactive)),
                    alternative(bind(vocabulary::NEXT), quote(active.clone())),
                ]),
            ),
            (control::vocabulary::DEFAULT, quote(active)),
        ],
    );
    name::record(
        "line edit",
        [
            (
                grap_runtime::vocabulary::PARAMS,
                Value::list([
                    Value::from(layout::vocabulary::CONTENT),
                    Value::from(vocabulary::UPDATE),
                    Value::from(vocabulary::PREFIX),
                    Value::from(vocabulary::SUFFIX),
                    Value::from(layout::vocabulary::SELECTION),
                ]),
            ),
            (grap_runtime::vocabulary::BODY, body),
        ],
    )
}

pub fn call(
    content: Value,
    update: Value,
    prefix: Value,
    suffix: Value,
    selection: Value,
) -> Value {
    grap_runtime::call(
        Value::from(vocabulary::LINE_EDIT),
        [
            (layout::vocabulary::CONTENT, content),
            (vocabulary::UPDATE, update),
            (vocabulary::PREFIX, prefix),
            (vocabulary::SUFFIX, suffix),
            (layout::vocabulary::SELECTION, selection),
        ],
    )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::LINE_EDIT, definition());
    for (cell, spelling) in [
        (vocabulary::GEOMETRY, "line edit geometry"),
        (vocabulary::CURRENT, "current"),
        (vocabulary::INPUT, "input"),
        (vocabulary::UPDATE, "update"),
        (vocabulary::PREFIX, "prefix"),
        (vocabulary::SUFFIX, "suffix"),
        (vocabulary::POINTER_DOWN, "line edit pointer down"),
        (vocabulary::POINTER_MOVE, "line edit pointer move"),
        (vocabulary::POINTER_UP, "line edit pointer up"),
        (vocabulary::KEY, "line edit key"),
        (vocabulary::IME, "line edit IME"),
        (vocabulary::NEXT, "_next line edit selection"),
        (vocabulary::SELECTION_COMMANDS, "selection commands"),
        (vocabulary::CURSOR_COMMANDS, "cursor commands"),
        (vocabulary::DRAW_TEXT, "_line edit draw text"),
        (vocabulary::DRAW_WIDTH, "_line edit draw width"),
        (vocabulary::DRAW_ASCENT, "_line edit draw ascent"),
        (vocabulary::DRAW_DESCENT, "_line edit draw descent"),
        (vocabulary::DRAW_SELECTION, "_line edit draw selection"),
        (vocabulary::DRAW_CURSOR, "_line edit draw cursor"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library {
        cells,
        ..Library::default()
    }
}

#[cfg(test)]
pub(crate) fn test_geometry_functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::GEOMETRY,
        ForeignFunction::new(|context, call, environment| {
            let read = |context: &mut grap_runtime::Context, field| {
                let expression = context.field(call, field)?.clone();
                context.eval(&expression, environment).ok()
            };
            let content = read(context, layout::vocabulary::CONTENT)
                .and_then(|value| crate::text::read(&value).map(str::to_owned));
            let prefix = read(context, vocabulary::PREFIX)
                .and_then(|value| crate::text::read(&value).map(str::to_owned));
            let suffix = read(context, vocabulary::SUFFIX)
                .and_then(|value| crate::text::read(&value).map(str::to_owned));
            Ok(match (content, prefix, suffix) {
                (Some(content), Some(prefix), Some(suffix)) => Value::record([
                    (
                        layout::vocabulary::CONTENT,
                        crate::text::value(format!("{prefix}{content}{suffix}")),
                    ),
                    (layout::vocabulary::WIDTH, crate::f64::value(20.0)),
                    (layout::vocabulary::ASCENT, crate::f64::value(10.0)),
                    (layout::vocabulary::DESCENT, crate::f64::value(3.0)),
                    (vocabulary::SELECTION_COMMANDS, Value::list([])),
                    (vocabulary::CURSOR_COMMANDS, Value::list([])),
                ]),
                _ => absent::value(),
            })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{control, layout, selection, text};

    fn event_kinds(handler: &Value) -> Vec<CellId> {
        handler
            .as_record()
            .and_then(|fields| fields.get(&grap_runtime::vocabulary::CLOSURE))
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&grap_runtime::vocabulary::BODY))
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&control::vocabulary::ALTERNATIVES))
            .and_then(Value::as_list)
            .expect("event handler case alternatives")
            .values()
            .map(|alternative| {
                alternative
                    .as_record()
                    .and_then(|fields| fields.get(&control::vocabulary::PATTERN))
                    .and_then(Value::as_record)
                    .and_then(|fields| fields.get(&layout::vocabulary::EVENT_KIND))
                    .and_then(Value::as_cell)
                    .expect("event-kind pattern")
            })
            .collect()
    }

    #[test]
    fn grap_builds_generic_text_with_one_five_way_event_handler() {
        let library = Library::<(), ()>::merge_all([
            name::library(),
            text::library(),
            control::library(),
            selection::library(),
            layout::library(),
            super::library(),
        ]);
        let expression = call(
            text::value("42"),
            grap_runtime::ffi(text::vocabulary::UPDATE),
            text::value(""),
            text::value("°"),
            Value::record([]),
        );
        let evaluation = grap_runtime::evaluate(
            &expression,
            |cell| library.cells.value(cell).cloned(),
            &library.functions.clone().merge(test_geometry_functions()),
            500,
        );
        assert!(evaluation.diagnostics.is_empty(), "{:?}", evaluation.diagnostics);

        let content = evaluation
            .result
            .as_record()
            .and_then(|fields| fields.get(&layout::vocabulary::ON_EVENT))
            .and_then(Value::as_record)
            .expect("on-event wrapper");
        let handler = content
            .get(&layout::vocabulary::HANDLER)
            .expect("event handler");
        assert_eq!(
            event_kinds(handler),
            [
                layout::vocabulary::POINTER_DOWN,
                layout::vocabulary::POINTER_MOVE,
                layout::vocabulary::POINTER_UP,
                layout::vocabulary::KEY,
                layout::vocabulary::IME,
            ]
        );
        let value = content
            .get(&layout::vocabulary::CHILD)
            .unwrap()
            .as_record()
            .and_then(|fields| fields.get(&layout::vocabulary::HOVERABLE))
            .unwrap();
        let layers = value
            .as_record()
            .and_then(|fields| fields.get(&layout::vocabulary::OVERLAY))
            .and_then(Value::as_list)
            .unwrap();
        let text = layers
            .values()
            .nth(1)
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&layout::vocabulary::TEXT))
            .and_then(Value::as_record)
            .unwrap();
        assert_eq!(
            text.get(&layout::vocabulary::CONTENT).and_then(text::read),
            Some("42°")
        );
    }

    #[test]
    fn an_inactive_editor_has_one_pointer_down_event_dispatch() {
        let library = Library::<(), ()>::merge_all([
            name::library(),
            text::library(),
            control::library(),
            selection::library(),
            layout::library(),
            super::library(),
        ]);
        let evaluation = grap_runtime::evaluate(
            &call(
                text::value("42"),
                grap_runtime::ffi(text::vocabulary::UPDATE),
                text::value(""),
                text::value(""),
                absent::value(),
            ),
            |cell| library.cells.value(cell).cloned(),
            &library.functions.clone().merge(test_geometry_functions()),
            500,
        );
        let wrapper = evaluation
            .result
            .as_record()
            .and_then(|fields| fields.get(&layout::vocabulary::ON_EVENT))
            .and_then(Value::as_record)
            .unwrap();
        assert_eq!(
            event_kinds(wrapper.get(&layout::vocabulary::HANDLER).unwrap()),
            [layout::vocabulary::POINTER_DOWN]
        );
        assert!(
            wrapper
                .get(&layout::vocabulary::CHILD)
                .and_then(Value::as_record)
                .is_some_and(|fields| fields.contains_key(&layout::vocabulary::HOVERABLE))
        );
    }
}
