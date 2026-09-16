use super::paths::Axis;
use super::{
    paths::{InvalidPath, MapPoints, Point3, Sink},
    run,
    vocabulary::*,
};
use crate::display::{Layout, ProjectionInput, widget};
use crate::libraries::{absent, f64, layout};
use puri::draw::Canvas;
use puri::{Affine, BezPath, Color, Point, Rect, Size, Stroke};
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

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?.get(&PREVIEW)?.as_record()?;
    let program = fields.get(&PROGRAM)?.clone();
    let width = f64::read(fields.get(&layout::vocabulary::WIDTH)?)?;
    let height = f64::read(fields.get(&layout::vocabulary::HEIGHT)?)?;
    let fuel = super::read_fuel(f64::read(fields.get(&layout::vocabulary::FUEL)?)?)?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let size = Size::new(width, height);
    Some(Layout::program(Rc::new(move |context, build| {
        let mut lines = projected();
        let evaluation = run(&mut lines, |scope| {
            ::grap::apply_scoped(&program, [], &context.inputs.sources, scope, fuel)
        });
        if !evaluation.completed || absent::is_absent(&evaluation.result) {
            return crate::display::at(
                [gid::Step::Key(
                    crate::libraries::presentation::vocabulary::RESULT,
                )],
                &evaluation.result,
            )
            .measure(context, build);
        }
        let Some(path) = fitted(lines.sink, size) else {
            return crate::display::at(
                [gid::Step::Key(
                    crate::libraries::presentation::vocabulary::RESULT,
                )],
                &absent::with_reason(INVALID_INPUT),
            )
            .measure(context, build);
        };
        let scale = context.inputs.styles.scale;
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
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    #[test]
    fn fits_extents_and_preserves_disconnected_passes() {
        let mut lines = projected();
        lines.start_at([0.0, 0.0, 0.0], Axis::Z).unwrap();
        lines.line_to([1.0, 1.0, 0.0]).unwrap();
        lines.start_at([1.0, 0.0, 0.0], Axis::Z).unwrap();
        lines.line_to([0.0, 1.0, 0.0]).unwrap();
        let path = fitted(lines.sink, Size::new(600.0, 300.0)).unwrap();
        assert_eq!(
            path.elements()
                .iter()
                .filter(|p| matches!(p, kurbo::PathEl::MoveTo(_)))
                .count(),
            2
        );
        let bounds = path.bounding_box();
        assert!(bounds.x0 >= 15.99 && bounds.y0 >= 15.99);
        assert!(bounds.x1 <= 584.01 && bounds.y1 <= 284.01);
    }
}
