//! Render-time Grap interpreter for Puri drawing programs.
//!
//! The program calls the same drawing vocabulary that the encoded
//! `Drawing` form uses, but a scoped FFI layer sends operations straight
//! to the concrete canvas. The only retained production representation is
//! the one render continuation required by the frame's hover/render split.

use super::Cx;
use crate::placed::{Placed, leaf};
use gid::{CellId, Value};
use measured::{Extent, Measured};
use progred_libraries::{absent, layout as layout_data};
use puri::draw::Canvas;
use kurbo::{Affine, BezPath, Circle, Rect};
use peniko::Brush;

#[derive(Clone)]
struct Faces {
    name: Brush,
    string: Brush,
    dim: Brush,
    label: Brush,
    id: Brush,
    accent_wash: Brush,
    ink: Brush,
}

impl Faces {
    fn new(styles: &crate::styles::Styles) -> Self {
        Self {
            name: styles.name.brush.clone(),
            string: styles.string.brush.clone(),
            dim: styles.dim.brush.clone(),
            label: styles.label.brush.clone(),
            id: styles.id.brush.clone(),
            accent_wash: styles.accent_wash.brush.clone(),
            ink: styles.ink.brush.clone(),
        }
    }

    fn resolve(&self, paint: progred_display::Paint) -> Brush {
        match paint {
            progred_display::Paint::Brush(brush) => brush,
            progred_display::Paint::Face(face) => match face {
                progred_display::Face::Name => self.name.clone(),
                progred_display::Face::String => self.string.clone(),
                progred_display::Face::Dim => self.dim.clone(),
                progred_display::Face::Label => self.label.clone(),
                progred_display::Face::Id => self.id.clone(),
                progred_display::Face::AccentWash => self.accent_wash.clone(),
                progred_display::Face::Ink => self.ink.clone(),
            },
        }
    }
}

fn evaluated_field(
    context: &mut grap::Context,
    call: grap::Expression,
    environment: &grap::Environment,
    field: CellId,
) -> Result<Option<Value>, grap::Halt> {
    context
        .field(call, field)
        .map(|value| context.eval(value, environment))
        .transpose()
}

fn number(
    context: &mut grap::Context,
    expression: grap::Expression,
    environment: &grap::Environment,
) -> Result<Option<f64>, grap::Halt> {
    Ok(context
        .eval_runtime(expression, environment)?
        .as_f64(progred_libraries::f64::read)
        .filter(|number| number.is_finite()))
}

/// Shape arguments are raw so coordinates may be ordinary Grap
/// expressions rather than a separately allocated quoted shape value.
fn shape(
    context: &mut grap::Context,
    expression: grap::Expression,
    environment: &grap::Environment,
) -> Result<Option<puri::Shape>, grap::Halt> {
    if let Some(content) = context.field(expression, layout_data::vocabulary::RECT) {
        let (Some(x), Some(y), Some(width), Some(height)) = (
            context.field(content, layout_data::vocabulary::X),
            context.field(content, layout_data::vocabulary::Y),
            context.field(content, layout_data::vocabulary::WIDTH),
            context.field(content, layout_data::vocabulary::HEIGHT),
        ) else {
            return Ok(None);
        };
        let (Some(x), Some(y), Some(width), Some(height)) = (
            number(context, x, environment)?,
            number(context, y, environment)?,
            number(context, width, environment)?,
            number(context, height, environment)?,
        ) else {
            return Ok(None);
        };
        return Ok((width >= 0.0 && height >= 0.0).then(|| {
            puri::Shape::Rect(Rect::new(x, y, x + width, y + height))
        }));
    }
    if let Some(content) = context.field(expression, layout_data::vocabulary::CIRCLE) {
        let (Some(x), Some(y), Some(radius)) = (
            context.field(content, layout_data::vocabulary::X),
            context.field(content, layout_data::vocabulary::Y),
            context.field(content, layout_data::vocabulary::RADIUS),
        ) else {
            return Ok(None);
        };
        let (Some(x), Some(y), Some(radius)) = (
            number(context, x, environment)?,
            number(context, y, environment)?,
            number(context, radius, environment)?,
        ) else {
            return Ok(None);
        };
        return Ok((radius >= 0.0).then(|| {
            puri::Shape::Circle(Circle::new((x, y), radius))
        }));
    }
    if let Some(content) = context.field(expression, layout_data::vocabulary::PATH) {
        let content = context.eval(content, environment)?;
        return Ok(layout_data::read_shape(&Value::record([(
            layout_data::vocabulary::PATH,
            content,
        )])));
    }
    let value = context.eval(expression, environment)?;
    Ok(layout_data::read_shape(&value))
}

/// Like shapes, literal transform operations evaluate their numeric
/// children in the caller's environment without constructing a quoted
/// transform value first.
fn transform(
    context: &mut grap::Context,
    expression: grap::Expression,
    environment: &grap::Environment,
) -> Result<Option<Affine>, grap::Halt> {
    let Some(operations) = context.elements(expression).map(|items| items.to_vec()) else {
        let value = context.eval(expression, environment)?;
        return Ok(layout_data::read_transform(&value));
    };
    let mut transform = Affine::IDENTITY;
    for operation in operations {
        if let Some(point) = context.field(operation, layout_data::vocabulary::TRANSLATE) {
            let (Some(x), Some(y)) = (
                context.field(point, layout_data::vocabulary::X),
                context.field(point, layout_data::vocabulary::Y),
            ) else {
                return Ok(None);
            };
            let (Some(x), Some(y)) = (
                number(context, x, environment)?,
                number(context, y, environment)?,
            ) else {
                return Ok(None);
            };
            transform *= Affine::translate((x, y));
        } else if let Some(angle) =
            context.field(operation, layout_data::vocabulary::ROTATE)
        {
            let Some(angle) = number(context, angle, environment)? else {
                return Ok(None);
            };
            transform *= Affine::rotate(angle);
        } else {
            return Ok(None);
        }
    }
    Ok(Some(transform))
}

pub(super) fn program_leaf<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    width: f64,
    ascent: f64,
    descent: f64,
    fuel: usize,
    program: Value,
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    let extent = Extent {
        width: width * scale,
        ascent: ascent * scale,
        descent: descent * scale,
    };
    let faces = Faces::new(cx.styles);
    let document = cx.sources.doc.cells.clone();
    let library = cx.sources.library.clone();
    let foreign = cx.foreign.clone();
    leaf(extent, move |builder, placement| {
        let outer = Affine::translate((placement.rect.x0, placement.rect.y0))
            * Affine::scale(scale);
        builder.ink(move |canvas: &mut Cv, _| {
            canvas.clip(
                Rect::new(0.0, 0.0, width, ascent + descent),
                outer,
                    |canvas| {
                    let canvas = std::cell::RefCell::new(canvas);
                    let path = std::cell::RefCell::new(BezPath::new());
                    let unit = Value::record([]);
                    let functions = [
                        layout_data::vocabulary::FILL,
                        layout_data::vocabulary::PATH,
                        layout_data::vocabulary::MOVE_TO,
                        layout_data::vocabulary::LINE_TO,
                        layout_data::vocabulary::CLOSE,
                    ];
                    let draw = |function,
                                context: &mut grap::Context<'_>,
                                call,
                                environment: &grap::Environment| {
                        match function {
                            layout_data::vocabulary::PATH => {
                                *path.borrow_mut() = BezPath::new();
                                Ok(unit.clone())
                            }
                            layout_data::vocabulary::MOVE_TO
                            | layout_data::vocabulary::LINE_TO => {
                                let (Some(x), Some(y)) = (
                                    context.field(call, layout_data::vocabulary::X),
                                    context.field(call, layout_data::vocabulary::Y),
                                ) else {
                                    return Ok(absent::value());
                                };
                                let (Some(x), Some(y)) = (
                                    number(context, x, environment)?,
                                    number(context, y, environment)?,
                                ) else {
                                    return Ok(absent::value());
                                };
                                if function == layout_data::vocabulary::MOVE_TO {
                                    path.borrow_mut().move_to((x, y));
                                } else {
                                    path.borrow_mut().line_to((x, y));
                                }
                                Ok(unit.clone())
                            }
                            layout_data::vocabulary::CLOSE => {
                                path.borrow_mut().close_path();
                                Ok(unit.clone())
                            }
                            layout_data::vocabulary::FILL => {
                                let Some(paint) = evaluated_field(
                                    context,
                                    call,
                                    environment,
                                    layout_data::vocabulary::PAINT,
                                )?
                                else {
                                    return Ok(context
                                        .missing_argument(layout_data::vocabulary::PAINT));
                                };
                                let shape = match context
                                    .field(call, layout_data::vocabulary::SHAPE)
                                {
                                    Some(expression) => shape(context, expression, environment)?,
                                    None => Some(puri::Shape::Path(path.borrow().clone())),
                                };
                                let transform = match context
                                    .field(call, layout_data::vocabulary::TRANSFORM)
                                {
                                    Some(expression) => {
                                        transform(context, expression, environment)?
                                    }
                                    None => Some(Affine::IDENTITY),
                                };
                                let (Some(shape), Some(paint), Some(transform)) =
                                    (shape, layout_data::read_paint(&paint), transform)
                                else {
                                    return Ok(absent::value());
                                };
                                canvas.borrow_mut().fill(
                                    shape,
                                    faces.resolve(paint),
                                    outer * transform,
                                );
                                Ok(unit.clone())
                            }
                            _ => unreachable!("the overlay only advertises drawing functions"),
                        }
                    };
                    let overlay = grap::ForeignOverlay::new(&functions, &draw);
                    let evaluation = grap::evaluate_scoped(
                        &program,
                        |cell| {
                            document
                                .value(cell)
                                .or_else(|| library.value(cell))
                                .cloned()
                        },
                        &foreign,
                        &overlay,
                        fuel,
                    );
                    debug_assert!(
                        evaluation.diagnostics.is_empty(),
                        "drawing program diagnostics: {:?}",
                        evaluation.diagnostics,
                    );
                },
            );
        });
    })
}
