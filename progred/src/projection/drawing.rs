//! Grap drawing programs in the layout library's vocabulary: `fill`, and
//! paths built with `path`, `move to`, `line to` and `close`. They draw into
//! the toolkit's recorded leaf, which records a visible program once per
//! frame for both hit-testing and painting.

use crate::display::widget::recorded::{Record, Recorder};
use crate::libraries::layout as layout_data;
use gid::{CellId, Step, Value};
use kurbo::{Affine, BezPath, Circle, Rect};
use std::cell::RefCell;
use std::rc::Rc;

/// A stored drawing's program, drawn into a recorded leaf `extent` logical
/// units big.
pub(crate) fn program(
    extent: measured::Extent,
    fuel: usize,
    program: grap::RuntimeValue,
) -> crate::view::Layout {
    crate::display::widget::recorded::recorded(
        extent,
        vec![Step::Key(layout_data::vocabulary::PROGRAM)],
        record(program, fuel),
    )
}

/// Runs `program` with the drawing functions installed, filling into the
/// recorder.
fn record(program: grap::RuntimeValue, fuel: usize) -> Record {
    Rc::new(move |host, recorder: &mut Recorder| {
        let recorder = RefCell::new(recorder);
        let path = RefCell::new(BezPath::new());
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
                    call: &grap::Expression,
                    environment: &grap::Environment| {
            match function {
                layout_data::vocabulary::PATH => Ok(context.effect(|| {
                    *path.borrow_mut() = BezPath::new();
                    unit.clone()
                })),
                layout_data::vocabulary::MOVE_TO | layout_data::vocabulary::LINE_TO => {
                    let Some(x) = context.field(call, layout_data::vocabulary::X) else {
                        return Ok(context.missing_argument(layout_data::vocabulary::X));
                    };
                    let Some(y) = context.field(call, layout_data::vocabulary::Y) else {
                        return Ok(context.missing_argument(layout_data::vocabulary::Y));
                    };
                    let (Some(x), Some(y)) = (
                        number(context, x, environment)?,
                        number(context, y, environment)?,
                    ) else {
                        return Ok(::grap::absent::with_detail(
                            layout_data::vocabulary::INVALID_DRAWING,
                            ::grap::absent::VALUE,
                            context.value(call).clone(),
                        ));
                    };
                    Ok(context.effect(|| {
                        if function == layout_data::vocabulary::MOVE_TO {
                            path.borrow_mut().move_to((x, y));
                        } else {
                            path.borrow_mut().line_to((x, y));
                        }
                        unit.clone()
                    }))
                }
                layout_data::vocabulary::CLOSE => Ok(context.effect(|| {
                    path.borrow_mut().close_path();
                    unit.clone()
                })),
                layout_data::vocabulary::FILL => {
                    let Some(paint) = evaluated_field(
                        context,
                        call,
                        environment,
                        layout_data::vocabulary::PAINT,
                    )?
                    else {
                        return Ok(context.missing_argument(layout_data::vocabulary::PAINT));
                    };
                    let shape = match context.field(call, layout_data::vocabulary::SHAPE) {
                        Some(shape) => evaluated_shape(&context.eval(shape, environment)?),
                        None => Some(puri::Shape::Path(path.borrow().clone())),
                    };
                    let transform = match context.field(call, layout_data::vocabulary::TRANSFORM) {
                        Some(transform) => {
                            evaluated_transform(&context.eval(transform, environment)?)
                        }
                        None => Some(Affine::IDENTITY),
                    };
                    let (Some(shape), Some(paint), Some(transform)) =
                        (shape, layout_data::read_paint(&paint), transform)
                    else {
                        return Ok(::grap::absent::with_detail(
                            layout_data::vocabulary::INVALID_DRAWING,
                            ::grap::absent::VALUE,
                            context.value(call).clone(),
                        ));
                    };
                    let calls = context.call_trace();
                    Ok(context.effect(|| {
                        recorder.borrow_mut().fill(shape, paint, transform, calls);
                        unit.clone()
                    }))
                }
                _ => unreachable!("the overlay only advertises drawing functions"),
            }
        };
        let overlay = grap::ForeignOverlay::from_value(&functions, &draw);
        let evaluation = grap::evaluate_runtime_scoped(&program, host, &overlay, fuel);
        evaluation.completed && !evaluation.result.declines()
    })
}

fn evaluated_field(
    context: &mut grap::Context,
    call: &grap::Expression,
    environment: &grap::Environment,
    field: CellId,
) -> Result<Option<Value>, grap::Halt> {
    context
        .field(call, field)
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
/// coordinate a round trip through stored bytes; others read as stored data.
fn evaluated_shape(value: &grap::RuntimeValue) -> Option<puri::Shape> {
    use layout_data::vocabulary::{CIRCLE, HEIGHT, RADIUS, RECT, ROUNDED_RECT, WIDTH, X, Y};
    if let Some(rect) = value.field(RECT) {
        let (x, y, width, height) = (
            coordinate(&rect, X)?,
            coordinate(&rect, Y)?,
            coordinate(&rect, WIDTH)?,
            coordinate(&rect, HEIGHT)?,
        );
        return (width >= 0.0 && height >= 0.0)
            .then(|| puri::Shape::Rect(Rect::new(x, y, x + width, y + height)));
    }
    if !value.contains_field(ROUNDED_RECT)
        && let Some(circle) = value.field(CIRCLE)
    {
        let (x, y, radius) = (
            coordinate(&circle, X)?,
            coordinate(&circle, Y)?,
            coordinate(&circle, RADIUS)?,
        );
        return (radius >= 0.0).then(|| puri::Shape::Circle(Circle::new((x, y), radius)));
    }
    layout_data::read_shape(&value.to_value())
}

fn evaluated_transform(value: &grap::RuntimeValue) -> Option<Affine> {
    use layout_data::vocabulary::{ROTATE, TRANSLATE, X, Y};
    let Some(mut operations) = value.list_values() else {
        return layout_data::read_transform(&value.to_value());
    };
    operations.try_fold(Affine::IDENTITY, |transform, operation| {
        Some(match operation.field(TRANSLATE) {
            Some(point) => {
                transform * Affine::translate((coordinate(&point, X)?, coordinate(&point, Y)?))
            }
            None => {
                let angle = operation
                    .field(ROTATE)?
                    .as_f64()
                    .filter(|n| n.is_finite())?;
                transform * Affine::rotate(angle)
            }
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
mod tests;
