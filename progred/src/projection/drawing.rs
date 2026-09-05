//! Grap drawing programs. Each frame records a visible program once,
//! sharing its leaf-local Puri commands between hit-testing and painting.

use super::Cx;
use crate::frame::Hovered;
use crate::hover::{Hover, SourceTrace};
use crate::placed::{Placed, leaf};
use crate::sources::Sources;
use gid::{CellId, Step, Value};
use kurbo::{Affine, Circle, Point, Rect, Shape as _};
use measured::{Extent, Measured};
use peniko::Brush;
use progred_libraries::{absent, layout as layout_data};
use puri::draw::{Canvas, DrawCmd, DrawList};
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

struct Recorded {
    commands: DrawList,
    hits: Vec<Hit>,
}

#[derive(Clone)]
struct Hit {
    shape: puri::Shape,
    transform: Affine,
    inverse: Affine,
    bounds: Rect,
    source: SourceTrace,
}

impl Hit {
    fn new(shape: puri::Shape, transform: Affine, source: SourceTrace) -> Self {
        let bounds = transform.transform_rect_bbox(shape_bounds(&shape));
        Self {
            shape,
            transform,
            inverse: transform.inverse(),
            bounds,
            source,
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
            .map(|hit| Hovered::Tree(Hover::Drawing(hit.source.clone())))
    }

    fn highlight<C: Canvas>(
        &self,
        canvas: &mut C,
        outer: Affine,
        source: &SourceTrace,
        brush: &Brush,
    ) {
        self.highlight_where(canvas, outer, brush, |hit| hit == source);
    }

    fn highlight_where<C: Canvas>(
        &self,
        canvas: &mut C,
        outer: Affine,
        brush: &Brush,
        matches: impl Fn(&SourceTrace) -> bool,
    ) {
        for hit in self.hits.iter().filter(|hit| matches(&hit.source)) {
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
        return Ok((width >= 0.0 && height >= 0.0)
            .then(|| puri::Shape::Rect(Rect::new(x, y, x + width, y + height))));
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
        return Ok((radius >= 0.0).then(|| puri::Shape::Circle(Circle::new((x, y), radius))));
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
    let Some(operation_count) = context.elements(expression).map(<[_]>::len) else {
        let value = context.eval(expression, environment)?;
        return Ok(layout_data::read_transform(&value));
    };
    let mut transform = Affine::IDENTITY;
    for index in 0..operation_count {
        let operation = context.elements(expression).unwrap()[index];
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
        } else if let Some(angle) = context.field(operation, layout_data::vocabulary::ROTATE) {
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
    program: &Value,
    sources: &Sources,
    faces: &Faces,
    input: &SourceTrace,
    fuel: usize,
) -> Recorded {
    #[derive(Clone, Default)]
    struct Drawing {
        commands: im::Vector<Rc<(DrawCmd, Option<Hit>)>>,
        path: im::Vector<kurbo::PathEl>,
    }
    let drawing = grap::Effects::new(Drawing::default());
    // Fill call sites are few; a scan beats hashing per drawn shape.
    let origins = RefCell::new(Vec::<(grap::Expression, Option<SourceTrace>)>::new());
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
                drawing.borrow_mut().path = im::Vector::new();
                Ok(unit.clone())
            }
            layout_data::vocabulary::MOVE_TO | layout_data::vocabulary::LINE_TO => {
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
                    drawing
                        .borrow_mut()
                        .path
                        .push_back(kurbo::PathEl::MoveTo((x, y).into()));
                } else {
                    drawing
                        .borrow_mut()
                        .path
                        .push_back(kurbo::PathEl::LineTo((x, y).into()));
                }
                Ok(unit.clone())
            }
            layout_data::vocabulary::CLOSE => {
                drawing
                    .borrow_mut()
                    .path
                    .push_back(kurbo::PathEl::ClosePath);
                Ok(unit.clone())
            }
            layout_data::vocabulary::FILL => {
                let Some(paint) =
                    evaluated_field(context, call, environment, layout_data::vocabulary::PAINT)?
                else {
                    return Ok(context.missing_argument(layout_data::vocabulary::PAINT));
                };
                let shape = match context.field(call, layout_data::vocabulary::SHAPE) {
                    Some(expression) => shape(context, expression, environment)?,
                    None => Some(puri::Shape::Path(
                        drawing.borrow().path.iter().copied().collect(),
                    )),
                };
                let transform = match context.field(call, layout_data::vocabulary::TRANSFORM) {
                    Some(expression) => transform(context, expression, environment)?,
                    None => Some(Affine::IDENTITY),
                };
                let (Some(shape), Some(paint), Some(transform)) =
                    (shape, layout_data::read_paint(&paint), transform)
                else {
                    return Ok(absent::value());
                };
                let cached = origins
                    .borrow()
                    .iter()
                    .find(|(site, _)| *site == call)
                    .map(|(_, source)| source.clone());
                let source = match cached {
                    Some(source) => source,
                    None => {
                        let source = context
                            .source_origin(call)
                            .map(|origin| SourceTrace::from_grap(origin, input));
                        origins.borrow_mut().push((call, source.clone()));
                        source
                    }
                };
                let hit = source.map(|source| Hit::new(shape.clone(), transform, source));
                drawing.borrow_mut().commands.push_back(Rc::new((
                    DrawCmd::Fill {
                        shape,
                        brush: faces.resolve(paint),
                        transform,
                    },
                    hit,
                )));
                Ok(unit.clone())
            }
            _ => unreachable!("the overlay only advertises drawing functions"),
        }
    };
    let overlay = grap::ForeignOverlay::new(&functions, &draw).with_effects(&drawing);
    grap::evaluate_scoped(
        program,
        |cell| sources.grap_definitions(cell),
        &overlay,
        fuel,
    );
    drawing
        .into_inner()
        .commands
        .into_iter()
        .map(Rc::unwrap_or_clone)
        .fold(
            Recorded {
                commands: DrawList::new(),
                hits: Vec::new(),
            },
            |mut recorded, (command, hit)| {
                recorded.commands.0.push(command);
                recorded.hits.extend(hit);
                recorded
            },
        )
}

pub(super) fn program_leaf<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    path: &[Step],
    width: f64,
    ascent: f64,
    descent: f64,
    fuel: usize,
    program: Value,
    select_source: Rc<dyn Fn(&mut C, &[crate::navigate::Descend<C>], &SourceTrace)>,
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    let extent = Extent {
        width: width * scale,
        ascent: ascent * scale,
        descent: descent * scale,
    };
    let faces = Faces::new(cx.styles);
    let input = SourceTrace::from_path(
        &cx.sources,
        path.iter()
            .cloned()
            .chain([Step::Key(layout_data::vocabulary::PROGRAM)])
            .collect::<Rc<[Step]>>(),
    );
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
            &input,
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
        builder.pick_dynamic(placement, move |world, target, descends| {
            if let Hovered::Tree(Hover::Drawing(source)) = target {
                select_source(world, descends, source);
                // The painted hit owns the pick even without a visible source occurrence.
                true
            } else {
                false
            }
        });
        builder.ink(move |canvas: &mut Cv, ink| {
            canvas.clip(
                Rect::new(0.0, 0.0, width, ascent + descent),
                outer,
                |canvas| {
                    puri::draw::replay_at(&drawing.commands, canvas, outer);
                    if let Some(source) = &selected {
                        drawing.highlight(canvas, outer, source, &selected_highlight);
                    }
                    if let Some(source) = ink.hovered_trace {
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
    use gid::{Cells, Resolution, new_cell_id};
    use progred_libraries::Libraries;

    #[test]
    fn a_declined_definition_discards_its_ink_hits_and_path_changes() {
        use layout_data::vocabulary as draw;
        use progred_libraries::{control, f64};
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
                progred_libraries::color::value(peniko::Color::BLACK),
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
        let mut stack = crate::stack::load::<()>();
        let library = new_cell_id();
        let mut fallback = Cells::new();
        fallback.set_value(function, grap::lambda([], fill));
        stack.libraries.insert(
            library,
            Value::record([]),
            progred_libraries::Definitions::from_parts(fallback, Default::default()),
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
            &Faces::new(&crate::styles::editor(1.0)),
            &SourceTrace::Stored(Rc::from([])),
            200,
        );
        let [
            DrawCmd::Fill {
                shape: puri::Shape::Path(path),
                ..
            },
        ] = drawing.commands.0.as_slice()
        else {
            panic!("only the fallback fill should remain");
        };
        assert_eq!(
            path.elements(),
            &[
                kurbo::PathEl::MoveTo((0.0, 0.0).into()),
                kurbo::PathEl::LineTo((10.0, 10.0).into()),
            ]
        );
        let [hit] = drawing.hits.as_slice() else {
            panic!("only the fallback hit should remain");
        };
        assert!(
            matches!(&hit.source, SourceTrace::InCell { cell, source: Resolution::Library(id), .. } if *cell == function && *id == library)
        );
    }

    #[test]
    fn recorded_hits_keep_the_executing_library_definition() {
        let function = new_cell_id();
        let library_ids = [new_cell_id(), new_cell_id()];
        let mut cells = Cells::new();
        cells.set_value(function, grap::lambda([], absent::decline()));
        let doc = gid::Document {
            root: Some(Value::from(function)),
            cells,
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
                        progred_libraries::color::value(peniko::Color::BLACK),
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
                    progred_libraries::Library::<(), ()>::named(
                        "drawing",
                        progred_libraries::Definitions::from_parts(
                            cells,
                            grap::ForeignFunctions::default(),
                        ),
                        vec![],
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
                &Faces::new(&crate::styles::editor(1.0)),
                &SourceTrace::Stored(Rc::from([])),
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
            assert_eq!(
                drawing.target_at(Point::new(5.0, 5.0), Affine::IDENTITY),
                Some(Hovered::Tree(Hover::Drawing(selected)))
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
            Some(Hovered::Tree(Hover::Drawing(front))),
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
