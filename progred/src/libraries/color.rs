//! Open sRGB RGB8 and RGBA8 conventions with an editable projected spelling.

use crate::libraries::{Library, f64, line_edit, name, text};
use gid::{Cells, Step, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x25d0e2034b4bd65bebb4811d65eab89c);
use crate::display::{
    Layout, PointEvent, PointUpdate, ProjectionInput, TextFamily, centered_row, col, descend,
    on_activate, on_hover, on_point, popover, widget,
};
use ::grap::{ForeignFunction, ForeignFunctions};
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

#[cfg(test)]
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
    fields
        .get(&vocabulary::RGBA)
        .and_then(Value::as_blob)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(Encoded::Rgba)
        .or_else(|| {
            fields
                .get(&vocabulary::RGB)
                .and_then(Value::as_blob)
                .and_then(|bytes| <[u8; 3]>::try_from(bytes).ok())
                .map(Encoded::Rgb)
        })
}

pub fn read(value: &Value) -> Option<Color> {
    Some(match encoded(value)? {
        Encoded::Rgb([red, green, blue]) => Color::from_rgba8(red, green, blue, 0xff),
        Encoded::Rgba([red, green, blue, alpha]) => Color::from_rgba8(red, green, blue, alpha),
    })
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

fn picker_leaf(
    height: f64,
    color: Hsva,
    draw: fn(Hsva, &mut dyn CanvasSink, Affine),
) -> Layout<crate::Editor, crate::frame::Hovered> {
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

fn picker(
    original: &Value,
    encoded: Encoded,
    hue: f64,
) -> Layout<crate::Editor, crate::frame::Hovered> {
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
        ForeignFunction::new(|context, call, environment| {
            let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
            };
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context.eval(current, environment)?;
            let input = context.eval(input, environment)?;
            Ok(text::read(&input)
                .and_then(|spelling| edit(spelling, Some(&current)))
                .unwrap_or_else(crate::libraries::absent::value))
        }),
    )
}

pub fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    replace_color(current?, parse(spelling)?)
}

fn swatch(color: Color) -> Layout<crate::Editor, crate::frame::Hovered> {
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

pub fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let encoded = encoded(input.value?)?;
    let color = read(input.value?)?;
    let initial_hue = hsva(encoded).hue;
    let selected_hue = picker_hue(input.selection);
    let next_selection = if selected_hue.is_some() {
        without_picker(input.selection)
    } else {
        picker_selection(initial_hue)
    };
    let target = input.targets.current();
    let swatch = if input.writable {
        let select_picker = target.select_with;
        on_activate(
            swatch(color),
            target.hover.clone(),
            Rc::new(move |world| select_picker(world, next_selection.clone())),
        )
    } else {
        swatch(color)
    };
    let swatch = on_hover(swatch, target.hover);
    let swatch = if input.writable
        && let Some(hue) = selected_hue
    {
        popover(swatch, picker(input.value?, encoded, hue))
    } else {
        swatch
    };
    let name =
        name::read(input.value?).map(|_| descend(Step::Key(name::vocabulary::NAME), None, None));
    let spelling = line_edit::layout_with_family(
        spelling(encoded),
        line_edit::native(edit),
        "#",
        "",
        TextFamily::Monospace,
    );
    Some(centered_row(
        4.0,
        std::iter::once(swatch)
            .chain(name)
            .chain(std::iter::once(spelling)),
    ))
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::RGB, name::record("rgb", []));
    cells.set_value(vocabulary::RGBA, name::record("rgba", []));
    cells.set_value(vocabulary::UPDATE, name::record("color update", []));
    cells.set_value(vocabulary::PICKER, name::record("color picker", []));
    cells.set_value(vocabulary::HUE, name::record("hue", []));
    named::insert(&mut cells);
    Library::named(
        ID,
        "color",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::partial(display),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recordable, Recorded};

    use crate::display::test_support::{ProjectionCall, inspect};
    use crate::display::{Env, RowAlignment};
    use gid::new_cell_id;

    #[test]
    fn swatch_keeps_its_metrics_fill_and_inset_border() {
        let color = Color::from_rgba8(0xb4, 0xe0, 0xfe, 0x99);
        let (extent, drawing) = crate::libraries::test_widgets::paint(&swatch(color));
        assert_eq!(
            extent,
            widget::Extent {
                width: 15.0,
                ascent: 11.0,
                descent: 4.0
            }
        );
        let [
            puri::DrawCmd::Fill {
                shape: puri::Shape::RoundedRect(fill),
                brush,
                ..
            },
            puri::DrawCmd::Stroke {
                shape: puri::Shape::RoundedRect(border),
                style,
                ..
            },
        ] = drawing.0.as_slice()
        else {
            panic!("fill followed by border");
        };
        assert_eq!(fill, border);
        assert_eq!(
            *fill,
            RoundedRect::from_rect(Rect::new(0.5, 0.5, 14.5, 14.5), 2.5)
        );
        assert_eq!(*brush, color.into());
        assert_eq!(style.width, 1.0);
    }

    #[test]
    fn rgb_and_rgba_are_open_library_data() {
        let color = Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff);
        let extra = new_cell_id();
        let enriched = Value::record(
            value(color)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"metadata".to_vec())),
        );

        assert_eq!(
            value(color).as_record().unwrap().get(&vocabulary::RGB),
            Some(&Value::from(vec![0xb4, 0xe0, 0xfe])),
        );
        assert_eq!(read(&enriched), Some(color));
        assert_eq!(
            value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0x99))
                .as_record()
                .unwrap()
                .get(&vocabulary::RGBA),
            Some(&Value::from(vec![0xb4, 0xe0, 0xfe, 0x99])),
        );
    }

    #[test]
    fn spelling_round_trips_through_the_update_function() {
        let extra = new_cell_id();
        let current = Value::record(
            value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff))
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"metadata".to_vec())),
        );
        let update = |input: &str| {
            crate::libraries::test_evaluate(
                &::grap::call(
                    ::grap::ffi(vocabulary::UPDATE),
                    [
                        (line_edit::vocabulary::CURRENT, current.clone()),
                        (line_edit::vocabulary::INPUT, text::value(input)),
                    ],
                ),
                |_| None,
                &functions(),
                100,
            )
            .result
        };

        let updated = update("ebb4cc99");
        assert_eq!(
            read(&updated),
            Some(Color::from_rgba8(0xeb, 0xb4, 0xcc, 0x99)),
        );
        assert_eq!(
            updated.as_record().unwrap().get(&extra),
            Some(&Value::from(b"metadata".to_vec())),
        );
        assert!(!updated.as_record().unwrap().contains_key(&vocabulary::RGB));
        assert_eq!(
            updated.as_record().unwrap().get(&vocabulary::RGBA),
            Some(&Value::from(vec![0xeb, 0xb4, 0xcc, 0x99])),
        );
        assert_eq!(
            read(&update("b4e")),
            Some(Color::from_rgba8(0xbb, 0x44, 0xee, 0xff)),
        );
        assert_eq!(
            read(&update("b4e8")),
            Some(Color::from_rgba8(0xbb, 0x44, 0xee, 0x88)),
        );
        assert!(crate::libraries::absent::is_absent(&update(
            "rebeccapurple"
        )));
    }

    #[test]
    fn projection_is_a_swatch_and_editable_spelling() {
        let color = value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff));
        let target = |_| crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_: &mut crate::Editor| false),
            select_with: std::rc::Rc::new(|_: &mut crate::Editor, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        };
        let layout = display(&ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env: &NoEval,
            value: Some(&color),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&target),
        })
        .expect("color projection");
        let Recorded::Row {
            alignment,
            children,
            ..
        } = layout.record()
        else {
            panic!("color projection is one row")
        };

        assert!(matches!(alignment, RowAlignment::Center));
        assert!(matches!(
            children[0],
            Recorded::Before { ref child, .. }
                if matches!(child.as_ref(), Recorded::Before { child, .. }
                    if matches!(child.as_ref(), Recorded::Widget(_)))
        ));
        assert!(matches!(
            crate::libraries::test_widgets::line(&children[1]),
            Some(line)
                if line.text == "b4e0fe"
                    && line.prefix == "#"
                    && line.suffix.is_empty()
                    && line.family == TextFamily::Monospace
        ));
    }

    #[test]
    fn a_read_only_color_has_no_picker_activation_or_popup() {
        let color = value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff));
        let selection = picker_selection(0.1);
        let target = |_| crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_: &mut crate::Editor| false),
            select_with: std::rc::Rc::new(|_: &mut crate::Editor, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        };
        let layout = display(&ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env: &NoEval,
            value: Some(&color),
            scale_factor: 1.0,
            writable: false,
            selection: Some(&selection),
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&target),
        })
        .expect("read-only color projection");
        let Recorded::Row { children, .. } = layout.record() else {
            panic!("color projection is one row")
        };

        assert!(matches!(
            children[0],
            Recorded::Before { ref child, .. }
                if matches!(child.as_ref(), Recorded::Widget(_))
        ));
    }

    #[test]
    fn a_named_color_projects_its_editable_name_and_hex() {
        let color = name::record(
            "rebeccapurple",
            [(vocabulary::RGB, Value::from(vec![0x66, 0x33, 0x99]))],
        );
        let target = |_| crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_: &mut crate::Editor| false),
            select_with: std::rc::Rc::new(|_: &mut crate::Editor, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        };
        let layout = display(&ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env: &NoEval,
            value: Some(&color),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&target),
        })
        .expect("named color projection");
        let Recorded::Row { children, .. } = layout.record() else {
            panic!("named color projection is one row")
        };

        assert!(matches!(&inspect(&(children[1])),
            ProjectionCall::Descend {
                step: Step::Key(field),
                ..
            } if *field == name::vocabulary::NAME
        ));
        assert!(matches!(
            crate::libraries::test_widgets::line(&children[2]),
            Some(line) if line.text == "663399"
        ));
    }

    #[test]
    fn picker_mode_floats_point_controls_and_preserves_open_metadata() {
        let extra = new_cell_id();
        let color = Value::record(
            value(Color::from_rgba8(0x66, 0x33, 0x99, 0xff))
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(vec![7])),
        );
        let selection = picker_selection(0.1);
        let target = |_| crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_: &mut crate::Editor| false),
            select_with: std::rc::Rc::new(|_: &mut crate::Editor, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        };
        let layout = display(&ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env: &NoEval,
            value: Some(&color),
            scale_factor: 1.0,
            writable: true,
            selection: Some(&selection),
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&target),
        })
        .expect("selected color projection");
        let Recorded::Row { children, .. } = layout.record() else {
            panic!("color projection is one row")
        };
        let Recorded::Floating { .. } = &children[0] else {
            panic!("picker mode floats from the swatch")
        };
        let Recorded::Col { children, .. } = picker(&color, encoded(&color).unwrap(), 0.1).record()
        else {
            panic!("picker controls are stacked")
        };
        let updated = crate::libraries::test_widgets::point_update(
            &children[0],
            PointEvent { x: 1.0, y: 0.0 },
        );

        assert_eq!(
            updated.value.as_record().unwrap().get(&extra),
            Some(&Value::from(vec![7]))
        );
        assert!(matches!(encoded(&updated.value), Some(Encoded::Rgb(_))));
        assert!(updated.selection.is_none());

        let updated = crate::libraries::test_widgets::point_update(
            &children[1],
            PointEvent { x: 0.25, y: 0.0 },
        );
        assert_eq!(picker_hue(updated.selection.as_ref()), Some(0.25));
    }

    #[test]
    fn closing_the_picker_removes_only_its_selection_state() {
        let other = new_cell_id();
        let mut selection = picker_selection(0.25).as_record().unwrap().clone();
        selection.insert(other, Value::from(vec![7]));
        let closed = without_picker(Some(&Value::Record(selection)));

        assert_eq!(picker_hue(Some(&closed)), None);
        assert_eq!(
            closed.as_record().unwrap().get(&other),
            Some(&Value::from(vec![7]))
        );
    }

    struct NoEval;

    impl Env for NoEval {
        fn apply_scoped(
            &self,
            _: &gid::Value,
            _: &[(gid::CellId, gid::Value)],
            _scope: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("color projection does not evaluate while projecting")
        }
    }
}
