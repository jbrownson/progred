//! Retained Grap drawing programs. A visible program records leaf-local
//! Puri commands when its expression or a cell it read changes; ordinary
//! frame rendering replays those commands at the current placement.

use super::Cx;
use crate::frame::Hovered;
use crate::hover::{Hover, SourceTrace};
use crate::placed::{Placed, leaf};
use gid::{CellId, Cells, Step, Value};
use measured::{Extent, Measured};
use progred_libraries::{absent, layout as layout_data};
use puri::draw::{Canvas, DrawList};
use kurbo::{Affine, BezPath, Circle, Point, Rect, Shape as _};
use peniko::Brush;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
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

#[derive(Default)]
pub(crate) struct Memo {
    state: RefCell<MemoState>,
}

#[derive(Default)]
struct MemoState {
    next: usize,
    nodes: Vec<Rc<RefCell<Node>>>,
}

#[derive(Default)]
struct Node {
    recording: Option<Recording>,
}

struct Recording {
    program: Value,
    fuel: usize,
    faces: Faces,
    input: SourceTrace,
    document: Cells,
    library: Cells,
    dependencies: BTreeSet<CellId>,
    drawing: Rc<Recorded>,
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
        self.highlight_where(canvas, outer, brush, |hit| hit.contains(source));
    }

    fn highlight_exact<C: Canvas>(
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
            canvas.fill(
                hit.shape.clone(),
                brush.clone(),
                outer * hit.transform,
            );
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

impl Memo {
    pub(crate) fn begin(&self) {
        self.state.borrow_mut().next = 0;
    }

    pub(crate) fn finish(&self) {
        let mut state = self.state.borrow_mut();
        let used = state.next;
        state.nodes.truncate(used);
    }

    fn node(&self) -> Rc<RefCell<Node>> {
        let mut state = self.state.borrow_mut();
        let index = state.next;
        state.next += 1;
        match state.nodes.get(index) {
            Some(node) => node.clone(),
            None => {
                let node = Rc::new(RefCell::new(Node::default()));
                state.nodes.push(node.clone());
                node
            }
        }
    }
}

impl Node {
    fn drawing(
        &mut self,
        program: &Value,
        fuel: usize,
        faces: &Faces,
        input: &SourceTrace,
        document: &Cells,
        library: &Cells,
        record: impl FnOnce() -> (Recorded, BTreeSet<CellId>),
    ) -> Rc<Recorded> {
        match self.recording.take() {
            Some(mut recording)
                if recording.valid(program, fuel, faces, input, document, library) =>
            {
                recording.document = document.clone();
                recording.library = library.clone();
                let drawing = recording.drawing.clone();
                self.recording = Some(recording);
                drawing
            }
            _ => {
                let (drawing, dependencies) = record();
                let drawing = Rc::new(drawing);
                self.recording = Some(Recording {
                    program: program.clone(),
                    fuel,
                    faces: faces.clone(),
                    input: input.clone(),
                    document: document.clone(),
                    library: library.clone(),
                    dependencies,
                    drawing: drawing.clone(),
                });
                drawing
            }
        }
    }
}

impl Recording {
    fn valid(
        &self,
        program: &Value,
        fuel: usize,
        faces: &Faces,
        input: &SourceTrace,
        document: &Cells,
        library: &Cells,
    ) -> bool {
        self.program == *program
            && self.fuel == fuel
            && self.faces == *faces
            && self.input == *input
            && ((self.document.ptr_eq(document) && self.library.ptr_eq(library))
                || self.dependencies.iter().all(|cell| {
                    resolved(&self.document, &self.library, *cell)
                        == resolved(document, library, *cell)
                }))
    }
}

fn resolved<'a>(document: &'a Cells, library: &'a Cells, cell: CellId) -> Option<&'a Value> {
    document.value(cell).or_else(|| library.value(cell))
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

fn record_program(
    program: &Value,
    document: &Cells,
    library: &Cells,
    foreign: &grap::ForeignFunctions,
    faces: &Faces,
    input: &SourceTrace,
    fuel: usize,
) -> (Recorded, BTreeSet<CellId>) {
    let canvas = RefCell::new(DrawList::new());
    let hits = RefCell::new(Vec::new());
    // Fill call sites are few; a scan beats hashing per drawn shape.
    let origins = RefCell::new(Vec::<(grap::Expression, Option<SourceTrace>)>::new());
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
                call,
                environment: &grap::Environment| {
        match function {
            layout_data::vocabulary::PATH => {
                *path.borrow_mut() = BezPath::new();
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
                if let Some(source) = source {
                    hits.borrow_mut()
                        .push(Hit::new(shape.clone(), transform, source));
                }
                canvas
                    .borrow_mut()
                    .fill(shape, faces.resolve(paint), transform);
                Ok(unit.clone())
            }
            _ => unreachable!("the overlay only advertises drawing functions"),
        }
    };
    let overlay = grap::ForeignOverlay::new(&functions, &draw);
    let evaluation = grap::evaluate_scoped(
        program,
        |cell| {
            document
                .value(cell)
                .or_else(|| library.value(cell))
                .cloned()
        },
        foreign,
        &overlay,
        fuel,
    );
    debug_assert!(
        evaluation.diagnostics.is_empty(),
        "drawing program diagnostics: {:?}",
        evaluation.diagnostics,
    );
    (
        Recorded {
            commands: canvas.into_inner(),
            hits: hits.into_inner(),
        },
        evaluation.dependencies,
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
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    let extent = Extent {
        width: width * scale,
        ascent: ascent * scale,
        descent: descent * scale,
    };
    let node = cx.drawing_memo.node();
    let faces = Faces::new(cx.styles);
    let input = SourceTrace::from_path(
        &cx.sources,
        path.iter()
            .cloned()
            .chain([Step::Key(layout_data::vocabulary::PROGRAM)])
            .collect::<Rc<[Step]>>(),
    );
    let document = cx.sources.doc.cells.clone();
    let library = cx.sources.library.clone();
    let foreign = cx.foreign.clone();
    let drawing = Rc::new(move || {
        node.borrow_mut().drawing(
            &program,
            fuel,
            &faces,
            &input,
            &document,
            &library,
            || record_program(
                &program,
                &document,
                &library,
                &foreign,
                &faces,
                &input,
                fuel,
            ),
        )
    });
    let highlight = cx.styles.accent_wash.brush.clone();
    let selected_highlight = cx.styles.selection_wash.clone();
    let selected = cx.selected_trace.clone();
    leaf(extent, move |builder, placement| {
        let outer = Affine::translate((placement.rect.x0, placement.rect.y0))
            * Affine::scale(scale);
        let probe_drawing = drawing.clone();
        builder.claim_dynamic(placement, move |point| {
            probe_drawing().target_at(point, outer)
        });
        builder.ink(move |canvas: &mut Cv, ink| {
            let drawing = drawing();
            canvas.clip(
                Rect::new(0.0, 0.0, width, ascent + descent),
                outer,
                |canvas| {
                    puri::draw::replay_at(&drawing.commands, canvas, outer);
                    if let Some(source) = &selected {
                        drawing.highlight_exact(canvas, outer, source, &selected_highlight);
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
    use gid::new_cell_id;
    use std::cell::Cell;

    fn commands(
        node: &mut Node,
        program: &Value,
        faces: &Faces,
        document: &Cells,
        library: &Cells,
        dependency: CellId,
        recordings: &Cell<usize>,
    ) -> Rc<Recorded> {
        node.drawing(
            program,
            100,
            faces,
            &SourceTrace::Stored(Rc::from([])),
            document,
            library,
            || {
                recordings.set(recordings.get() + 1);
                (
                    Recorded {
                        commands: DrawList::new(),
                        hits: Vec::new(),
                    },
                    BTreeSet::from([dependency]),
                )
            },
        )
    }

    #[test]
    fn a_node_reuses_commands_until_its_program_or_a_dependency_changes() {
        let dependency = new_cell_id();
        let unrelated = new_cell_id();
        let program = Value::from(vec![1]);
        let brush = Brush::from(peniko::Color::BLACK);
        let faces = Faces {
            name: brush.clone(),
            string: brush.clone(),
            dim: brush.clone(),
            label: brush.clone(),
            id: brush.clone(),
            accent_wash: brush.clone(),
            ink: brush,
        };
        let library = Cells::new();
        let mut document = Cells::new();
        document.set_value(dependency, Value::from(vec![2]));
        let recordings = Cell::new(0);
        let mut node = Node::default();

        let first = commands(
            &mut node,
            &program,
            &faces,
            &document,
            &library,
            dependency,
            &recordings,
        );
        let unchanged = commands(
            &mut node,
            &program,
            &faces,
            &document,
            &library,
            dependency,
            &recordings,
        );
        assert!(Rc::ptr_eq(&first, &unchanged));
        assert_eq!(recordings.get(), 1);

        document.set_value(unrelated, Value::from(vec![3]));
        let unrelated_change = commands(
            &mut node,
            &program,
            &faces,
            &document,
            &library,
            dependency,
            &recordings,
        );
        assert!(Rc::ptr_eq(&first, &unrelated_change));
        assert_eq!(recordings.get(), 1);

        document.set_value(dependency, Value::from(vec![4]));
        let dependency_change = commands(
            &mut node,
            &program,
            &faces,
            &document,
            &library,
            dependency,
            &recordings,
        );
        assert!(!Rc::ptr_eq(&first, &dependency_change));
        assert_eq!(recordings.get(), 2);

        let changed_faces = Faces {
            ink: Brush::from(peniko::Color::WHITE),
            ..faces.clone()
        };
        let face_change = commands(
            &mut node,
            &program,
            &changed_faces,
            &document,
            &library,
            dependency,
            &recordings,
        );
        assert!(!Rc::ptr_eq(&dependency_change, &face_change));
        assert_eq!(recordings.get(), 3);

        let program_change = commands(
            &mut node,
            &Value::from(vec![5]),
            &changed_faces,
            &document,
            &library,
            dependency,
            &recordings,
        );
        assert!(!Rc::ptr_eq(&face_change, &program_change));
        assert_eq!(recordings.get(), 4);
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
            drawing.target_at(
                Point::new(25.0, 35.0),
                Affine::translate((20.0, 30.0)),
            ),
            Some(Hovered::Tree(Hover::Drawing(front))),
        );
    }

    #[test]
    fn a_source_descendant_highlights_the_operation_that_contains_it() {
        let cell = new_cell_id();
        let call = new_cell_id();
        let argument = new_cell_id();
        let source = SourceTrace::InCell {
            cell,
            path: Rc::from([Step::Key(call)]),
        };
        let hovered = SourceTrace::InCell {
            cell,
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

        assert_eq!(highlighted.0.len(), 1);
    }

    #[test]
    fn selecting_a_source_descendant_does_not_highlight_its_operation() {
        let cell = new_cell_id();
        let call = new_cell_id();
        let argument = new_cell_id();
        let source = SourceTrace::InCell {
            cell,
            path: Rc::from([Step::Key(call)]),
        };
        let selected = SourceTrace::InCell {
            cell,
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

        drawing.highlight_exact(
            &mut highlighted,
            Affine::IDENTITY,
            &selected,
            &Brush::from(peniko::Color::WHITE),
        );

        assert!(highlighted.0.is_empty());
    }
}
