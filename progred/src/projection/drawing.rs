//! Grap drawing programs. Each frame records a visible program once,
//! sharing its leaf-local Puri commands between hit-testing and painting.

use super::Cx;
use crate::frame::Hovered;
use crate::hover::{Hover, SourceCalls, SourceTrace};
use crate::libraries::{absent, layout as layout_data};
use crate::placed::{HoverPass, leaf};
use crate::sources::Sources;
use gid::{CellId, Step, Value};
use kurbo::{Affine, BezPath, Circle, Point, Rect, Shape as _};
use measured::{Extent, Measured};
use peniko::Brush;
use puri::draw::{Canvas, DrawList};
use std::cell::{LazyCell, RefCell};
use std::rc::Rc;

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

    fn resolve(&self, paint: crate::display::Paint) -> Brush {
        match paint {
            crate::display::Paint::Brush(brush) => brush,
            crate::display::Paint::Face(face) => match face {
                crate::display::Face::Name => self.name.clone(),
                crate::display::Face::String => self.string.clone(),
                crate::display::Face::Dim => self.dim.clone(),
                crate::display::Face::Label => self.label.clone(),
                crate::display::Face::Id => self.id.clone(),
                crate::display::Face::AccentWash => self.accent_wash.clone(),
                crate::display::Face::Ink => self.ink.clone(),
            },
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
        .field(&call, field)
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

/// Shape arguments are raw so coordinates may be ordinary Grap
/// expressions rather than a separately allocated quoted shape value.
fn shape(
    context: &mut grap::Context,
    expression: grap::Expression,
    environment: &grap::Environment,
) -> Result<Option<puri::Shape>, grap::Halt> {
    if let Some(content) = context.field(&expression, layout_data::vocabulary::RECT) {
        let (Some(x), Some(y), Some(width), Some(height)) = (
            context.field(&content, layout_data::vocabulary::X),
            context.field(&content, layout_data::vocabulary::Y),
            context.field(&content, layout_data::vocabulary::WIDTH),
            context.field(&content, layout_data::vocabulary::HEIGHT),
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
        return Ok((width >= 0.0 && height >= 0.0)
            .then(|| puri::Shape::Rect(Rect::new(x, y, x + width, y + height))));
    }
    if let Some(content) = context.field(&expression, layout_data::vocabulary::CIRCLE) {
        let (Some(x), Some(y), Some(radius)) = (
            context.field(&content, layout_data::vocabulary::X),
            context.field(&content, layout_data::vocabulary::Y),
            context.field(&content, layout_data::vocabulary::RADIUS),
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
        return Ok((radius >= 0.0).then(|| puri::Shape::Circle(Circle::new((x, y), radius))));
    }
    if let Some(content) = context.field(&expression, layout_data::vocabulary::PATH) {
        let content = context.eval_to_value(content, environment)?;
        return Ok(layout_data::read_shape(&Value::record([(
            layout_data::vocabulary::PATH,
            content,
        )])));
    }
    let value = context.eval_to_value(expression, environment)?;
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
    let Some(operation_count) = context.elements(&expression).map(<[_]>::len) else {
        let value = context.eval_to_value(expression, environment)?;
        return Ok(layout_data::read_transform(&value));
    };
    let mut transform = Affine::IDENTITY;
    for index in 0..operation_count {
        let operation = context.elements(&expression).unwrap()[index].clone();
        if let Some(point) = context.field(&operation, layout_data::vocabulary::TRANSLATE) {
            let (Some(x), Some(y)) = (
                context.field(&point, layout_data::vocabulary::X),
                context.field(&point, layout_data::vocabulary::Y),
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
        } else if let Some(angle) = context.field(&operation, layout_data::vocabulary::ROTATE) {
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
                        absent::vocabulary::VALUE,
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
                    Some(expression) => shape(context, expression, environment)?,
                    None => Some(puri::Shape::Path(path.borrow().clone())),
                };
                let transform = match context.field(call, layout_data::vocabulary::TRANSFORM) {
                    Some(expression) => transform(context, expression, environment)?,
                    None => Some(Affine::IDENTITY),
                };
                let (Some(shape), Some(paint), Some(transform)) =
                    (shape, layout_data::read_paint(&paint), transform)
                else {
                    return Ok(::grap::absent::with_detail(
                        layout_data::vocabulary::INVALID_DRAWING,
                        absent::vocabulary::VALUE,
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
        .map(|path| SourceTrace::from_path(&cx.sources, Rc::from(path.as_ref())));
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
mod tests {
    use super::*;
    use crate::libraries::Libraries;
    use gid::{Cells, Resolution, new_cell_id};

    #[test]
    fn declining_after_drawing_discards_the_whole_recording() {
        use crate::libraries::{control, f64};
        use layout_data::vocabulary as draw;
        let sequence = |values| {
            grap::call(
                control::vocabulary::DO.into(),
                [(control::vocabulary::EXPRESSIONS, Value::list(values))],
            )
        };
        let point = |function: CellId, x, y| {
            grap::call(
                function.into(),
                [(draw::X, f64::value(x)), (draw::Y, f64::value(y))],
            )
        };
        let fill = grap::call(
            draw::FILL.into(),
            [(
                draw::PAINT,
                crate::libraries::color::value(peniko::Color::BLACK),
            )],
        );
        let function = new_cell_id();
        let mut cells = Cells::new();
        cells.set_value(
            function,
            grap::lambda(
                [],
                sequence(vec![
                    grap::call(draw::PATH.into(), []),
                    point(draw::MOVE_TO, 100.0, 100.0),
                    point(draw::LINE_TO, 110.0, 110.0),
                    fill.clone(),
                    absent::decline(),
                ]),
            ),
        );
        let doc = gid::Document { root: None, cells };
        let mut stack = crate::stack::load();
        let library = new_cell_id();
        let mut fallback = Cells::new();
        fallback.set_value(function, grap::lambda([], fill));
        stack.libraries.insert(
            library,
            crate::libraries::Definitions::from_parts(fallback, Default::default()),
        );
        let drawing = record_program(
            &sequence(vec![
                grap::call(draw::PATH.into(), []),
                point(draw::MOVE_TO, 0.0, 0.0),
                point(draw::LINE_TO, 10.0, 10.0),
                grap::call(function.into(), []),
            ]),
            &Sources {
                doc: &doc,
                libraries: &stack.libraries,
            },
            &Faces::new(&crate::styles::editor(
                crate::styles::Theme::Light.palette(),
                1.0,
            )),
            Some(&SourceTrace::Stored(Rc::from([]))),
            200,
        );
        assert!(drawing.commands.0.is_empty());
        assert!(drawing.hits.is_empty());
    }

    #[test]
    fn recorded_hits_keep_the_executing_library_definition() {
        let function = new_cell_id();
        let library_ids = [new_cell_id(), new_cell_id()];
        let doc = gid::Document {
            root: Some(Value::from(function)),
            cells: Cells::new(),
        };
        let definition = grap::lambda(
            [],
            grap::call(
                Value::from(layout_data::vocabulary::FILL),
                [
                    (
                        layout_data::vocabulary::SHAPE,
                        layout_data::rect(0.0, 0.0, 10.0, 10.0),
                    ),
                    (
                        layout_data::vocabulary::PAINT,
                        crate::libraries::color::value(peniko::Color::BLACK),
                    ),
                ],
            ),
        );
        for order in [library_ids, [library_ids[1], library_ids[0]]] {
            let libraries = Libraries::from_contributions(order.map(|id| {
                let mut cells = Cells::new();
                cells.set_value(function, definition.clone());
                (
                    id,
                    crate::libraries::Library::<(), ()>::named(
                        id,
                        "drawing",
                        crate::libraries::Definitions::from_parts(
                            cells,
                            grap::ForeignFunctions::default(),
                        ),
                        crate::display::partial(|_| None),
                    ),
                )
            }))
            .0;
            let sources = Sources {
                doc: &doc,
                libraries: &libraries,
            };
            let drawing = record_program(
                &grap::call(Value::from(function), []),
                &sources,
                &Faces::new(&crate::styles::editor(
                    crate::styles::Theme::Light.palette(),
                    1.0,
                )),
                Some(&SourceTrace::Stored(Rc::from([]))),
                100,
            );
            assert_eq!(drawing.hits.len(), 1);
            let selected = SourceTrace::from_path(
                &sources,
                Rc::from([
                    Step::Follow(Resolution::Library(order[0])),
                    Step::Key(grap::vocabulary::BODY),
                ]),
            );
            let Some(Hovered::Tree(hover)) =
                drawing.target_at(Point::new(5.0, 5.0), Affine::IDENTITY)
            else {
                panic!("drawing should have a source");
            };
            assert_eq!(
                super::super::source_link::hover_source::<crate::Editor>(&sources, &[], &hover),
                Some(selected)
            );
            let other = SourceTrace::from_path(
                &sources,
                Rc::from([
                    Step::Follow(Resolution::Library(order[1])),
                    Step::Key(grap::vocabulary::BODY),
                ]),
            );
            let mut highlight = DrawList::new();
            drawing.highlight(
                &mut highlight,
                Affine::IDENTITY,
                &other,
                &Brush::from(peniko::Color::WHITE),
            );
            assert!(highlight.0.is_empty());
        }
    }

    #[test]
    fn recorded_hits_use_paint_order_and_the_current_placement() {
        let back = SourceTrace::Stored(Rc::from([Step::Key(new_cell_id())]));
        let front = SourceTrace::Stored(Rc::from([Step::Key(new_cell_id())]));
        let drawing = Recorded {
            commands: DrawList::new(),
            hits: vec![
                Hit::new(
                    puri::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                    Affine::IDENTITY,
                    back,
                ),
                Hit::new(
                    puri::Shape::Circle(Circle::new((5.0, 5.0), 4.0)),
                    Affine::IDENTITY,
                    front.clone(),
                ),
            ],
        };

        assert_eq!(
            drawing.target_at(Point::new(25.0, 35.0), Affine::translate((20.0, 30.0)),),
            Some(Hovered::Tree(Hover::Source(front))),
        );
    }

    #[test]
    fn a_source_descendant_does_not_highlight_its_operation() {
        let cell = new_cell_id();
        let call = new_cell_id();
        let argument = new_cell_id();
        let source = SourceTrace::InCell {
            cell,
            source: Resolution::Document,
            path: Rc::from([Step::Key(call)]),
        };
        let hovered = SourceTrace::InCell {
            cell,
            source: Resolution::Document,
            path: Rc::from([Step::Key(call), Step::Key(argument)]),
        };
        let drawing = Recorded {
            commands: DrawList::new(),
            hits: vec![Hit::new(
                puri::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                Affine::IDENTITY,
                source,
            )],
        };
        let mut highlighted = DrawList::new();

        drawing.highlight(
            &mut highlighted,
            Affine::IDENTITY,
            &hovered,
            &Brush::from(peniko::Color::WHITE),
        );

        assert!(highlighted.0.is_empty());
    }

    #[test]
    fn an_exact_source_highlights_its_operation() {
        let cell = new_cell_id();
        let call = new_cell_id();
        let source = SourceTrace::InCell {
            cell,
            source: Resolution::Document,
            path: Rc::from([Step::Key(call)]),
        };
        let drawing = Recorded {
            commands: DrawList::new(),
            hits: vec![Hit::new(
                puri::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                Affine::IDENTITY,
                source.clone(),
            )],
        };
        let mut highlighted = DrawList::new();

        drawing.highlight(
            &mut highlighted,
            Affine::IDENTITY,
            &source,
            &Brush::from(peniko::Color::WHITE),
        );

        assert_eq!(highlighted.0.len(), 1);
    }
}
