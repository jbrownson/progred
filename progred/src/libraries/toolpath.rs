//! Experimental CAM path geometry, not machine-ready motion. Generators emit
//! through a sink; recording is one consumer, not the program representation.

use crate::libraries::{Definitions, Library, absent, f64, layout, name, presentation};
use ::grap::{
    Context, Environment, Evaluation, Expression, ForeignFunction, ForeignFunctions,
    ForeignOverlay, Halt,
};
use gid::{CellId, Cells, Value};
use std::{cell::RefCell, rc::Rc};

mod computation;
pub mod cutter;
mod fidget;
mod mesh;
pub mod paths;
mod playback;
mod preview;
mod refined;
pub mod stock;
#[cfg(test)]
mod tests;

pub const ID: CellId = CellId::from_u128(0xa3f84c8e8c0930058475518fe17bf84a);

pub mod vocabulary {
    use gid::CellId;
    pub const TOOLPATH: CellId = CellId::from_u128(0xfaaa4df9bd9bc104fc05c09a722f1ae9);
    pub const TOOL_AXIS: CellId = CellId::from_u128(0xc674bd24061031aff48153e907df4d59);
    pub const START_AT: CellId = CellId::from_u128(0x0b3ea6a88bdb8a105704e4af8b7950a5);
    pub const LINE_TO: CellId = CellId::from_u128(0xe88873219d230571430941334a51600f);
    pub const MAP_POINTS: CellId = CellId::from_u128(0x923fb213d0e56383539b43b1fc807458);
    pub const MAP_AXES: CellId = CellId::from_u128(0x5d5bf7f3beb0c1cba07226bab9683852);
    pub const WITH_TOOL: CellId = CellId::from_u128(0xd7c2368e471579d060460d3546158974);
    pub const MAPPER: CellId = CellId::from_u128(0xa33c6e235d5d94885fcb1afff59e94e5);
    pub const POINT: CellId = CellId::from_u128(0x61ca3de7d2601772ebedf8e2b2f5e0c5);
    pub const X: CellId = CellId::from_u128(0xae582d23ec214f7e2896d91c1b6756f6);
    pub const Y: CellId = CellId::from_u128(0x92227b03eda56073f7615c0688040b16);
    pub const Z: CellId = CellId::from_u128(0xece1c34e1fe1c2c45757d82d4ed6493d);
    pub const PREVIEW: CellId = CellId::from_u128(0xcdb8ec5a8c49550656067ace7eeb39b7);
    pub const PREVIEW_3D: CellId = CellId::from_u128(0x90764cc11a9ad180a4be8319232f2f8b);
    pub const PREVIEW_MESH: CellId = CellId::from_u128(0x6b5562bf1e69f9671cf97ad54985e4d0);
    pub const PREVIEW_REFINED: CellId = CellId::from_u128(0x76f66199a77dd4a6f7af65f29f1cacf7);
    pub const LINE_RADIUS: CellId = CellId::from_u128(0x4de64314b3008dcdf01ac387dc68e63f);
    pub const PROGRAM: CellId = CellId::from_u128(0xf23c804bf137581b76605a51366d43b7);
    pub const SEQUENCE: CellId = CellId::from_u128(0x1ecd44ed838a86892635ca2785826f2a);
    pub const FOCUS: CellId = CellId::from_u128(0x0548bc872965d8922fb8f93785b1f0d1);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0xa7763f9186b2c417fe1258246bb327db);
    pub const OUTPUT_REQUIRED: CellId = CellId::from_u128(0x1c905a1c1b3b904f8999fa61b3927897);
    pub const PLAYBACK: CellId = CellId::from_u128(0x0955045e00d5d5f2139fb3ab19591c3e);
    pub const PROGRESS: CellId = CellId::from_u128(0x13feb828ce93822d8d72a3ef3765e2f0);
    pub const TOOL_DIAMETER: CellId = CellId::from_u128(0x900b8ac5725af07ea981455d015541bf);
    pub const TOOL_LENGTH: CellId = CellId::from_u128(0x147708f5640ec6a4f9b6f4e4ffd2f7ab);
    pub const PROFILE_TOLERANCE: CellId = CellId::from_u128(0x4c3b5d5884e040ad1c226f37935ea68b);
    pub const STOCK_MIN: CellId = CellId::from_u128(0x8ea1172ebbc67d795f0810ee5ca04d00);
    pub const STOCK_MAX: CellId = CellId::from_u128(0x607b5e9a8636c8258959709bf0c3506b);
    pub const STOCK: CellId = CellId::from_u128(0xc7dc9217fe6809ea89ba31d458706e5c);
}

use paths::{Axis, InvalidPath, Point3, Sink};
use vocabulary::*;

const EMITTERS: &[CellId] = &[START_AT, LINE_TO, MAP_POINTS, MAP_AXES, WITH_TOOL, SEQUENCE];

fn point_value(point: Point3) -> Value {
    Value::record([X, Y, Z].into_iter().zip(point.map(f64::value)))
}

fn read_point(value: &Value) -> Option<Point3> {
    let fields = value.as_record()?;
    let point = [
        f64::read(fields.get(&X)?)?,
        f64::read(fields.get(&Y)?)?,
        f64::read(fields.get(&Z)?)?,
    ];
    point.into_iter().all(f64::is_finite).then_some(point)
}

enum Error {
    Invalid(Value),
    Halt(Halt),
}

impl From<Halt> for Error {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

fn invalid() -> Error {
    Error::Invalid(absent::with_reason(INVALID_INPUT))
}

fn argument(context: &Context, call: Expression, field: CellId) -> Result<Expression, Error> {
    context
        .field(call, field)
        .ok_or_else(|| Error::Invalid(context.missing_argument(field)))
}

fn number(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    field: CellId,
) -> Result<f64, Error> {
    let expression = argument(context, call, field)?;
    context
        .eval_f64(expression, environment)?
        .filter(|n| n.is_finite())
        .ok_or_else(invalid)
}

fn point(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<Point3, Error> {
    Ok([
        number(context, call, environment, X)?,
        number(context, call, environment, Y)?,
        number(context, call, environment, Z)?,
    ])
}

fn result(value: Result<Value, Error>) -> Result<Value, Halt> {
    match value {
        Ok(value) => Ok(value),
        Err(Error::Invalid(value)) => Ok(value),
        Err(Error::Halt(halt)) => Err(halt),
    }
}

fn read_fuel(value: f64) -> Option<usize> {
    (value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value < usize::MAX as f64)
        .then_some(value as usize)
}

fn fuel(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<usize, Error> {
    if context.field(call, layout::vocabulary::FUEL).is_some() {
        read_fuel(number(
            context,
            call,
            environment,
            layout::vocabulary::FUEL,
        )?)
        .ok_or_else(invalid)
    } else {
        Ok(::grap::DEFAULT_FUEL)
    }
}

struct Output<'a> {
    sink: RefCell<&'a mut dyn Sink<Error = InvalidPath>>,
    mappers: RefCell<Rc<Vec<Value>>>,
    axis_mappers: RefCell<Rc<Vec<Value>>>,
}

fn mapped_point(
    context: &mut Context,
    mappers: &RefCell<Rc<Vec<Value>>>,
    mut point: Point3,
) -> Result<Point3, Error> {
    let mappers = mappers.borrow().clone();
    for mapper in mappers.iter().rev() {
        let value = context.apply(mapper, [X, Y, Z].into_iter().zip(point.map(f64::value)))?;
        point = read_point(&value).ok_or_else(|| {
            if absent::is_absent(&value) {
                Error::Invalid(value)
            } else {
                invalid()
            }
        })?;
    }
    Ok(point)
}

struct Emission<'a, 'b, 'c> {
    output: &'a Output<'b>,
    context: &'a mut Context<'c>,
}

impl Emission<'_, '_, '_> {
    fn point(&mut self, point: Point3) -> Result<Point3, Error> {
        self.context.burn()?;
        mapped_point(self.context, &self.output.mappers, point)
    }
    fn start_at(&mut self, point: Point3, axis: Axis) -> Result<(), Error> {
        let point = self.point(point)?;
        let axis = if self.output.axis_mappers.borrow().is_empty() {
            axis
        } else {
            Axis::new(mapped_point(
                self.context,
                &self.output.axis_mappers,
                axis.vector(),
            )?)
            .ok_or_else(invalid)?
        };
        self.context
            .effect(|| self.output.sink.borrow_mut().start_at(point, axis))
            .map_err(|_| invalid())
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Error> {
        let point = self.point(point)?;
        self.context
            .effect(|| self.output.sink.borrow_mut().line_to(point))
            .map_err(|_| invalid())
    }
}

fn operation(
    function: CellId,
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    output: &Output,
) -> Result<Value, Error> {
    match function {
        SEQUENCE => {
            fn sequence(
                context: &mut Context,
                program: &Value,
                output: &Output,
            ) -> Result<Value, Halt> {
                context.burn()?;
                if let Some(list) = program.as_list() {
                    for child in list.values() {
                        let value = sequence(context, child, output)?;
                        if absent::is_absent(&value) {
                            return Ok(value);
                        }
                    }
                    Ok(Value::record([]))
                } else {
                    context.effect(|| output.sink.borrow_mut().end_path());
                    let result = context.apply(program, []);
                    context.effect(|| output.sink.borrow_mut().end_path());
                    result
                }
            }
            let program = argument(context, call, PROGRAM)?;
            let program = context.eval(program, environment)?;
            if absent::is_absent(&program) {
                return Ok(program);
            }
            return Ok(sequence(context, &program, output)?);
        }
        START_AT | LINE_TO => {
            let point = point(context, call, environment)?;
            let mut emission = Emission { output, context };
            if function == START_AT {
                let axis = if let Some(expression) = emission.context.field(call, TOOL_AXIS) {
                    let value = emission.context.eval(expression, environment)?;
                    Axis::new(read_point(&value).ok_or_else(invalid)?).ok_or_else(invalid)?
                } else {
                    Axis::Z
                };
                emission.start_at(point, axis)?;
            } else {
                emission.line_to(point)?;
            }
        }
        MAP_POINTS | MAP_AXES => {
            let mapper = argument(context, call, MAPPER)?;
            let expression = argument(context, call, ::grap::vocabulary::EXPRESSION)?;
            let mapper = context.eval(mapper, environment)?;
            let mappers = if function == MAP_POINTS {
                &output.mappers
            } else {
                &output.axis_mappers
            };
            let parent = mappers.borrow().clone();
            Rc::make_mut(&mut mappers.borrow_mut()).push(mapper);
            let value = context.eval(expression, environment);
            mappers.replace(parent);
            return Ok(value?);
        }
        WITH_TOOL => {
            let tool = argument(context, call, cutter::vocabulary::TOOL)?;
            let expression = argument(context, call, ::grap::vocabulary::EXPRESSION)?;
            let value = context.eval(tool, environment)?;
            if absent::is_absent(&value) {
                return Ok(value);
            }
            let tool = cutter::Tool::read(&value).ok_or_else(invalid)?;
            // Release the sink borrow before evaluating the body: nested calls
            // emit into it too. Always leave the scope, including on a halt.
            context.effect(|| output.sink.borrow_mut().enter_tool(&tool));
            let value = context.eval(expression, environment);
            context.effect(|| output.sink.borrow_mut().leave_tool());
            return Ok(value?);
        }
        _ => unreachable!("only toolpath emitters are installed in this scope"),
    }
    Ok(Value::record([]))
}

/// Run with caller-owned output. Consumers that need atomic results stage their
/// sink and discard it on failure; the preview does this for each frame.
pub fn run(
    sink: &mut dyn Sink<Error = InvalidPath>,
    evaluate: impl FnOnce(&ForeignOverlay<'_>) -> Evaluation,
) -> Evaluation {
    let output = Output {
        sink: RefCell::new(sink),
        mappers: RefCell::new(Rc::new(Vec::new())),
        axis_mappers: RefCell::new(Rc::new(Vec::new())),
    };
    let emit = |function, context: &mut Context<'_>, call, environment: &Environment| {
        result(operation(function, context, call, environment, &output))
    };
    evaluate(&ForeignOverlay::new(EMITTERS, &emit).tracked())
}

fn functions() -> ForeignFunctions {
    let functions = EMITTERS
        .iter()
        .fold(ForeignFunctions::default(), |functions, &cell| {
            functions.register(
                cell,
                ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))).tracked(),
            )
        });
    functions
        .register(PREVIEW_3D, ForeignFunction::new(fidget::preview).tracked())
        .register(PREVIEW_MESH, ForeignFunction::new(mesh::preview).tracked())
        .register(
            PREVIEW_REFINED,
            ForeignFunction::new(refined::preview).tracked(),
        )
        .register(
            POINT,
            ForeignFunction::new(|context, call, environment| {
                result(point(context, call, environment).map(point_value))
            })
            .tracked(),
        )
        .register(
            PREVIEW,
            ForeignFunction::new(|context, call, environment| {
                result((|| {
                    let program = argument(context, call, presentation::vocabulary::VALUE)?;
                    let program = context.eval(program, environment)?;
                    let width = number(context, call, environment, layout::vocabulary::WIDTH)?;
                    let height = number(context, call, environment, layout::vocabulary::HEIGHT)?;
                    if width <= 0.0 || height <= 0.0 {
                        return Err(invalid());
                    }
                    let fuel = fuel(context, call, environment)?;
                    Ok(Value::record([(
                        PREVIEW,
                        Value::record([
                            (PROGRAM, program),
                            (layout::vocabulary::WIDTH, f64::value(width)),
                            (layout::vocabulary::HEIGHT, f64::value(height)),
                            (layout::vocabulary::FUEL, f64::value(fuel as f64)),
                        ]),
                    )]))
                })())
            })
            .tracked(),
        )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (id, spelling) in [
        (TOOLPATH, "toolpaths"),
        (START_AT, "start at"),
        (TOOL_AXIS, "tool axis"),
        (LINE_TO, "line to"),
        (MAP_POINTS, "map points"),
        (MAP_AXES, "map axes"),
        (WITH_TOOL, "with tool"),
        (MAPPER, "mapping"),
        (POINT, "point"),
        (X, "x"),
        (Y, "y"),
        (Z, "z"),
        (PREVIEW, "preview paths"),
        (PREVIEW_3D, "preview paths 3d"),
        (PREVIEW_MESH, "preview paths mesh"),
        (PREVIEW_REFINED, "preview paths refined"),
        (LINE_RADIUS, "line radius"),
        (PROGRAM, "program"),
        (SEQUENCE, "sequence paths"),
        (FOCUS, "focus"),
        (PLAYBACK, "playback"),
        (PROGRESS, "progress"),
        (TOOL_DIAMETER, "tool diameter"),
        (TOOL_LENGTH, "tool length"),
        (PROFILE_TOLERANCE, "profile tolerance"),
        (STOCK_MIN, "stock minimum"),
        (STOCK_MAX, "stock maximum"),
        (STOCK, "stock"),
        (INVALID_INPUT, "invalid toolpath input"),
        (OUTPUT_REQUIRED, "toolpath output required"),
    ] {
        cells.set_value(id, name::record(spelling, []));
    }
    for (id, spelling) in cutter::names() {
        cells.set_value(id, name::record(spelling, []));
    }
    Library::named(
        ID,
        "toolpaths",
        Definitions::from_parts(cells, cutter::functions(functions())),
        crate::display::compose_partials([
            crate::display::partial(cutter::display),
            crate::display::partial(preview::display),
            crate::display::partial(fidget::display),
            crate::display::partial(mesh::display),
            crate::display::partial(refined::display),
        ]),
    )
}
