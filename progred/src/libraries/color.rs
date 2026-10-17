//! Open sRGB RGB8 and RGBA8 conventions with an editable projected spelling.

use crate::libraries::{Library, f64, line_edit, name, text};
use crate::view::{Layout, ProjectionInput};
use gid::{Cells, Step, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x25d0e2034b4bd65bebb4811d65eab89c);
use crate::display::{
    PointEvent, PointUpdate, TextFamily, activatable, centered_row, col, descend, on_point,
    popover, widget,
};
use ::grap::{ForeignFunction, ForeignFunctions, RuntimeValue};
use puri::draw::CanvasSink;
use puri::{Affine, Canvas, Color, Rect, RoundedRect, Stroke};
use puri_widgets::color_picker::{self, Hsva};
use std::rc::Rc;

mod named;

pub mod vocabulary {
    use gid::CellId;

    pub const RGB: CellId = CellId::from_u128(0x6c8a17cbe463186cc8b07e536ccffa6b);
    pub const RGBA: CellId = CellId::from_u128(0xf77aef58bcc71dd838f089adc650e2d4);
    pub const UPDATE: CellId = CellId::from_u128(0xd3fd7b475567d1881c9023c90bd9864c);
    pub const PICKER: CellId = CellId::from_u128(0xd30d721bc1db563c75f899cc15c10580);
    pub const HUE: CellId = CellId::from_u128(0x4d226747147aa5cc9e6629e34c43a366);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0x55167b59c5530135d0738c9d2f7d264b);
}

#[derive(Clone, Copy)]
enum Encoded {
    Rgb([u8; 3]),
    Rgba([u8; 4]),
}

#[cfg(test)]
pub fn value(color: Color) -> Value {
    let rgba = color.to_rgba8();
    encoded_value(if rgba.a == 0xff {
        Encoded::Rgb([rgba.r, rgba.g, rgba.b])
    } else {
        Encoded::Rgba([rgba.r, rgba.g, rgba.b, rgba.a])
    })
}

fn encoded_value(color: Encoded) -> Value {
    Value::record([encoded_field(color)])
}

fn encoded_field(color: Encoded) -> (gid::CellId, Value) {
    let (field, bytes): (_, &[u8]) = match &color {
        Encoded::Rgb(bytes) => (vocabulary::RGB, bytes),
        Encoded::Rgba(bytes) => (vocabulary::RGBA, bytes),
    };
    (field, Value::from(bytes.to_vec()))
}

fn encoded(value: &Value) -> Option<Encoded> {
    let fields = value.as_record()?;
    encoded_bytes(
        fields.get(&vocabulary::RGBA).and_then(Value::as_blob),
        fields.get(&vocabulary::RGB).and_then(Value::as_blob),
    )
}

fn encoded_runtime(value: &RuntimeValue) -> Option<Encoded> {
    let rgba = value.field(vocabulary::RGBA);
    let rgb = value.field(vocabulary::RGB);
    encoded_bytes(
        rgba.as_ref().and_then(RuntimeValue::as_blob),
        rgb.as_ref().and_then(RuntimeValue::as_blob),
    )
}

fn encoded_bytes(rgba: Option<&[u8]>, rgb: Option<&[u8]>) -> Option<Encoded> {
    rgba.and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(Encoded::Rgba)
        .or_else(|| {
            rgb.and_then(|bytes| <[u8; 3]>::try_from(bytes).ok())
                .map(Encoded::Rgb)
        })
}

pub fn read(value: &Value) -> Option<Color> {
    encoded(value).map(decoded_color)
}

fn decoded_color(encoded: Encoded) -> Color {
    match encoded {
        Encoded::Rgb([red, green, blue]) => Color::from_rgba8(red, green, blue, 0xff),
        Encoded::Rgba([red, green, blue, alpha]) => Color::from_rgba8(red, green, blue, alpha),
    }
}

fn parse(spelling: &str) -> Option<Encoded> {
    let bytes = spelling.trim();
    let byte = |at| u8::from_str_radix(bytes.get(at..at + 2)?, 16).ok();
    let nibble = |at| {
        let nibble = u8::from_str_radix(bytes.get(at..at + 1)?, 16).ok()?;
        Some(nibble * 0x11)
    };
    match bytes.len() {
        3 => Some(Encoded::Rgb([nibble(0)?, nibble(1)?, nibble(2)?])),
        4 => Some(Encoded::Rgba([
            nibble(0)?,
            nibble(1)?,
            nibble(2)?,
            nibble(3)?,
        ])),
        6 => Some(Encoded::Rgb([byte(0)?, byte(2)?, byte(4)?])),
        8 => Some(Encoded::Rgba([byte(0)?, byte(2)?, byte(4)?, byte(6)?])),
        _ => None,
    }
}

fn spelling(color: Encoded) -> String {
    match color {
        Encoded::Rgb([red, green, blue]) => format!("{red:02x}{green:02x}{blue:02x}"),
        Encoded::Rgba([red, green, blue, alpha]) => {
            format!("{red:02x}{green:02x}{blue:02x}{alpha:02x}")
        }
    }
}

fn replace_color(current: &Value, color: Encoded) -> Option<Value> {
    let mut fields = current.as_record()?.clone();
    fields.remove(&vocabulary::RGB);
    fields.remove(&vocabulary::RGBA);
    let (field, value) = encoded_field(color);
    fields.insert(field, value);
    Some(Value::Record(fields))
}

fn picker_selection(hue: f64) -> Value {
    Value::record([(
        vocabulary::PICKER,
        Value::record([(vocabulary::HUE, f64::value(hue))]),
    )])
}

fn new_color() -> crate::display::Completion {
    crate::display::Completion::new("new color", |session| {
        let Some(selection) = session.model.selection.as_ref() else {
            return;
        };
        if selection.stage(&session.sources()) != crate::selection::Stage::Pending
            || !selection.writable(&session.sources())
            || selection.value(&session.sources()).is_some()
        {
            return;
        }
        let next = crate::selection::Selection::from_scoped_payload(
            selection.root(),
            &session.sources(),
            selection.scope().clone(),
            selection.path().to_vec(),
            picker_selection(0.0),
        );
        session.model.selection = Some(next);
    })
    .with_aliases(["color", "colour"])
}

fn completions(
    request: &crate::display::CompletionRequest<'_>,
) -> Option<Vec<crate::display::Completion>> {
    (!request.raw
        && request.kind == crate::display::CompletionKind::Value
        && request.scope == crate::display::CompletionScope::Everything
        && request.value().is_none())
    .then(|| vec![new_color()])
}

/// The seed belongs only to the picker. The first point interaction writes the
/// chosen color through the same location and gesture machinery as an edit.
fn missing_picker(input: &ProjectionInput<'_, RuntimeValue>) -> Option<Layout> {
    input.value.is_none().then_some(())?;
    input.writable.then_some(())?;
    let hue = picker_hue(input.selection)?;
    let seed = Encoded::Rgb([255, 0, 0]);
    let close = input.targets.current().select_with;
    Some(widget::before(
        popover(
            crate::display::dim("color"),
            picker(&encoded_value(seed), seed, hue),
        ),
        Rc::new(move |_| {
            let close = close.clone();
            Box::new(move |output, _| {
                use puri::handler::HasHandler;
                output.handler().on_key(move |world, event| {
                    if event.state.is_down()
                        && event.key
                            == ui_events::keyboard::Key::Named(
                                ui_events::keyboard::NamedKey::Escape,
                            )
                    {
                        close(world, crate::selection::payload::empty_pending());
                        true
                    } else {
                        false
                    }
                });
            })
        }),
    ))
}

fn picker_hue(selection: Option<&Value>) -> Option<f64> {
    selection
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::PICKER))
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::HUE))
        .and_then(f64::read)
}

fn without_picker(selection: Option<&Value>) -> Value {
    let mut fields = selection
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    fields.remove(&vocabulary::PICKER);
    Value::Record(fields)
}

fn picker_leaf(height: f64, color: Hsva, draw: fn(Hsva, &mut dyn CanvasSink, Affine)) -> Layout {
    Layout::widget(Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        widget::paint(
            widget::Extent {
                width: color_picker::WIDTH * scale,
                ascent: height * scale,
                descent: 0.0,
            },
            move |canvas, placement| {
                draw(
                    color,
                    canvas,
                    Affine::translate((placement.rect.x0, placement.rect.y0))
                        * Affine::scale(scale),
                );
            },
        )
    }))
}

fn picker_update(
    original: Value,
    color: Hsva,
    update: impl Fn(Hsva, PointEvent) -> Hsva + 'static,
    update_hue: bool,
) -> crate::display::PointHandler {
    Rc::new(move |point| {
        let color = update(color, point);
        let rgba = color.to_rgba8();
        let encoded = if matches!(encoded(&original), Some(Encoded::Rgba(_))) {
            Encoded::Rgba(rgba)
        } else {
            Encoded::Rgb([rgba[0], rgba[1], rgba[2]])
        };
        PointUpdate {
            value: replace_color(&original, encoded).unwrap_or_else(|| original.clone()),
            selection: update_hue.then(|| picker_selection(color.hue)),
        }
    })
}

fn hsva(encoded: Encoded) -> Hsva {
    let rgba = match encoded {
        Encoded::Rgb([red, green, blue]) => [red, green, blue, 0xff],
        Encoded::Rgba(rgba) => rgba,
    };
    Hsva::from_rgba8(rgba)
}

fn picker(original: &Value, encoded: Encoded, hue: f64) -> Layout {
    let color = Hsva {
        hue,
        ..hsva(encoded)
    };
    let plane = on_point(
        picker_leaf(color_picker::PLANE_HEIGHT, color, color_picker::plane),
        picker_update(
            original.clone(),
            color,
            |color, point| color.with_plane(point.x, point.y),
            false,
        ),
    );
    let hue = on_point(
        picker_leaf(color_picker::RAIL_HEIGHT, color, color_picker::hue),
        picker_update(
            original.clone(),
            color,
            |color, point| color.with_hue(point.x),
            true,
        ),
    );
    col(
        0,
        8.0,
        [plane, hue]
            .into_iter()
            .chain(matches!(encoded, Encoded::Rgba(_)).then(|| {
                on_point(
                    picker_leaf(color_picker::RAIL_HEIGHT, color, color_picker::alpha),
                    picker_update(
                        original.clone(),
                        color,
                        |color, point| color.with_alpha(point.x),
                        false,
                    ),
                )
            })),
    )
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::UPDATE,
        ForeignFunction::from_value(|context, call, environment| {
            let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
            };
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context.eval_to_value(current, environment)?;
            let input = context.eval_to_value(input, environment)?;
            Ok(text::read(&input)
                .and_then(|spelling| edit(spelling, Some(&current)))
                .unwrap_or_else(|| {
                    ::grap::absent::with_detail(
                        vocabulary::INVALID_INPUT,
                        line_edit::vocabulary::INPUT,
                        input.clone(),
                    )
                }))
        })
        .parameters([line_edit::vocabulary::INPUT, line_edit::vocabulary::CURRENT])
        .tracked(),
    )
}

pub fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    replace_color(current?, parse(spelling)?)
}

fn swatch(color: Color) -> Layout {
    Layout::widget(Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let border = context.inputs.styles.dim.brush.clone();
        widget::paint(
            widget::Extent {
                width: 15.0 * scale,
                ascent: 11.0 * scale,
                descent: 4.0 * scale,
            },
            move |canvas, placement| {
                let shape = RoundedRect::from_rect(Rect::new(0.5, 0.5, 14.5, 14.5), 2.5);
                let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                    * Affine::scale(scale);
                canvas.fill(shape, color, transform);
                canvas.stroke(shape, Stroke::new(1.0), border, transform);
            },
        )
    }))
}

pub fn display(input: &ProjectionInput<'_, RuntimeValue>) -> Option<Layout> {
    if input.value.is_none() {
        return missing_picker(input);
    }
    let encoded = encoded_runtime(input.value?)?;
    let color = decoded_color(encoded);
    let initial_hue = hsva(encoded).hue;
    let selected_hue = picker_hue(input.selection);
    let next_selection = if selected_hue.is_some() {
        without_picker(input.selection)
    } else {
        picker_selection(initial_hue)
    };
    let target = input.targets.current();
    // Read-only colors still select; only a writable one opens the picker.
    let activate: crate::display::ActionHandler<crate::Editor> = if input.writable {
        let select_picker = target.select_with;
        Rc::new(move |world| select_picker(world, next_selection.clone()))
    } else {
        target.select
    };
    let swatch = activatable(swatch(color), target.hover, activate);
    let swatch = if input.writable
        && let Some(hue) = selected_hue
    {
        popover(swatch, picker(input.value?.as_value(), encoded, hue))
    } else {
        swatch
    };
    let name = input
        .value?
        .field(name::vocabulary::NAME)
        .filter(|value| value.as_text().is_some())
        .map(|_| descend(Step::Key(name::vocabulary::NAME), None, None));
    let named = name.is_some();
    let spelling = crate::display::line_edit(hex_editor(encoded));
    Some(crate::display::structure::with_extra_fields(
        input,
        |key| {
            key == vocabulary::RGB
                || key == vocabulary::RGBA
                || (named && key == name::vocabulary::NAME)
        },
        centered_row(
            4.0,
            std::iter::once(swatch)
                .chain(name)
                .chain(std::iter::once(spelling)),
        ),
    ))
}

fn hex_editor(color: Encoded) -> crate::display::LineEdit {
    line_edit::description(
        spelling(color),
        None::<String>,
        line_edit::native(edit),
        "#",
        "",
        TextFamily::Monospace,
    )
}

pub fn library() -> Library {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::RGB, name::record("rgb", []));
    cells.set_value(vocabulary::RGBA, name::record("rgba", []));
    cells.set_value(vocabulary::UPDATE, name::record("color update", []));
    cells.set_value(vocabulary::PICKER, name::record("color picker", []));
    cells.set_value(vocabulary::HUE, name::record("hue", []));
    cells.set_value(
        vocabulary::INVALID_INPUT,
        name::record("invalid color input", []),
    );
    named::insert(&mut cells);
    Library::named(
        ID,
        "color",
        crate::libraries::Definitions::from_parts(cells, functions()),
    )
    .with_projection(crate::display::runtime_partial(display))
    .with_completions(completions)
}

#[cfg(test)]
mod tests;
