//! Experimental CAM path geometry, not machine-ready motion. Generators emit
//! through a sink; recording is one consumer, not the program representation.

use crate::libraries::{Definitions, Library, absent, f64, layout, name, presentation};
use ::grap::{
    Context, Environment, Evaluation, Expression, ForeignFunction, ForeignFunctions,
    ForeignOverlay, Halt,
};
use gid::{CellId, Cells, Value};
use std::{cell::RefCell, rc::Rc};

pub mod paths;
mod preview;
#[cfg(test)]
mod tests;

pub const ID: CellId = CellId::from_u128(0xa3f84c8e8c0930058475518fe17bf84a);

pub mod vocabulary {
    use gid::CellId;
    pub const TOOLPATH: CellId = CellId::from_u128(0xfaaa4df9bd9bc104fc05c09a722f1ae9);
    pub const START_AT: CellId = CellId::from_u128(0x0b3ea6a88bdb8a105704e4af8b7950a5);
    pub const LINE_TO: CellId = CellId::from_u128(0xe88873219d230571430941334a51600f);
    pub const MAP_POINTS: CellId = CellId::from_u128(0x923fb213d0e56383539b43b1fc807458);
    pub const MAPPER: CellId = CellId::from_u128(0xa33c6e235d5d94885fcb1afff59e94e5);
    pub const POINT: CellId = CellId::from_u128(0x61ca3de7d2601772ebedf8e2b2f5e0c5);
    pub const X: CellId = CellId::from_u128(0xae582d23ec214f7e2896d91c1b6756f6);
    pub const Y: CellId = CellId::from_u128(0x92227b03eda56073f7615c0688040b16);
    pub const Z: CellId = CellId::from_u128(0xece1c34e1fe1c2c45757d82d4ed6493d);
    pub const PREVIEW: CellId = CellId::from_u128(0xcdb8ec5a8c49550656067ace7eeb39b7);
    pub const PROGRAM: CellId = CellId::from_u128(0xf23c804bf137581b76605a51366d43b7);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0xa7763f9186b2c417fe1258246bb327db);
    pub const OUTPUT_REQUIRED: CellId = CellId::from_u128(0x1c905a1c1b3b904f8999fa61b3927897);
}

use paths::{InvalidPath, Point3, Sink};
use vocabulary::*;

const EMITTERS: &[CellId] = &[START_AT, LINE_TO, MAP_POINTS];

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

struct Output<'a> {
    sink: RefCell<&'a mut dyn Sink<Error = InvalidPath>>,
    mappers: RefCell<Rc<Vec<Value>>>,
    failure: RefCell<Option<Value>>,
}

struct Emission<'a, 'b, 'c> {
    output: &'a Output<'b>,
    context: &'a mut Context<'c>,
}

impl Emission<'_, '_, '_> {
    fn point(&mut self, mut point: Point3) -> Result<Point3, Error> {
        self.context.burn()?;
        let mappers = self.output.mappers.borrow().clone();
        for mapper in mappers.iter().rev() {
            let value = self
                .context
                .apply(mapper, [X, Y, Z].into_iter().zip(point.map(f64::value)))?;
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
}

impl Sink for Emission<'_, '_, '_> {
    type Error = Error;

    fn start_at(&mut self, point: Point3) -> Result<(), Error> {
        let point = self.point(point)?;
        self.context
            .effect(|| self.output.sink.borrow_mut().start_at(point))
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
        START_AT | LINE_TO => {
            let point = point(context, call, environment)?;
            let mut emission = Emission { output, context };
            if function == START_AT {
                emission.start_at(point)?;
            } else {
                emission.line_to(point)?;
            }
        }
        MAP_POINTS => {
            let mapper = argument(context, call, MAPPER)?;
            let expression = argument(context, call, ::grap::vocabulary::EXPRESSION)?;
            let mapper = context.eval(mapper, environment)?;
            let parent = output.mappers.borrow().clone();
            Rc::make_mut(&mut output.mappers.borrow_mut()).push(mapper);
            let value = context.eval(expression, environment);
            output.mappers.replace(parent);
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
        failure: RefCell::new(None),
    };
    let emit = |function, context: &mut Context<'_>, call, environment: &Environment| {
        if let Some(error) = output.failure.borrow().as_ref() {
            return Ok(error.clone());
        }
        match operation(function, context, call, environment, &output) {
            Ok(value) => Ok(value),
            Err(Error::Halt(halt)) => Err(halt),
            Err(Error::Invalid(error)) => Ok(context.effect(|| {
                output.failure.replace(Some(error.clone()));
                error
            })),
        }
    };
    let mut evaluation = evaluate(&ForeignOverlay::new(EMITTERS, &emit));
    if evaluation.completed {
        if let Some(error) = output.failure.into_inner() {
            evaluation.result = error;
        }
    }
    evaluation
}

fn functions() -> ForeignFunctions {
    let functions = EMITTERS
        .iter()
        .fold(ForeignFunctions::default(), |functions, &cell| {
            functions.register(
                cell,
                ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
            )
        });
    functions
        .register(
            POINT,
            ForeignFunction::new(|context, call, environment| {
                result(point(context, call, environment).map(point_value))
            }),
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
                    let fuel = if context.field(call, layout::vocabulary::FUEL).is_some() {
                        number(context, call, environment, layout::vocabulary::FUEL)?
                    } else {
                        ::grap::DEFAULT_FUEL as f64
                    };
                    if fuel < 0.0 || fuel.fract() != 0.0 || fuel >= usize::MAX as f64 {
                        return Err(invalid());
                    }
                    Ok(Value::record([(
                        PREVIEW,
                        Value::record([
                            (PROGRAM, program),
                            (layout::vocabulary::WIDTH, f64::value(width)),
                            (layout::vocabulary::HEIGHT, f64::value(height)),
                            (layout::vocabulary::FUEL, f64::value(fuel)),
                        ]),
                    )]))
                })())
            }),
        )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (id, spelling) in [
        (TOOLPATH, "toolpaths"),
        (START_AT, "start at"),
        (LINE_TO, "line to"),
        (MAP_POINTS, "map points"),
        (MAPPER, "mapping"),
        (POINT, "point"),
        (X, "x"),
        (Y, "y"),
        (Z, "z"),
        (PREVIEW, "preview paths"),
        (PROGRAM, "program"),
        (INVALID_INPUT, "invalid toolpath input"),
        (OUTPUT_REQUIRED, "toolpath output required"),
    ] {
        cells.set_value(id, name::record(spelling, []));
    }
    Library::named(
        ID,
        "toolpaths",
        Definitions::from_parts(cells, functions()),
        crate::display::partial(preview::display),
    )
}
