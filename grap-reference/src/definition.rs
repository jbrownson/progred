//! What Grap's core means: a slow, simple evaluator, written as Verus
//! specification functions, over the GID values of `value`.
//!
//! The core is what Grap's evaluator does itself, not what libraries add:
//! cells; the forms a record's `isa` makes it, calls, lambdas and value
//! wrappers; the closures lambdas come to; the absents all of these come to;
//! where code was written, which closures and some absents carry; fuel; and
//! how deeply evaluations nest. With it come the number operations Grap runs
//! natively, which the numbers library registers as primitives. Calling any
//! other foreign function leaves the core, and evaluation that gets there
//! says so (`Outcome::Beyond`) rather than guessing.
//!
//! Each function returns its outcome and the fuel it spent. Every expression
//! evaluated spends one unit before anything else; evaluation halts when that
//! would leave none, having spent all there was. Spending, rather than
//! returning what's left, is what lets the definition terminate visibly: a
//! caller hands on what's left after what it has spent, never more. An
//! expression is evaluated with the depth evaluations may still nest within
//! it, and one evaluated with none left halts too, once it has spent its unit.

#[cfg(verus_only)]
use crate::value::*;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// What the host implements a foreign function with: an operation Grap runs
/// natively, or code of its own, which the definition leaves out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Implementation {
    Primitive(Primitive),
    Foreign,
}

/// A cell's definition, as the host gives it: its value, and the code it
/// implements under the cell, if any. Referencing the cell evaluates the
/// value. Calling it runs the implementation when the value is a foreign
/// function implemented under the cell itself, and otherwise calls what the
/// value comes to.
#[cfg(verus_only)]
pub struct Definition {
    pub value: Value,
    pub implementation: Option<Implementation>,
}

/// Why an evaluation stopped before finishing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Halt {
    FuelExhausted,
    /// Evaluations nested deeper than `MAX_DEPTH`.
    DepthExhausted,
}

/// How deeply evaluations may nest, as Grap's `MAX_DEPTH` says.
pub const MAX_DEPTH: usize = 256;

/// A number type Grap holds natively: a record whose tag holds the number's
/// little-endian bytes, `{f64: 8 bytes}`, `{f32: 4 bytes}` or `{u64: 8 bytes}`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Number {
    F64,
    F32,
    U64,
}

/// An operation Grap runs natively on the numbers under its labels, which a
/// library registers as a cell's foreign function. Called like any function,
/// it differs only in running the operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(inconsistent_fields)]
pub enum Primitive {
    Binary { operation: BinaryOperation, left: u128, right: u128 },
    Unary { operation: UnaryOperation, operand: u128 },
}

/// The numbers library's operations on two numbers of one type. Each float
/// operation is Rust's, as `float_arithmetic` and `float_comparison` say.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinaryOperation {
    /// f64 `+`.
    F64Sum,
    /// f64 `*`.
    F64Multiply,
    /// f64 `-`.
    F64Subtract,
    /// f64 `/`.
    F64Divide,
    /// `f64::min`.
    F64Min,
    /// `f64::max`.
    F64Max,
    /// `f64::hypot`.
    F64Hypot,
    /// f64 `<`, a boolean.
    F64Less,
    /// f64 `==`, a boolean.
    F64Equal,
    /// f32 `+`.
    F32Sum,
    /// f32 `-`.
    F32Subtract,
    /// f32 `*`.
    F32Multiply,
    /// f32 `/`.
    F32Divide,
    /// f32 `<`, a boolean.
    F32Less,
    /// f32 `==`, a boolean.
    F32Equal,
    /// u64 `+`, failing with the reason when the sum doesn't fit.
    U64Sum(u128),
    /// u64 `-`, failing with the reason below zero.
    U64Subtract(u128),
    /// u64 `*`, failing with the reason when the product doesn't fit.
    U64Multiply(u128),
    /// u64 `/`, rounding down, failing with the reason dividing by zero.
    U64Divide(u128),
    /// u64 `<`, a boolean.
    U64Less,
    /// u64 `==`, a boolean.
    U64Equal,
}

/// The numbers library's operations on one number, as `float_unary` and
/// `float_predicate` say.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnaryOperation {
    /// `f64::sin`.
    F64Sin,
    /// `f64::cos`.
    F64Cos,
    /// `f64::floor`.
    F64Floor,
    /// `f64::ceil`.
    F64Ceil,
    /// `f64::is_finite`, a boolean.
    F64IsFinite,
}

/// What an operation comes to: a number of its operands' type, as its bytes;
/// a boolean; or a failure, as its reason.
#[cfg(verus_only)]
pub enum Answer {
    Number(Seq<u8>),
    Truth(bool),
    Failure(u128),
}


/// A step from code into code within it, as GID paths take them: into a
/// record's field, into a list's element at a position, by its bytes, or
/// through a reference.
#[cfg(verus_only)]
pub enum Step {
    Key(u128),
    Element(Seq<u8>),
    Follow,
}

/// Where an origin's path starts: the input an evaluation was handed, the
/// host's document root, or a cell's definition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Start {
    Input,
    Root,
    Cell(u128),
}

/// Where code was written: where its path starts, and the path from there.
#[cfg(verus_only)]
pub struct Origin {
    pub start: Start,
    pub path: Seq<Step>,
}

/// What evaluating an expression came to.
#[cfg(verus_only)]
pub enum Outcome {
    /// A value; an absent is a value too.
    Done(Value),
    /// Fuel ran out, which halts the whole evaluation.
    OutOfFuel,
    /// Evaluations nested too deeply, which halts the whole evaluation.
    OutOfDepth,
    /// Evaluation called a foreign function, which this definition leaves out.
    Beyond,
}

/// The fuel left after spending some.
pub open spec fn left(fuel: nat, spent: nat) -> nat {
    if spent <= fuel {
        (fuel - spent) as nat
    } else {
        0
    }
}

/// The first definition the host has for `cell`: the document's, then each
/// loaded library's in order.
pub open spec fn resolve(host: Seq<(u128, Definition)>, cell: u128) -> Option<Definition>
    decreases host.len(),
{
    if host.len() == 0 {
        None
    } else if host[0].0 == cell {
        Some(host[0].1)
    } else {
        resolve(host.drop_first(), cell)
    }
}

// Where code was written.

/// The code under `field` of code written at `origin`, which has no origin
/// when that code has none: code a program builds as it runs.
pub open spec fn child(origin: Option<Origin>, field: u128) -> Option<Origin> {
    match origin {
        Some(origin) => Some(Origin { start: origin.start, path: origin.path.push(Step::Key(field)) }),
        None => None,
    }
}

/// Where a cell's definition starts.
pub open spec fn defined_at(cell: u128) -> Origin {
    Origin { start: Start::Cell(cell), path: Seq::empty() }
}

/// Where the input an evaluation was handed starts.
pub open spec fn input() -> Origin {
    Origin { start: Start::Input, path: Seq::empty() }
}

/// A step as GID path data: `{key: cell}`, `{element: {indexable: bytes}}`
/// or `{follow: {}}`.
pub open spec fn step_value(step: Step) -> Value {
    match step {
        Step::Key(cell) => Value::Record(map![STEP_KEY => Value::Cell(cell)]),
        Step::Element(position) => Value::Record(
            map![STEP_ELEMENT => Value::Record(map![STEP_INDEXABLE => Value::Blob(position)])],
        ),
        Step::Follow => Value::Record(map![STEP_FOLLOW => Value::Record(Map::empty())]),
    }
}

/// Steps' values, in order.
pub open spec fn step_values(path: Seq<Step>) -> Seq<Value> {
    Seq::new(path.len(), |i: int| step_value(path[i]))
}

/// A path as GID data: a list of its steps.
pub open spec fn path_value(path: Seq<Step>) -> Value {
    Value::List(step_values(path))
}

/// An origin as GID data: `{input: path}`, `{root: path}` or `{cell, path}`.
pub open spec fn origin_value(origin: Origin) -> Value {
    match origin.start {
        Start::Input => Value::Record(map![ORIGIN_INPUT => path_value(origin.path)]),
        Start::Root => Value::Record(map![ORIGIN_ROOT => path_value(origin.path)]),
        Start::Cell(cell) => Value::Record(
            map![ORIGIN_CELL => Value::Cell(cell), ORIGIN_PATH => path_value(origin.path)],
        ),
    }
}

/// Whether bytes are a GID position's: some, the last not zero.
pub open spec fn position(bytes: Seq<u8>) -> bool {
    bytes.len() > 0 && bytes.last() != 0
}

/// The step GID path data is: one with exactly one of `key`, `element` and
/// `follow`, a key naming a cell and an element holding a position.
pub open spec fn read_step(value: Value) -> Option<Step> {
    match value {
        Value::Record(fields) => {
            let key = fields.contains_key(STEP_KEY);
            let element = fields.contains_key(STEP_ELEMENT);
            let follow = fields.contains_key(STEP_FOLLOW);
            if key && !element && !follow {
                match fields[STEP_KEY] {
                    Value::Cell(cell) => Some(Step::Key(cell)),
                    _ => None,
                }
            } else if !key && element && !follow {
                match fields[STEP_ELEMENT] {
                    Value::Record(element) => if element.contains_key(STEP_INDEXABLE) {
                        match element[STEP_INDEXABLE] {
                            Value::Blob(bytes) => if position(bytes) {
                                Some(Step::Element(bytes))
                            } else {
                                None
                            },
                            _ => None,
                        }
                    } else {
                        None
                    },
                    _ => None,
                }
            } else if !key && !element && follow {
                Some(Step::Follow)
            } else {
                None
            }
        },
        _ => None,
    }
}

/// The steps values are, when each is one.
pub open spec fn read_steps(values: Seq<Value>) -> Option<Seq<Step>>
    decreases values.len(),
{
    if values.len() == 0 {
        Some(Seq::empty())
    } else {
        match read_steps(values.drop_last()) {
            Some(steps) => match read_step(values.last()) {
                Some(step) => Some(steps.push(step)),
                None => None,
            },
            None => None,
        }
    }
}

/// The path GID data is: a list of steps.
pub open spec fn read_path(value: Value) -> Option<Seq<Step>> {
    match value {
        Value::List(values) => read_steps(values),
        _ => None,
    }
}

/// The origin GID data is: one with exactly one of `input`, `root` and
/// `cell`, holding a path, or with a cell, naming it and with its `path`.
pub open spec fn read_origin(value: Value) -> Option<Origin> {
    match value {
        Value::Record(fields) => {
            let input = fields.contains_key(ORIGIN_INPUT);
            let root = fields.contains_key(ORIGIN_ROOT);
            let cell = fields.contains_key(ORIGIN_CELL);
            if input && !root && !cell {
                match read_path(fields[ORIGIN_INPUT]) {
                    Some(path) => Some(Origin { start: Start::Input, path }),
                    None => None,
                }
            } else if !input && root && !cell {
                match read_path(fields[ORIGIN_ROOT]) {
                    Some(path) => Some(Origin { start: Start::Root, path }),
                    None => None,
                }
            } else if !input && !root && cell && fields.contains_key(ORIGIN_PATH) {
                match fields[ORIGIN_CELL] {
                    Value::Cell(cell) => match read_path(fields[ORIGIN_PATH]) {
                        Some(path) => Some(Origin { start: Start::Cell(cell), path }),
                        None => None,
                    },
                    _ => None,
                }
            } else {
                None
            }
        },
        _ => None,
    }
}

/// `{call: origin}`, where the call an absent is about is, when it has an
/// origin, and nothing when it doesn't.
pub open spec fn called(call: Option<Origin>) -> Seq<(u128, Value)> {
    match call {
        Some(call) => seq![(ABSENT_CALL, origin_value(call))],
        None => Seq::empty(),
    }
}

// The absents the core's forms come to.

/// `{isa: absent, reason: missing cell, cell}`.
pub open spec fn missing_cell(cell: u128) -> Value {
    with_detail(MISSING_CELL, ABSENT_CELL, Value::Cell(cell))
}

/// Why a call has no argument under `parameter`: carrying where the call
/// is, when it has an origin.
pub open spec fn missing_argument(parameter: u128, call: Option<Origin>) -> Value {
    absent(MISSING_ARGUMENT, seq![(ABSENT_CELL, Value::Cell(parameter))] + called(call))
}

/// Why a form can't be used while it lacks `field`, which its kind
/// requires.
pub open spec fn missing_field(field: u128) -> Value {
    with_detail(MISSING_FIELD, ABSENT_CELL, Value::Cell(field))
}

/// Why `callable` can't be called: an absent `callable` as it is, since what
/// failed before it is the reason.
pub open spec fn not_callable(callable: Value) -> Value {
    if is_absent(callable) {
        callable
    } else {
        with_detail(NOT_CALLABLE, ABSENT_VALUE, callable)
    }
}

/// The code under a field a form's kind requires: what's there, or, while
/// it's missing, why, which evaluates to itself.
pub open spec fn required(fields: Map<u128, Value>, field: u128) -> Value {
    if fields.contains_key(field) {
        fields[field]
    } else {
        missing_field(field)
    }
}

/// A lambda's parameters, when each is a cell, or the first that isn't.
pub open spec fn parameters(params: Seq<Value>) -> Result<Seq<u128>, Value>
    decreases params.len(),
{
    if params.len() == 0 {
        Ok(Seq::empty())
    } else {
        match params[0] {
            Value::Cell(cell) => match parameters(params.drop_first()) {
                Ok(cells) => Ok(seq![cell] + cells),
                Err(first) => Err(first),
            },
            first => Err(first),
        }
    }
}

/// A closure: `{isa: closure, lambda, environment}`, with where its body was
/// written under `body origin` when it was.
pub open spec fn closure_record(
    lambda: Map<u128, Value>,
    environment: Map<u128, Value>,
    body_origin: Option<Origin>,
) -> Value {
    let fields = map![ISA => Value::Cell(KIND_CLOSURE), LAMBDA => Value::Record(lambda), ENVIRONMENT => Value::Record(environment)];
    match body_origin {
        Some(origin) => Value::Record(fields.insert(BODY_ORIGIN, origin_value(origin))),
        None => Value::Record(fields),
    }
}

/// A lambda `{isa: lambda, params, body, ...}`, written at `origin`,
/// evaluated in `environment`: a closure capturing the environment, holding
/// the lambda as written, its body why it's missing if it is. Parameters
/// that aren't a list, or include something that isn't a cell, come to an
/// absent instead.
pub open spec fn lambda(
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
) -> Value {
    let malformed = with_detail(MALFORMED_LAMBDA, ABSENT_VALUE, Value::Record(fields));
    if fields.contains_key(PARAMS) {
        match fields[PARAMS] {
            Value::List(params) => match parameters(params) {
                Ok(_) => closure_record(
                    fields.insert(BODY, required(fields, BODY)),
                    environment,
                    child(origin, BODY),
                ),
                Err(parameter) => with_detail(INVALID_PARAMETER, ABSENT_VALUE, parameter),
            },
            _ => malformed,
        }
    } else {
        malformed
    }
}

/// Where a closure's body was written, when its record says so legibly.
pub open spec fn body_origin(fields: Map<u128, Value>) -> Option<Origin> {
    if fields.contains_key(BODY_ORIGIN) {
        read_origin(fields[BODY_ORIGIN])
    } else {
        None
    }
}

/// A closure's parameters, body, where its body was written and captured
/// environment, when `callable` is a closure record: its lambda with a list
/// of cells for parameters, and a body, and a record for its environment.
pub open spec fn closure(callable: Value) -> Option<(Seq<u128>, Value, Option<Origin>, Map<u128, Value>)> {
    match callable {
        Value::Record(fields) => if kind(fields) == Some(KIND_CLOSURE) && fields.contains_key(LAMBDA)
            && fields.contains_key(ENVIRONMENT) {
            match fields[LAMBDA] {
                Value::Record(lambda) => match fields[ENVIRONMENT] {
                    Value::Record(captured) => if lambda.contains_key(PARAMS) && lambda.contains_key(
                        BODY,
                    ) {
                        match lambda[PARAMS] {
                            Value::List(params) => match parameters(params) {
                                Ok(cells) => Some((cells, lambda[BODY], body_origin(fields), captured)),
                                Err(_) => None,
                            },
                            _ => None,
                        }
                    } else {
                        None
                    },
                    _ => None,
                },
                _ => None,
            }
        } else {
            None
        },
        _ => None,
    }
}

/// The cell a foreign function is implemented under, when `value` is one:
/// `{isa: foreign function, implementation: cell}`.
pub open spec fn implementation_cell(value: Value) -> Option<u128> {
    match value {
        Value::Record(fields) => if kind(fields) == Some(KIND_FOREIGN_FUNCTION) && fields.contains_key(
            IMPLEMENTATION,
        ) {
            match fields[IMPLEMENTATION] {
                Value::Cell(cell) => Some(cell),
                _ => None,
            }
        } else {
            None
        },
        _ => None,
    }
}

/// The cell calling `callable` calls: the cell it is, or the one a foreign
/// function is implemented under.
pub open spec fn callee_cell(callable: Value) -> Option<u128> {
    match callable {
        Value::Cell(cell) => Some(cell),
        _ => implementation_cell(callable),
    }
}

// Evaluation.

/// Evaluating `expression`, written at `origin`, in `environment`: one unit
/// of fuel, then, with evaluations allowed to nest one less deep within it,
/// by its form. A cell is a reference. A record is what its `isa` makes it:
/// a call, a lambda, or a value wrapper, whose value is evaluated in the same
/// environment; a record of any other kind, or of none, and anything else,
/// are data, their own value.
pub open spec fn eval(
    expression: Value,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 0nat, 0nat,
{
    if fuel < 2 {
        (Outcome::OutOfFuel, fuel)
    } else if depth == 0 {
        (Outcome::OutOfDepth, 1)
    } else {
        let rest = (fuel - 1) as nat;
        let within = (depth - 1) as nat;
        let (outcome, spent) = match expression {
            Value::Cell(cell) => reference(cell, environment, host, rest, within),
            Value::Record(fields) => if kind(fields) == Some(KIND_CALL) {
                call(fields, origin, environment, host, rest, within)
            } else if kind(fields) == Some(KIND_LAMBDA) {
                (Outcome::Done(lambda(fields, origin, environment)), 0)
            } else if kind(fields) == Some(KIND_VALUE_WRAPPER) {
                eval(required(fields, VALUE), child(origin, VALUE), environment, host, rest, within)
            } else {
                (Outcome::Done(expression), 0)
            },
            _ => (Outcome::Done(expression), 0),
        };
        (outcome, spent + 1)
    }
}

/// A reference to `cell`: its lexical binding; otherwise its definition's
/// value, evaluated where it's referenced.
pub open spec fn reference(
    cell: u128,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 1nat, 0nat,
{
    if environment.contains_key(cell) {
        (Outcome::Done(environment[cell]), 0)
    } else {
        match resolve(host, cell) {
            None => (Outcome::Done(missing_cell(cell)), 0),
            Some(definition) => eval(
                definition.value,
                Some(defined_at(cell)),
                environment,
                host,
                fuel,
                depth,
            ),
        }
    }
}

/// A call `{isa: call, function, ...arguments}`, written at `origin`. A
/// function that's a cell without a lexical binding calls the cell; one with
/// a binding spends a unit of fuel and applies the bound value; any other is
/// evaluated and its value applied. A missing function is why it's missing,
/// which isn't callable.
pub open spec fn call(
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 4nat, 0nat,
{
    match required(fields, FUNCTION) {
        Value::Cell(cell) => if environment.contains_key(cell) {
            if fuel < 2 {
                (Outcome::OutOfFuel, fuel)
            } else {
                let (outcome, spent) = apply(
                    environment[cell],
                    fields,
                    origin,
                    environment,
                    host,
                    (fuel - 1) as nat,
                    depth,
                );
                (outcome, spent + 1)
            }
        } else {
            call_cell(cell, fields, origin, environment, host, fuel, depth)
        },
        function => {
            let (outcome, spent) = eval(
                function,
                child(origin, FUNCTION),
                environment,
                host,
                fuel,
                depth,
            );
            match outcome {
                Outcome::Done(callable) => {
                    let (applied, more) = apply(
                        callable,
                        fields,
                        origin,
                        environment,
                        host,
                        left(fuel, spent),
                        depth,
                    );
                    (applied, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
    }
}

/// Calling `cell`: a unit of fuel, then its definition. A value that's a
/// foreign function implemented under the cell runs what the host
/// implements there: a primitive's operation, or code that leaves the core;
/// without any, it isn't callable. Any other value is called as code.
pub open spec fn call_cell(
    cell: u128,
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 2nat, 0nat,
{
    if fuel < 2 {
        (Outcome::OutOfFuel, fuel)
    } else {
        let rest = (fuel - 1) as nat;
        let (outcome, spent) = match resolve(host, cell) {
            None => (Outcome::Done(missing_cell(cell)), 0),
            Some(definition) => if implementation_cell(definition.value) == Some(cell) {
                match definition.implementation {
                    Some(Implementation::Primitive(primitive)) => primitive_call(
                        primitive,
                        fields,
                        origin,
                        environment,
                        host,
                        rest,
                        depth,
                    ),
                    Some(Implementation::Foreign) => (Outcome::Beyond, 0),
                    None => (Outcome::Done(not_callable(definition.value)), 0),
                }
            } else {
                call_code(definition.value, cell, fields, origin, environment, host, rest, depth)
            },
        };
        (outcome, spent + 1)
    }
}

/// Calling `cell`, whose definition is code, `value`: it's evaluated where
/// the call is, and applied unless it comes to an absent, which is the
/// call's result as it is.
pub open spec fn call_code(
    value: Value,
    cell: u128,
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 5nat, 0nat,
{
    let (evaluated, spent) = eval(value, Some(defined_at(cell)), environment, host, fuel, depth);
    match evaluated {
        Outcome::Done(callable) => if is_absent(callable) {
            (Outcome::Done(callable), spent)
        } else {
            let (applied, more) = apply(
                callable,
                fields,
                origin,
                environment,
                host,
                left(fuel, spent),
                depth,
            );
            (applied, spent + more)
        },
        stopped => (stopped, spent),
    }
}

/// Applying a callable to a call's arguments: a closure binds its parameters
/// and evaluates its body; a value naming a cell calls the cell; anything
/// else isn't callable.
pub open spec fn apply(
    callable: Value,
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 3nat, 0nat,
{
    match closure(callable) {
        Some((params, body, body_origin, captured)) => bind(
            params,
            body,
            body_origin,
            captured,
            fields,
            origin,
            environment,
            host,
            fuel,
            depth,
        ),
        None => match callee_cell(callable) {
            Some(cell) => call_cell(cell, fields, origin, environment, host, fuel, depth),
            None => (Outcome::Done(not_callable(callable)), 0),
        },
    }
}

/// Binding a closure's parameters in order, each to the call's argument
/// under it, evaluated where the call is, or, missing, to its absence; then
/// evaluating its body in what it captured, with those bindings. A later
/// parameter shadows an earlier one of the same cell.
pub open spec fn bind(
    params: Seq<u128>,
    body: Value,
    body_origin: Option<Origin>,
    captured: Map<u128, Value>,
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 2nat, params.len(),
{
    if params.len() == 0 {
        eval(body, body_origin, captured, host, fuel, depth)
    } else {
        let parameter = params[0];
        if fields.contains_key(parameter) {
            let (evaluated, spent) = eval(
                fields[parameter],
                child(origin, parameter),
                environment,
                host,
                fuel,
                depth,
            );
            match evaluated {
                Outcome::Done(argument) => {
                    let (outcome, more) = bind(
                        params.drop_first(),
                        body,
                        body_origin,
                        captured.insert(parameter, argument),
                        fields,
                        origin,
                        environment,
                        host,
                        left(fuel, spent),
                        depth,
                    );
                    (outcome, spent + more)
                },
                stopped => (stopped, spent),
            }
        } else {
            bind(
                params.drop_first(),
                body,
                body_origin,
                captured.insert(parameter, missing_argument(parameter, origin)),
                fields,
                origin,
                environment,
                host,
                fuel,
                depth,
            )
        }
    }
}

/// A primitive's operand: the call's argument under `label`, evaluated where
/// the call is, or, missing, its absence, at no cost.
pub open spec fn operand(
    label: u128,
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 1nat, 0nat,
{
    if fields.contains_key(label) {
        eval(fields[label], child(origin, label), environment, host, fuel, depth)
    } else {
        (Outcome::Done(missing_argument(label, origin)), 0)
    }
}

/// Calling a primitive: its operands in order, then its operation on them.
/// Halting or leaving the core on an operand stops the call there.
pub open spec fn primitive_call(
    primitive: Primitive,
    fields: Map<u128, Value>,
    origin: Option<Origin>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    fuel: nat,
    depth: nat,
) -> (Outcome, nat)
    decreases fuel, 2nat, 0nat,
{
    match primitive {
        Primitive::Binary { operation, left: first_label, right: second_label } => {
            let (first, spent) = operand(
                first_label,
                fields,
                origin,
                environment,
                host,
                fuel,
                depth,
            );
            match first {
                Outcome::Done(first) => {
                    let (second, more) = operand(
                        second_label,
                        fields,
                        origin,
                        environment,
                        host,
                        left(fuel, spent),
                        depth,
                    );
                    match second {
                        Outcome::Done(second) => (
                            Outcome::Done(
                                binary(operation, origin, first_label, first, second_label, second),
                            ),
                            spent + more,
                        ),
                        stopped => (stopped, spent + more),
                    }
                },
                stopped => (stopped, spent),
            }
        },
        Primitive::Unary { operation, operand: label } => {
            let (value, spent) = operand(label, fields, origin, environment, host, fuel, depth);
            match value {
                Outcome::Done(value) => (Outcome::Done(unary(operation, origin, label, value)), spent),
                stopped => (stopped, spent),
            }
        },
    }
}

// Numbers.

/// A binary operation on its operands' values, under their labels, for a
/// call written at `call`: its answer when both are numbers of its type,
/// and otherwise "invalid arguments", carrying why each that isn't couldn't
/// be used.
pub open spec fn binary(
    operation: BinaryOperation,
    call: Option<Origin>,
    first_label: u128,
    first: Value,
    second_label: u128,
    second: Value,
) -> Value {
    let number = binary_number(operation);
    match (number_bytes(number, first), number_bytes(number, second)) {
        (Some(left), Some(right)) => answer_value(number, binary_answer(operation, left, right)),
        (Some(_), None) => caused_by_all(
            INVALID_ARGUMENTS,
            call,
            seq![(second_label, not_a_number(number, second))],
        ),
        (None, Some(_)) => caused_by_all(
            INVALID_ARGUMENTS,
            call,
            seq![(first_label, not_a_number(number, first))],
        ),
        (None, None) => caused_by_all(
            INVALID_ARGUMENTS,
            call,
            seq![
                (first_label, not_a_number(number, first)),
                (second_label, not_a_number(number, second)),
            ],
        ),
    }
}

/// A unary operation on its operand's value, under its label, for a call
/// written at `call`.
pub open spec fn unary(operation: UnaryOperation, call: Option<Origin>, label: u128, value: Value) -> Value {
    let number = unary_number(operation);
    match number_bytes(number, value) {
        Some(bytes) => answer_value(number, unary_answer(operation, bytes)),
        None => caused_by_all(INVALID_ARGUMENTS, call, seq![(label, not_a_number(number, value))]),
    }
}

/// The field a number of the type holds its bytes under.
pub open spec fn tag(number: Number) -> u128 {
    match number {
        Number::F64 => F64,
        Number::F32 => F32,
        Number::U64 => U64,
    }
}

/// How many bytes a number of the type holds.
pub open spec fn width(number: Number) -> nat {
    match number {
        Number::F32 => 4,
        _ => 8,
    }
}

/// Why a value isn't a number of the type.
pub open spec fn not_reason(number: Number) -> u128 {
    match number {
        Number::F64 => NOT_F64,
        Number::F32 => NOT_F32,
        Number::U64 => NOT_U64,
    }
}

/// The bytes of the `number` that `value` is: a record whose tag holds a blob
/// of the type's width. Its other fields don't matter, and nor does its also
/// being an absent.
pub open spec fn number_bytes(number: Number, value: Value) -> Option<Seq<u8>> {
    match value {
        Value::Record(fields) => if fields.contains_key(tag(number)) {
            match fields[tag(number)] {
                Value::Blob(bytes) => if bytes.len() == width(number) {
                    Some(bytes)
                } else {
                    None
                },
                _ => None,
            }
        } else {
            None
        },
        _ => None,
    }
}

/// Why `value` isn't a `number`: an absent says why already, and passes on
/// as it is; anything else is `{isa: absent, reason: not <type>, value}`.
pub open spec fn not_a_number(number: Number, value: Value) -> Value {
    if is_absent(value) {
        value
    } else {
        with_detail(not_reason(number), ABSENT_VALUE, value)
    }
}

/// An answer as a value: `{<type>: bytes}`, `{bool: true}` or `{bool:
/// false}`, or `{isa: absent, reason}`.
pub open spec fn answer_value(number: Number, answer: Answer) -> Value {
    match answer {
        Answer::Number(bytes) => Value::Record(map![tag(number) => Value::Blob(bytes)]),
        Answer::Truth(truth) => Value::Record(
            map![BOOL => Value::Cell(if truth { TRUE } else { FALSE })],
        ),
        Answer::Failure(reason) => absent(reason, Seq::empty()),
    }
}

/// The type of a binary operation's operands, and of a number it answers.
pub open spec fn binary_number(operation: BinaryOperation) -> Number {
    match operation {
        BinaryOperation::F64Sum
        | BinaryOperation::F64Multiply
        | BinaryOperation::F64Subtract
        | BinaryOperation::F64Divide
        | BinaryOperation::F64Min
        | BinaryOperation::F64Max
        | BinaryOperation::F64Hypot
        | BinaryOperation::F64Less
        | BinaryOperation::F64Equal => Number::F64,
        BinaryOperation::F32Sum
        | BinaryOperation::F32Subtract
        | BinaryOperation::F32Multiply
        | BinaryOperation::F32Divide
        | BinaryOperation::F32Less
        | BinaryOperation::F32Equal => Number::F32,
        _ => Number::U64,
    }
}

/// The type of a unary operation's operand, and of a number it answers:
/// every one the library registers is on f64s.
pub open spec fn unary_number(operation: UnaryOperation) -> Number {
    Number::F64
}

/// What a binary operation comes to on its operands' bytes.
pub open spec fn binary_answer(operation: BinaryOperation, left: Seq<u8>, right: Seq<u8>) -> Answer {
    match operation {
        BinaryOperation::U64Sum(overflow) => checked((le(left) + le(right)) as int, overflow),
        BinaryOperation::U64Subtract(overflow) => checked(le(left) - le(right), overflow),
        BinaryOperation::U64Multiply(overflow) => checked((le(left) * le(right)) as int, overflow),
        BinaryOperation::U64Divide(division_by_zero) => if le(right) == 0 {
            Answer::Failure(division_by_zero)
        } else {
            Answer::Number(le_bytes(le(left) / le(right), 8))
        },
        BinaryOperation::U64Less => Answer::Truth(le(left) < le(right)),
        BinaryOperation::U64Equal => Answer::Truth(le(left) == le(right)),
        BinaryOperation::F64Less
        | BinaryOperation::F64Equal
        | BinaryOperation::F32Less
        | BinaryOperation::F32Equal => Answer::Truth(float_comparison(operation, left, right)),
        _ => Answer::Number(float_arithmetic(operation, left, right)),
    }
}

/// What a unary operation comes to on its operand's bytes.
pub open spec fn unary_answer(operation: UnaryOperation, operand: Seq<u8>) -> Answer {
    match operation {
        UnaryOperation::F64IsFinite => Answer::Truth(float_predicate(operation, operand)),
        _ => Answer::Number(float_unary(operation, operand)),
    }
}

/// `n` as a u64, when it is one, or the `reason` it isn't.
pub open spec fn checked(n: int, reason: u128) -> Answer {
    if 0 <= n < 0x1_0000_0000_0000_0000 {
        Answer::Number(le_bytes(n as nat, 8))
    } else {
        Answer::Failure(reason)
    }
}

/// The number little-endian `bytes` hold.
pub open spec fn le(bytes: Seq<u8>) -> nat
    decreases bytes.len(),
{
    if bytes.len() == 0 {
        0
    } else {
        bytes[0] as nat + 256 * le(bytes.drop_first())
    }
}

/// `n` as `width` little-endian bytes.
pub open spec fn le_bytes(n: nat, width: nat) -> Seq<u8>
    decreases width,
{
    if width == 0 {
        Seq::empty()
    } else {
        seq![(n % 256) as u8] + le_bytes(n / 256, (width - 1) as nat)
    }
}

// The float operations, as Rust computes them on the platform the evaluator
// runs on, on and to numbers' bytes. IEEE 754 defines `+`, `-`, `*`, `/`,
// `floor`, `ceil` and comparisons exactly, but for a NaN's sign and payload,
// which Rust leaves to the platform, as it does which of two equal zeros
// `min` and `max` answer and how closely `hypot`, `sin` and `cos` round. So
// the definition names these operations rather than defining them: the
// trusted functions in `number` compute them, and the definition holds Grap
// to whatever those compute.

/// A float binary operation's result.
pub uninterp spec fn float_arithmetic(
    operation: BinaryOperation,
    left: Seq<u8>,
    right: Seq<u8>,
) -> Seq<u8>;

/// A float comparison's answer.
pub uninterp spec fn float_comparison(
    operation: BinaryOperation,
    left: Seq<u8>,
    right: Seq<u8>,
) -> bool;

/// A float unary operation's result.
pub uninterp spec fn float_unary(operation: UnaryOperation, operand: Seq<u8>) -> Seq<u8>;

/// A float predicate's answer.
pub uninterp spec fn float_predicate(operation: UnaryOperation, operand: Seq<u8>) -> bool;

// Absents caused by others.

/// The longest chain of causes an absent grows: past it, an absent passes on
/// unchanged instead of gaining another layer.
pub const CHAIN_LIMIT: usize = 8;

/// An absent for `reason`, about a call written at `call`, carrying each
/// detail under its field: its causes are those that are absent. An absent
/// the same as one kept before it isn't repeated. When the longest chain of
/// causes among them holds `CHAIN_LIMIT` absents or more, that detail passes
/// on unchanged instead: the last of the longest.
pub open spec fn caused_by_all(
    reason: u128,
    call: Option<Origin>,
    details: Seq<(u128, Value)>,
) -> Value {
    let kept = distinct(details);
    if kept.len() > 0 && chain_at_least(kept[longest(kept)].1, CHAIN_LIMIT as nat) {
        kept[longest(kept)].1
    } else {
        absent(reason, called(call) + kept)
    }
}

/// The details, in order, without an absent the same as one kept before it.
pub open spec fn distinct(details: Seq<(u128, Value)>) -> Seq<(u128, Value)>
    decreases details.len(),
{
    if details.len() == 0 {
        details
    } else {
        let kept = distinct(details.drop_last());
        let detail = details.last().1;
        if is_absent(detail) && exists|i: int| 0 <= i < kept.len() && (#[trigger] kept[i]).1 == detail {
            kept
        } else {
            kept.push(details.last())
        }
    }
}

/// Where among `details` the longest chain of causes is: the last of those
/// as long as any.
pub open spec fn longest(details: Seq<(u128, Value)>) -> int
    decreases details.len(),
{
    if details.len() <= 1 {
        0
    } else {
        let before = longest(details.drop_last());
        if as_long(details.last().1, details[before].1) {
            details.len() - 1
        } else {
            before
        }
    }
}

/// Whether `value`'s longest chain of causes is at least as long as
/// `other`'s.
pub open spec fn as_long(value: Value, other: Value) -> bool {
    forall|n: nat| #[trigger] chain_at_least(other, n) ==> chain_at_least(value, n)
}

/// Whether `value`'s causes hold a chain of at least `n` absents: none
/// trivially; otherwise `value` is absent, and, past one, a detail of it
/// holds the rest. Its kind and reason are cells, never absent, so only its
/// causes can.
pub open spec fn chain_at_least(value: Value, n: nat) -> bool
    decreases n,
{
    if n == 0 {
        true
    } else {
        match value {
            Value::Record(fields) => is_absent(value) && (n == 1 || exists|field: u128|
                #[trigger] fields.contains_key(field) && chain_at_least(
                    fields[field],
                    (n - 1) as nat,
                )),
            _ => false,
        }
    }
}

/// A program, handed to an evaluation as its input, evaluated with `fuel`:
/// its result, the fuel left, and why it halted, if it did; or `None` when
/// it calls a foreign function. A halted program comes to the halt's absent,
/// `{isa: absent, reason: fuel exhausted}` with no fuel left, or `{isa:
/// absent, reason: depth exhausted}`.
pub open spec fn evaluate(expression: Value, host: Seq<(u128, Definition)>, fuel: nat) -> Option<
    (Value, nat, Option<Halt>),
> {
    let (outcome, spent) = eval(
        expression,
        Some(input()),
        Map::empty(),
        host,
        fuel,
        MAX_DEPTH as nat,
    );
    match outcome {
        Outcome::Done(result) => Some((result, left(fuel, spent), None)),
        Outcome::OutOfFuel => Some((absent(FUEL_EXHAUSTED, Seq::empty()), 0, Some(Halt::FuelExhausted))),
        Outcome::OutOfDepth => Some(
            (absent(DEPTH_EXHAUSTED, Seq::empty()), left(fuel, spent), Some(Halt::DepthExhausted)),
        ),
        Outcome::Beyond => None,
    }
}

} // verus!
