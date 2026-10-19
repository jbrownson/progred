use super::{Error, argument, fuel, invalid, number, result_layout, run, vocabulary::*};
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::widget;
use crate::libraries::{layout, presentation};
use ::grap::{Context, Environment, Expression, RuntimeValue};
use cam::paths::{Axis, InvalidPath, MapPoints, Point3, Sink};
use kurbo::{Affine, BezPath, Point, Rect, Size, Stroke};
use peniko::Color;
use puri::draw::Canvas;
use std::rc::Rc;

fn isometric([x, y, z]: Point3) -> Point {
    Point::new(
        x / 2.0_f64.sqrt() - y / 2.0_f64.sqrt(),
        (x / 6.0_f64.sqrt() + y / 6.0_f64.sqrt()) - z * (2.0_f64 / 3.0).sqrt(),
    )
}

#[derive(Default)]
struct Lines2D {
    path: BezPath,
    bounds: Option<Rect>,
    started: bool,
}

impl Lines2D {
    fn point(&mut self, [x, y, _]: Point3) -> Result<Point, InvalidPath> {
        let point = Point::new(x, y);
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(InvalidPath::NonFinitePoint);
        }
        self.bounds = Some(
            self.bounds
                .map_or(Rect::from_points(point, point), |bounds| {
                    bounds.union_pt(point)
                }),
        );
        Ok(point)
    }
}

impl Sink for Lines2D {
    type Error = InvalidPath;
    fn end_path(&mut self) {
        self.started = false;
    }
    fn start_at(&mut self, point: Point3, _: Axis) -> Result<(), Self::Error> {
        let point = self.point(point)?;
        self.path.move_to(point);
        self.started = true;
        Ok(())
    }
    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        if !self.started {
            return Err(InvalidPath::MissingStart);
        }
        let point = self.point(point)?;
        self.path.line_to(point);
        Ok(())
    }
}

fn projected() -> MapPoints<Lines2D, impl FnMut(Point3) -> Result<Point3, InvalidPath>> {
    MapPoints {
        sink: Lines2D::default(),
        map: |point| {
            let point = isometric(point);
            Ok([point.x, point.y, 0.0])
        },
    }
}

fn fitted(lines: Lines2D, size: Size) -> Option<BezPath> {
    let Lines2D {
        mut path, bounds, ..
    } = lines;
    if let Some(bounds) = bounds {
        if !bounds.width().is_finite() || !bounds.height().is_finite() {
            return None;
        }
        let padding = 16.0_f64.min(size.width / 4.0).min(size.height / 4.0);
        let scale = ((size.width - 2.0 * padding) / bounds.width())
            .min((size.height - 2.0 * padding) / bounds.height());
        let scale = if scale.is_finite() { scale } else { 1.0 };
        let center = (
            bounds.x0 / 2.0 + bounds.x1 / 2.0,
            bounds.y0 / 2.0 + bounds.y1 / 2.0,
        );
        path.apply_affine(
            Affine::translate((size.width / 2.0, size.height / 2.0))
                * Affine::scale(scale)
                * Affine::translate((-center.0, -center.1)),
        );
    }
    Some(path)
}

/// The 2D preview's value: the program, and the size and fuel to draw it with.
pub(super) fn preview(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Error> {
    let program = argument(context, call, presentation::vocabulary::VALUE)?;
    let program = context.eval(program, environment)?;
    let width = number(context, call, environment, layout::vocabulary::WIDTH)?;
    let height = number(context, call, environment, layout::vocabulary::HEIGHT)?;
    if width <= 0.0 || height <= 0.0 {
        Err(invalid())
    } else {
        let fuel = fuel(context, call, environment)?;
        Ok(RuntimeValue::record([(
            PREVIEW,
            RuntimeValue::record([
                (PROGRAM, program),
                (layout::vocabulary::WIDTH, RuntimeValue::f64(width)),
                (layout::vocabulary::HEIGHT, RuntimeValue::f64(height)),
                (layout::vocabulary::FUEL, RuntimeValue::f64(fuel as f64)),
            ]),
        )]))
    }
}

pub(super) fn display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let fields = input.value?.field(PREVIEW)?;
    let program = fields.field(PROGRAM)?;
    let width = fields.field(layout::vocabulary::WIDTH)?.as_f64()?;
    let height = fields.field(layout::vocabulary::HEIGHT)?.as_f64()?;
    let fuel = ::grap::f64::to_usize(fields.field(layout::vocabulary::FUEL)?.as_f64()?)?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let size = Size::new(width, height);
    Some(Layout::program(Rc::new(move |context, build| {
        let mut lines = projected();
        let evaluation = run(&mut lines, |scope| {
            ::grap::apply_expression(&program, [], context.viewing().host(), Some(scope), fuel)
        });
        if !evaluation.completed || evaluation.result.absent_reason().is_some() {
            result_layout(&evaluation.result).measure(context, build)
        } else if let Some(path) = fitted(lines.sink, size) {
            let scale = context.viewing().scale();
            measured::choices::ChoiceLayout::fixed(widget::paint(
                widget::Extent {
                    width: width * scale,
                    ascent: height * scale / 2.0,
                    descent: height * scale / 2.0,
                },
                move |canvas, placement| {
                    let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                        * Affine::scale(scale);
                    canvas.fill(
                        Rect::from_origin_size(Point::ZERO, size),
                        Color::WHITE,
                        transform,
                    );
                    canvas.stroke(
                        path,
                        Stroke::new(1.25),
                        Color::new([0.08, 0.48, 0.32, 1.0]),
                        transform,
                    );
                },
            ))
        } else {
            result_layout(::grap::absent::value(INVALID_INPUT)).measure(context, build)
        }
    })))
}

#[cfg(test)]
mod tests;
