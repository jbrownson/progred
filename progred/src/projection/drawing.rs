//! Grap drawing programs. Each frame records a visible program once,
//! sharing its leaf-local Puri commands between hit-testing and painting.

use super::Cx;
use crate::frame::Hovered;
use crate::hover::{Hover, SourceCalls, SourceTrace};
use crate::libraries::layout as layout_data;
use crate::placed::{HoverPass, leaf};
use crate::sources::Sources;
use gid::{CellId, Step, Value};
use kurbo::{Affine, BezPath, Circle, Point, Rect, Shape as _};
use measured::{Extent, Measured};
use peniko::Brush;
use puri::draw::{Canvas, DrawList};
use std::cell::{LazyCell, RefCell};
use std::rc::Rc;

/// Drawings record after the frame's style borrow ends, so they keep a copy.
struct Faces(crate::styles::Styles);

impl Faces {
    fn new(styles: &crate::styles::Styles) -> Self {
        Self(styles.clone())
    }

    fn resolve(&self, paint: crate::display::Paint) -> Brush {
        match paint {
            crate::display::Paint::Brush(brush) => brush,
            crate::display::Paint::Face(face) => {
                crate::display::widget::style::face_style(&self.0, face)
                    .brush
                    .clone()
            }
        }
    }
}

struct Recorded {
    commands: DrawList,
    hits: Vec<Hit>,
}

struct Hit {
    shape: puri::Shape,
    transform: Affine,
    inverse: Affine,
    bounds: Rect,
    source: HitSource,
}

enum HitSource {
    Exact(SourceTrace),
    Calls(SourceCalls),
}

impl From<SourceTrace> for HitSource {
    fn from(source: SourceTrace) -> Self {
        Self::Exact(source)
    }
}

impl From<SourceCalls> for HitSource {
    fn from(source: SourceCalls) -> Self {
        Self::Calls(source)
    }
}

impl Hit {
    fn new(shape: puri::Shape, transform: Affine, source: impl Into<HitSource>) -> Self {
        let bounds = transform.transform_rect_bbox(shape_bounds(&shape));
        Self {
            shape,
            transform,
            inverse: transform.inverse(),
            bounds,
            source: source.into(),
        }
    }

    fn contains(&self, point: Point) -> bool {
        self.bounds.contains(point) && shape_contains(&self.shape, self.inverse * point)
    }
}

impl Recorded {
    fn target_at(&self, point: Point, outer: Affine) -> Option<Hovered> {
        let point = outer.inverse() * point;
        self.hits
            .iter()
            .rev()
            .find(|hit| hit.contains(point))
            .map(|hit| {
                Hovered::Tree(match &hit.source {
                    HitSource::Calls(calls) => Hover::Calls(calls.clone()),
                    HitSource::Exact(source) => Hover::Source(source.clone()),
                })
            })
    }

    fn highlight<C: Canvas + ?Sized>(
        &self,
        canvas: &mut C,
        outer: Affine,
        source: &SourceTrace,
        brush: &Brush,
    ) {
        for hit in self.hits.iter().filter(|hit| match &hit.source {
            HitSource::Calls(calls) => calls.contains(source),
            HitSource::Exact(hit) => hit == source,
        }) {
            canvas.fill(hit.shape.clone(), brush.clone(), outer * hit.transform);
        }
    }
}

fn shape_bounds(shape: &puri::Shape) -> Rect {
    match shape {
        puri::Shape::Rect(shape) => shape.bounding_box(),
        puri::Shape::RoundedRect(shape) => shape.bounding_box(),
        puri::Shape::Circle(shape) => shape.bounding_box(),
        puri::Shape::Line(shape) => shape.bounding_box(),
        puri::Shape::Path(shape) => shape.bounding_box(),
    }
}

fn shape_contains(shape: &puri::Shape, point: Point) -> bool {
    match shape {
        puri::Shape::Rect(shape) => shape.contains(point),
        puri::Shape::RoundedRect(shape) => shape.contains(point),
        puri::Shape::Circle(shape) => shape.contains(point),
        puri::Shape::Line(shape) => shape.contains(point),
        puri::Shape::Path(shape) => shape.contains(point),
    }
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

fn record_program(
    program: impl Into<grap::RuntimeValue>,
    sources: &Sources,
    faces: &Faces,
    input: Option<&SourceTrace>,
    fuel: usize,
) -> Recorded {
    let program = program.into();
    let canvas = RefCell::new(DrawList::new());
    let hits = RefCell::new(Vec::new());
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
                let Some(paint) =
                    evaluated_field(context, call, environment, layout_data::vocabulary::PAINT)?
                else {
                    return Ok(context.missing_argument(layout_data::vocabulary::PAINT));
                };
                let shape = match context.field(call, layout_data::vocabulary::SHAPE) {
                    Some(shape) => evaluated_shape(&context.eval(shape, environment)?),
                    None => Some(puri::Shape::Path(path.borrow().clone())),
                };
                let transform = match context.field(call, layout_data::vocabulary::TRANSFORM) {
                    Some(transform) => evaluated_transform(&context.eval(transform, environment)?),
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
                let calls = context
                    .call_trace()
                    .map(|trace| SourceCalls::new(trace, input.cloned()));
                Ok(context.effect(|| {
                    if let Some(calls) = calls.filter(SourceCalls::has_source) {
                        hits.borrow_mut()
                            .push(Hit::new(shape.clone(), transform, calls));
                    }
                    canvas
                        .borrow_mut()
                        .fill(shape, faces.resolve(paint), transform);
                    unit.clone()
                }))
            }
            _ => unreachable!("the overlay only advertises drawing functions"),
        }
    };
    let overlay = grap::ForeignOverlay::from_value(&functions, &draw);
    let evaluation = grap::evaluate_runtime_scoped(&program, sources, &overlay, fuel);
    if evaluation.completed && !evaluation.result.declines() {
        Recorded {
            commands: canvas.into_inner(),
            hits: hits.into_inner(),
        }
    } else {
        Recorded {
            commands: DrawList::new(),
            hits: Vec::new(),
        }
    }
}

pub(crate) fn program_leaf(
    cx: &Cx,
    path: &[Step],
    width: f64,
    ascent: f64,
    descent: f64,
    fuel: usize,
    program: impl Into<grap::RuntimeValue>,
) -> Measured<HoverPass<crate::Editor>> {
    let program = program.into();
    let scale = cx.styles.scale;
    let extent = Extent {
        width: width * scale,
        ascent: ascent * scale,
        descent: descent * scale,
    };
    let faces = Faces::new(cx.styles);
    let program_path: Vec<_> = path
        .iter()
        .cloned()
        .chain([Step::Key(layout_data::vocabulary::PROGRAM)])
        .collect();
    let input = cx
        .edits
        .source(&program_path)
        .map(|location| SourceTrace::from_location(&cx.sources, &location));
    let document = cx.sources.doc.clone();
    let libraries = cx.sources.libraries.clone();
    let drawing = Rc::new(LazyCell::new(move || {
        record_program(
            &program,
            &Sources {
                doc: &document,
                libraries: &libraries,
            },
            &faces,
            input.as_ref(),
            fuel,
        )
    }));
    let highlight = cx.styles.accent_wash.brush.clone();
    let selected_highlight = cx.styles.selection_wash.clone();
    let selected = cx.selected_trace.clone();
    leaf(extent, move |builder, placement| {
        let outer =
            Affine::translate((placement.rect.x0, placement.rect.y0)) * Affine::scale(scale);
        let probe_drawing = drawing.clone();
        builder.claim_dynamic(placement, move |point| {
            probe_drawing.target_at(point, outer)
        });
        super::source_link::handlers(builder, placement, scale);
        builder.render(move |canvas: &mut dyn puri::draw::CanvasSink, hover| {
            canvas.clip(
                Rect::new(0.0, 0.0, width, ascent + descent),
                outer,
                |canvas| {
                    puri::draw::replay_at(&drawing.commands, canvas, outer);
                    if let Some(source) = &selected {
                        drawing.highlight(canvas, outer, source, &selected_highlight);
                    }
                    if let Some(source) = hover.hovered_trace.as_ref() {
                        drawing.highlight(canvas, outer, source, &highlight);
                    }
                },
            );
        });
    })
}

#[cfg(test)]
mod tests;
