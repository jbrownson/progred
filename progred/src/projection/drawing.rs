//! Retained Grap drawing programs. A visible program records leaf-local
//! Puri commands when its expression or a cell it read changes; ordinary
//! frame rendering replays those commands at the current placement.

use super::Cx;
use crate::placed::{Placed, leaf};
use gid::{CellId, Cells, Value};
use measured::{Extent, Measured};
use progred_libraries::{absent, layout as layout_data};
use puri::draw::{Canvas, DrawList};
use kurbo::{Affine, BezPath, Circle, Rect};
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
    document: Cells,
    library: Cells,
    dependencies: BTreeSet<CellId>,
    commands: Rc<DrawList>,
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
    fn commands(
        &mut self,
        program: &Value,
        fuel: usize,
        faces: &Faces,
        document: &Cells,
        library: &Cells,
        record: impl FnOnce() -> (DrawList, BTreeSet<CellId>),
    ) -> Rc<DrawList> {
        match self.recording.take() {
            Some(mut recording) if recording.valid(program, fuel, faces, document, library) => {
                recording.document = document.clone();
                recording.library = library.clone();
                let commands = recording.commands.clone();
                self.recording = Some(recording);
                commands
            }
            _ => {
                let (commands, dependencies) = record();
                let commands = Rc::new(commands);
                self.recording = Some(Recording {
                    program: program.clone(),
                    fuel,
                    faces: faces.clone(),
                    document: document.clone(),
                    library: library.clone(),
                    dependencies,
                    commands: commands.clone(),
                });
                commands
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
        document: &Cells,
        library: &Cells,
    ) -> bool {
        self.program == *program
            && self.fuel == fuel
            && self.faces == *faces
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

fn record_program(
    program: &Value,
    document: &Cells,
    library: &Cells,
    foreign: &grap::ForeignFunctions,
    faces: &Faces,
    fuel: usize,
) -> (DrawList, BTreeSet<CellId>) {
    let canvas = RefCell::new(DrawList::new());
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
    (canvas.into_inner(), evaluation.dependencies)
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
    let node = cx.drawing_memo.node();
    let faces = Faces::new(cx.styles);
    let document = cx.sources.doc.cells.clone();
    let library = cx.sources.library.clone();
    let foreign = cx.foreign.clone();
    leaf(extent, move |builder, placement| {
        let outer = Affine::translate((placement.rect.x0, placement.rect.y0))
            * Affine::scale(scale);
        builder.ink(move |canvas: &mut Cv, _| {
            let commands = node.borrow_mut().commands(
                &program,
                fuel,
                &faces,
                &document,
                &library,
                || record_program(&program, &document, &library, &foreign, &faces, fuel),
            );
            canvas.clip(
                Rect::new(0.0, 0.0, width, ascent + descent),
                outer,
                |canvas| puri::draw::replay_at(&commands, canvas, outer),
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
    ) -> Rc<DrawList> {
        node.commands(program, 100, faces, document, library, || {
            recordings.set(recordings.get() + 1);
            (DrawList::new(), BTreeSet::from([dependency]))
        })
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
}
