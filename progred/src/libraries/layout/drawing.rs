//! Drawing programs: Grap code that fills shapes with `fill`, building paths
//! with `path`, `move to`, `line to` and `close`. They draw into the toolkit's
//! recorded leaf, which records a visible program once per frame for both
//! hit-testing and painting.

use super::{read_paint, read_shape, vocabulary};
use crate::display::Host;
use crate::display::widget::recorded::{Record, Recorder};
use gid::{CellId, Step, Value};
use kurbo::{Affine, BezPath, Circle, Rect};
use std::cell::RefCell;
use std::rc::Rc;

/// A drawing's program, drawn into a recorded leaf `extent` logical units
/// big. `stored` is where the program is stored below the occurrence, if it
/// is, so the fills it draws link to the calls that drew them there.
pub(super) fn program<W: Host>(
    extent: measured::Extent,
    fuel: usize,
    stored: Option<Vec<Step>>,
    program: grap::RuntimeValue,
) -> crate::display::host::Layout<W> {
    crate::display::widget::recorded::recorded(extent, stored, record(program, fuel))
}

/// Runs `program` with the drawing functions installed, filling into the
/// recorder, each fill with the calls it was drawn under.
fn record(program: grap::RuntimeValue, fuel: usize) -> Record {
    Rc::new(move |host, recorder: &mut Recorder| {
        let recorder = RefCell::new(recorder);
        let tracer = grap::Tracer::default();
        let path = RefCell::new(BezPath::new());
        let unit = Value::record([]);
        let functions = [
            vocabulary::FILL,
            vocabulary::PATH,
            vocabulary::MOVE_TO,
            vocabulary::LINE_TO,
            vocabulary::CLOSE,
        ];
        let draw = |function,
                    context: &mut grap::Context<'_>,
                    call: &grap::Expression,
                    environment: &grap::Environment| {
            match function {
                vocabulary::PATH => Ok(context.effect(|| {
                    *path.borrow_mut() = BezPath::new();
                    unit.clone()
                })),
                vocabulary::MOVE_TO | vocabulary::LINE_TO => {
                    let Some(x) = call.field(vocabulary::X) else {
                        return Ok(call.missing_argument(vocabulary::X));
                    };
                    let Some(y) = call.field(vocabulary::Y) else {
                        return Ok(call.missing_argument(vocabulary::Y));
                    };
                    Ok(
                        match (
                            number(context, x, environment)?,
                            number(context, y, environment)?,
                        ) {
                            (Some(x), Some(y)) => context.effect(|| {
                                if function == vocabulary::MOVE_TO {
                                    path.borrow_mut().move_to((x, y));
                                } else {
                                    path.borrow_mut().line_to((x, y));
                                }
                                unit.clone()
                            }),
                            _ => invalid_drawing(call),
                        },
                    )
                }
                vocabulary::CLOSE => Ok(context.effect(|| {
                    path.borrow_mut().close_path();
                    unit.clone()
                })),
                vocabulary::FILL => {
                    let Some(paint) =
                        evaluated_field(context, call, environment, vocabulary::PAINT)?
                    else {
                        return Ok(call.missing_argument(vocabulary::PAINT));
                    };
                    let shape = match call.field(vocabulary::SHAPE) {
                        Some(shape) => evaluated_shape(&context.eval(shape, environment)?),
                        None => Some(puri::draw::Shape::Path(path.borrow().clone())),
                    };
                    let transform = match call.field(vocabulary::TRANSFORM) {
                        Some(transform) => {
                            evaluated_transform(&context.eval(transform, environment)?)
                        }
                        None => Some(Affine::IDENTITY),
                    };
                    Ok(match (shape, read_paint(&paint), transform) {
                        (Some(shape), Some(paint), Some(transform)) => {
                            let calls = tracer.trace();
                            context.effect(|| {
                                recorder.borrow_mut().fill(shape, paint, transform, calls);
                                unit.clone()
                            })
                        }
                        _ => invalid_drawing(call),
                    })
                }
                _ => unreachable!("the overlay only advertises drawing functions"),
            }
        };
        let overlay = grap::ForeignOverlay::from_value(&functions, &draw);
        let evaluation = grap::evaluate_runtime_scoped(
            &program,
            &grap::Observed::new(host, &tracer),
            &overlay,
            fuel,
        );
        evaluation.halt.is_none() && !evaluation.result.declines()
    })
}

/// An absent naming `call`, a drawing call with an argument it can't read.
fn invalid_drawing(call: &grap::Expression) -> Value {
    ::grap::absent::with_detail(
        vocabulary::INVALID_DRAWING,
        ::grap::absent::VALUE,
        call.value().clone(),
    )
}

fn evaluated_field(
    context: &mut grap::Context,
    call: &grap::Expression,
    environment: &grap::Environment,
    field: CellId,
) -> Result<Option<Value>, grap::Halt> {
    call.field(field)
        .map(|value| context.eval_to_value(value, environment))
        .transpose()
}

fn number(
    context: &mut grap::Context,
    expression: grap::Expression,
    environment: &grap::Environment,
) -> Result<Option<f64>, grap::Halt> {
    Ok(context
        .eval_f64(expression, environment)?
        .filter(|number| number.is_finite()))
}

/// The common shapes read straight from evaluated values, sparing each
/// coordinate a round trip through stored bytes; others read as stored data,
/// as does a circle carrying a rounded rect, which `read_shape` takes first.
fn evaluated_shape(value: &grap::RuntimeValue) -> Option<puri::draw::Shape> {
    use vocabulary::{CIRCLE, HEIGHT, RADIUS, RECT, ROUNDED_RECT, WIDTH, X, Y};
    if let Some(rect) = value.field(RECT) {
        let (x, y, width, height) = (
            coordinate(&rect, X)?,
            coordinate(&rect, Y)?,
            coordinate(&rect, WIDTH)?,
            coordinate(&rect, HEIGHT)?,
        );
        (width >= 0.0 && height >= 0.0)
            .then(|| puri::draw::Shape::Rect(Rect::new(x, y, x + width, y + height)))
    } else if !value.contains_field(ROUNDED_RECT)
        && let Some(circle) = value.field(CIRCLE)
    {
        let (x, y, radius) = (
            coordinate(&circle, X)?,
            coordinate(&circle, Y)?,
            coordinate(&circle, RADIUS)?,
        );
        (radius >= 0.0).then(|| puri::draw::Shape::Circle(Circle::new((x, y), radius)))
    } else {
        read_shape(&value.to_value())
    }
}

fn evaluated_transform(value: &grap::RuntimeValue) -> Option<Affine> {
    use vocabulary::{ROTATE, TRANSLATE, X, Y};
    value
        .list_values()?
        .try_fold(Affine::IDENTITY, |transform, operation| {
            Some(match operation.field(TRANSLATE) {
                Some(point) => {
                    transform * Affine::translate((coordinate(&point, X)?, coordinate(&point, Y)?))
                }
                None => transform * Affine::rotate(coordinate(&operation, ROTATE)?),
            })
        })
}

fn coordinate(value: &grap::RuntimeValue, key: CellId) -> Option<f64> {
    value
        .field(key)?
        .as_f64()
        .filter(|number| number.is_finite())
}

#[cfg(test)]
#[path = "tests/drawing.rs"]
mod tests;
