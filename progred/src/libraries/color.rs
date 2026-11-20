//! sRGB colors, literals of two kinds: `{isa: rgb, bytes: <3 bytes>}` and
//! `{isa: rgba, bytes: <4 bytes>}`, drawn as a swatch and an editable hex
//! spelling.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{
    PointEvent, PointUpdate, TextFamily, centered_row, col, on_activate, on_hover, on_point,
    popover, widget,
};
use crate::libraries::{Library, f64, name};
use ::grap::RuntimeValue;
use gid::Value;
use kurbo::{Affine, Rect, RoundedRect, Stroke};
use peniko::Color;
use puri::draw::{Canvas, CanvasSink};
use puri_widgets::color_picker::{self, Hsva};
use std::rc::Rc;

pub const ID: gid::CellId = gid::CellId::from_u128(0x25d0e2034b4bd65bebb4811d65eab89c);

mod named;

pub mod vocabulary {
    use gid::CellId;

    pub const RGB: CellId = CellId::from_u128(0x6c8a17cbe463186cc8b07e536ccffa6b);
    pub const RGBA: CellId = CellId::from_u128(0xf77aef58bcc71dd838f089adc650e2d4);
    pub const PICKER: CellId = CellId::from_u128(0xd30d721bc1db563c75f899cc15c10580);
    pub const HUE: CellId = CellId::from_u128(0x4d226747147aa5cc9e6629e34c43a366);
}

#[derive(Clone, Copy)]
enum Encoded {
    Rgb([u8; 3]),
    Rgba([u8; 4]),
}

impl Encoded {
    /// Its fields as a literal.
    fn fields(self) -> [(gid::CellId, Value); 2] {
        match self {
            Self::Rgb(bytes) => ::grap::literal::fields(vocabulary::RGB, bytes.to_vec()),
            Self::Rgba(bytes) => ::grap::literal::fields(vocabulary::RGBA, bytes.to_vec()),
        }
    }

    /// The color whose bytes `literal` gives for its kind, if it's one.
    fn of(literal: impl Fn(gid::CellId) -> Option<Vec<u8>>) -> Option<Self> {
        let rgba = || <[u8; 4]>::try_from(literal(vocabulary::RGBA)?).ok();
        let rgb = || <[u8; 3]>::try_from(literal(vocabulary::RGB)?).ok();
        rgba().map(Self::Rgba).or_else(|| rgb().map(Self::Rgb))
    }
}

/// An opaque color, `{isa: rgb, bytes}`.
#[cfg(any(test, feature = "cam-profile"))]
pub fn rgb(bytes: [u8; 3]) -> Value {
    encoded_value(Encoded::Rgb(bytes))
}

fn encoded_value(color: Encoded) -> Value {
    Value::record(color.fields())
}

fn encoded(value: &Value) -> Option<Encoded> {
    Encoded::of(|kind| ::grap::literal::bytes(value, kind).map(<[u8]>::to_vec))
}

fn encoded_runtime(value: &RuntimeValue) -> Option<Encoded> {
    Encoded::of(|kind| value.literal(kind).map(<[u8]>::to_vec))
}

pub fn read(value: &Value) -> Option<Color> {
    encoded(value).map(decoded_color)
}

/// An opaque color's red, green and blue bytes.
pub fn read_opaque(value: &Value) -> Option<[u8; 3]> {
    match encoded(value)? {
        Encoded::Rgb(rgb) => Some(rgb),
        Encoded::Rgba([red, green, blue, 0xff]) => Some([red, green, blue]),
        Encoded::Rgba(_) => None,
    }
}

fn decoded_color(encoded: Encoded) -> Color {
    match encoded {
        Encoded::Rgb([red, green, blue]) => Color::from_rgba8(red, green, blue, 0xff),
        Encoded::Rgba([red, green, blue, alpha]) => Color::from_rgba8(red, green, blue, alpha),
    }
}

fn parse(spelling: &str) -> Option<Encoded> {
    let bytes = spelling.trim();
    // Digits only: from_str_radix alone would take a sign, reading `+1+2+3`.
    bytes
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit())
        .then_some(())?;
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

/// `current` holding `color` instead, its other fields kept.
fn replace_color(current: &Value, color: Encoded) -> Option<Value> {
    current.as_record()?;
    Some(super::keeping_fields(Some(current), encoded_value(color)))
}

fn picker_selection(hue: f64) -> Value {
    Value::record([(
        vocabulary::PICKER,
        Value::record([(vocabulary::HUE, f64::value(hue))]),
    )])
}

fn new_color() -> crate::display::Completion {
    crate::display::Completion::new("color", crate::display::Offered::New(None), |session| {
        if let Some(picking) = session.pending()
            && picking.stage() == crate::selection::Stage::Pending
            && picking.value().is_none()
        {
            picking.select_with(picker_selection(0.0));
        }
    })
    .with_aliases(["colour"])
}

fn completions(
    request: &crate::display::CompletionRequest<'_>,
) -> Option<Vec<crate::display::Completion>> {
    (request.kind == crate::display::CompletionKind::Value
        && request.scope == crate::display::CompletionScope::Everything
        && request.value().is_none())
    .then(|| vec![new_color()])
}

/// The seed belongs only to the picker. The first point interaction writes the
/// chosen color through the same location and gesture machinery as an edit.
fn missing_picker<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    input.writable.then_some(())?;
    let hue = picker_hue(input.selection)?;
    let seed = Encoded::Rgb([255, 0, 0]);
    let site = input.site();
    Some(widget::before(
        popover(
            crate::display::dim("color"),
            picker(&encoded_value(seed), seed, hue),
        ),
        Rc::new(move |_| {
            let site = site.clone();
            Box::new(move |output, _| {
                use puri::handler::HasHandler;
                output.handler().on_key(move |world, event| {
                    if event.state.is_down()
                        && event.key
                            == ui_events::keyboard::Key::Named(
                                ui_events::keyboard::NamedKey::Escape,
                            )
                    {
                        site.open(&mut world.session())
                            .select_with(crate::selection::payload::empty_pending());
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
    fields.set(vocabulary::PICKER, None);
    Value::Record(fields)
}

fn picker_leaf<W: Host>(
    height: f64,
    color: Hsva,
    draw: fn(Hsva, &mut dyn CanvasSink, Affine),
) -> Layout<W> {
    Layout::widget(Rc::new(move |context| {
        let scale = context.viewing().scale();
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

fn picker<W: Host>(original: &Value, encoded: Encoded, hue: f64) -> Layout<W> {
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

fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    replace_color(current?, parse(spelling)?)
}

fn swatch<W: Host>(color: Color) -> Layout<W> {
    Layout::widget(Rc::new(move |context| {
        let scale = context.viewing().scale();
        let border = context.face_brush(crate::display::Face::Dim);
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

/// A color as its swatch, any text name, and its hex spelling.
fn present<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let value = input.value?;
    let encoded = encoded_runtime(value)?;
    let color = decoded_color(encoded);
    let initial_hue = hsva(encoded).hue;
    let selected_hue = picker_hue(input.selection);
    let next_selection = if selected_hue.is_some() {
        without_picker(input.selection)
    } else {
        picker_selection(initial_hue)
    };
    let site = input.site();
    // Read-only colors still select; only a writable one opens the picker.
    let activate: crate::display::Action = if input.writable {
        let site = site.clone();
        Rc::new(move |session| {
            site.open(session).select_with(next_selection.clone());
            true
        })
    } else {
        crate::display::widget::navigation::selecting(site.clone())
    };
    let hover = W::Hover::from(site.hover());
    let swatch = on_hover(on_activate(swatch(color), hover.clone(), activate), hover);
    let swatch = if input.writable
        && let Some(hue) = selected_hue
    {
        popover(swatch, picker(value.as_value(), encoded, hue))
    } else {
        swatch
    };
    let spelling = crate::display::line_edit(hex_editor(encoded));
    Some(crate::display::structure::Fields::default().after(
        input,
        |key| crate::libraries::isa::literal_field(key) || key == name::vocabulary::NAME,
        name::with_name(input, centered_row(4.0, [swatch, spelling])),
    ))
}

fn hex_editor(color: Encoded) -> crate::display::LineEdit {
    crate::display::LineEdit {
        text: spelling(color),
        update: Rc::new(edit),
        prefix: "#".into(),
        suffix: String::new(),
        family: TextFamily::Monospace,
        placeholder: None,
    }
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "color",
        [
            (vocabulary::RGB, "rgb"),
            (vocabulary::RGBA, "rgba"),
            (vocabulary::PICKER, "color picker"),
            (vocabulary::HUE, "hue"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_constants(named::colors())
    .with_kinds([
        (
            vocabulary::RGB,
            vec![crate::libraries::isa::vocabulary::BYTES],
        ),
        (
            vocabulary::RGBA,
            vec![crate::libraries::isa::vocabulary::BYTES],
        ),
    ])
    .with_projections_for([
        (vocabulary::RGB, crate::display::runtime_partial(present)),
        (vocabulary::RGBA, crate::display::runtime_partial(present)),
    ])
    // A place a picker is opened on has no color yet, so no kind.
    .with_projection(crate::display::runtime_partial(missing_picker))
    .with_completions(completions)
}

#[cfg(test)]
#[path = "tests/color.rs"]
pub(crate) mod tests;
