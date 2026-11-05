//! What Grap's core means: a slow, simple evaluator, written as Verus
//! specification functions, over the GID values of `value`.
//!
//! The core is what Grap's evaluator does itself, not what libraries add:
//! cells, lambdas, calls, value wrappers, closures, the absents those come to,
//! and fuel; and the number operations it runs natively, which the numbers
//! library registers as primitives. Calling any other foreign function leaves
//! the core, and evaluation that gets there says so (`Outcome::Beyond`) rather
//! than guessing.
//!
//! Each function returns its outcome and the fuel it spent. Every expression
//! evaluated spends one unit before anything else; evaluation halts when that
//! would leave none, having spent all there was. Spending, rather than
//! returning what's left, is what lets the definition terminate visibly: a
//! caller hands on what's left after what it has spent, never more.

#[cfg(verus_only)]
use crate::value::*;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A cell's definition, as the host resolves it: Grap's own, a foreign
/// function's, or a primitive's. A function's descriptive value is what
/// referencing its cell evaluates; calling a foreign function leaves the core,
/// while calling a primitive runs its operation.
#[cfg(verus_only)]
pub enum Definition {
    Grap(Value),
    Foreign(Value),
    Primitive(Value, Primitive),
}

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

/// How a cell under resolution was reached. A reference notices a cycle
/// only through another reference to the same cell, a call through anything
/// under resolution; the evaluator detects them differently, and this says
/// what it does.
pub enum Reached {
    Reference,
    Call,
}

/// What evaluating an expression came to.
#[cfg(verus_only)]
pub enum Outcome {
    /// A value; an absent is a value too.
    Done(Value),
    /// Fuel ran out, which halts the whole evaluation.
    OutOfFuel,
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

/// The value a definition is evaluated as.
pub open spec fn definition_value(definition: Definition) -> Value {
    match definition {
        Definition::Grap(value) => value,
        Definition::Foreign(description) => description,
        Definition::Primitive(description, _) => description,
    }
}

/// Where `cell` first went under resolution, however it was reached.
pub open spec fn first_resolving(stack: Seq<(u128, Reached)>, cell: u128) -> Option<int>
    decreases stack.len(),
{
    if stack.len() == 0 {
        None
    } else if stack[0].0 == cell {
        Some(0)
    } else {
        match first_resolving(stack.drop_first(), cell) {
            Some(at) => Some(at + 1),
            None => None,
        }
    }
}

/// Where a reference first put `cell` under resolution.
pub open spec fn referenced_at(stack: Seq<(u128, Reached)>, cell: u128) -> Option<int>
    decreases stack.len(),
{
    if stack.len() == 0 {
        None
    } else if stack[0].0 == cell && stack[0].1 is Reference {
        Some(0)
    } else {
        match referenced_at(stack.drop_first(), cell) {
            Some(at) => Some(at + 1),
            None => None,
        }
    }
}

/// The cells under resolution from `at` on.
pub open spec fn resolving_cells(stack: Seq<(u128, Reached)>, at: int) -> Seq<Value> {
    Seq::new((stack.len() - at) as nat, |i: int| Value::Cell(stack[at + i].0))
}

/// `{absent: cell cycle, cycle: [...]}`: the cells under resolution from
/// `at` on, then `cell` again.
pub open spec fn cycle(stack: Seq<(u128, Reached)>, at: int, cell: u128) -> Value {
    absent(CELL_CYCLE, CYCLE, Value::List(resolving_cells(stack, at).push(Value::Cell(cell))))
}

pub open spec fn missing_cell(cell: u128) -> Value {
    absent(MISSING_CELL, ABSENT_CELL, Value::Cell(cell))
}

pub open spec fn missing_argument(parameter: u128) -> Value {
    absent(MISSING_ARGUMENT, ABSENT_CELL, Value::Cell(parameter))
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

/// `{params, body, ...}` evaluated in `environment`: a closure capturing the
/// environment, the record `{closure: {params, body, ..., environment}}`,
/// which keeps the lambda's other fields. Parameters that aren't a list, or
/// include something that isn't a cell, come to an absent instead.
pub open spec fn lambda(fields: Map<u128, Value>, environment: Map<u128, Value>) -> Value {
    match fields[PARAMS] {
        Value::List(params) => match parameters(params) {
            Ok(_) => Value::Record(
                map![CLOSURE => Value::Record(fields.remove(BODY_ORIGIN).insert(ENVIRONMENT, Value::Record(environment)))],
            ),
            Err(parameter) => absent(INVALID_PARAMETER, ABSENT_VALUE, parameter),
        },
        _ => absent(MALFORMED_LAMBDA, ABSENT_VALUE, Value::Record(fields)),
    }
}

/// A closure record's parameters, body and captured environment, when
/// `callable` is one: `{closure: {params: [cells], body, environment: {...}}}`.
pub open spec fn closure(callable: Value) -> Option<(Seq<u128>, Value, Map<u128, Value>)> {
    match callable {
        Value::Record(outer) => if outer.contains_key(CLOSURE) {
            match outer[CLOSURE] {
                Value::Record(fields) => if fields.contains_key(PARAMS) && fields.contains_key(BODY)
                    && fields.contains_key(ENVIRONMENT) {
                    match (fields[PARAMS], fields[ENVIRONMENT]) {
                        (Value::List(params), Value::Record(captured)) => match parameters(params) {
                            Ok(cells) => Some((cells, fields[BODY], captured)),
                            Err(_) => None,
                        },
                        _ => None,
                    }
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

/// The cell a callable names, as a bare cell or as `{ffi: cell}`.
pub open spec fn named_cell(callable: Value) -> Option<u128> {
    match callable {
        Value::Cell(cell) => Some(cell),
        Value::Record(fields) => if fields.contains_key(FFI) {
            match fields[FFI] {
                Value::Cell(cell) => Some(cell),
                _ => None,
            }
        } else {
            None
        },
        _ => None,
    }
}

/// Evaluating `expression` in `environment`: one unit of fuel, then by its
/// form. A record with a `function` is a call; one with `params` and `body`,
/// a lambda; one with a `value`, that value's expression, in the same
/// environment. Anything else is data, its own value.
pub open spec fn eval(
    expression: Value,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 0nat, 0nat,
{
    if fuel < 2 {
        (Outcome::OutOfFuel, fuel)
    } else {
        let rest = (fuel - 1) as nat;
        let (outcome, spent) = match expression {
            Value::Cell(cell) => reference(cell, environment, host, stack, rest),
            Value::Record(fields) => if fields.contains_key(FUNCTION) {
                call(fields, environment, host, stack, rest)
            } else if fields.contains_key(PARAMS) && fields.contains_key(BODY) {
                (Outcome::Done(lambda(fields, environment)), 0)
            } else if fields.contains_key(VALUE) {
                eval(fields[VALUE], environment, host, stack, rest)
            } else {
                (Outcome::Done(expression), 0)
            },
            _ => (Outcome::Done(expression), 0),
        };
        (outcome, spent + 1)
    }
}

/// A reference to `cell`: its lexical binding; otherwise its definition,
/// evaluated where it's referenced, unless a reference already has the cell
/// under resolution.
pub open spec fn reference(
    cell: u128,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 1nat, 0nat,
{
    if environment.contains_key(cell) {
        (Outcome::Done(environment[cell]), 0)
    } else {
        match resolve(host, cell) {
            None => (Outcome::Done(missing_cell(cell)), 0),
            Some(definition) => match referenced_at(stack, cell) {
                Some(at) => (Outcome::Done(cycle(stack, at, cell)), 0),
                None => eval(
                    definition_value(definition),
                    environment,
                    host,
                    stack.push((cell, Reached::Reference)),
                    fuel,
                ),
            },
        }
    }
}

/// A call `{function, ...arguments}`. A function that's a cell without a
/// lexical binding calls through the cell's definition; one with a binding
/// spends a unit of fuel and applies the bound value; any other is evaluated
/// and its value applied.
pub open spec fn call(
    fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 4nat, 0nat,
{
    match fields[FUNCTION] {
        Value::Cell(cell) => if environment.contains_key(cell) {
            if fuel < 2 {
                (Outcome::OutOfFuel, fuel)
            } else {
                let (outcome, spent) = apply(
                    environment[cell],
                    fields,
                    environment,
                    host,
                    stack,
                    (fuel - 1) as nat,
                );
                (outcome, spent + 1)
            }
        } else {
            through_cell(cell, fields, environment, host, stack, fuel)
        },
        function => {
            let (outcome, spent) = eval(function, environment, host, stack, fuel);
            match outcome {
                Outcome::Done(callable) => {
                    let (applied, more) = apply(
                        callable,
                        fields,
                        environment,
                        host,
                        stack,
                        left(fuel, spent),
                    );
                    (applied, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
    }
}

/// Calling through `cell`: a unit of fuel, then its definition. A foreign
/// function's leaves the core, and a primitive's runs. Grap's is evaluated
/// where the call is, unless anything already has the cell under
/// resolution, and is applied unless it comes to an absent, which is the
/// call's result as it is.
pub open spec fn through_cell(
    cell: u128,
    call_fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 2nat, 0nat,
{
    if fuel < 2 {
        (Outcome::OutOfFuel, fuel)
    } else {
        let rest = (fuel - 1) as nat;
        let (outcome, spent) = match resolve(host, cell) {
            None => (Outcome::Done(missing_cell(cell)), 0),
            Some(Definition::Foreign(_)) => (Outcome::Beyond, 0),
            Some(Definition::Primitive(_, primitive)) => primitive_call(
                primitive,
                call_fields,
                environment,
                host,
                stack,
                rest,
            ),
            Some(Definition::Grap(definition)) => match first_resolving(stack, cell) {
                Some(at) => (Outcome::Done(cycle(stack, at, cell)), 0),
                None => {
                    let (evaluated, spent) = eval(
                        definition,
                        environment,
                        host,
                        stack.push((cell, Reached::Call)),
                        rest,
                    );
                    match evaluated {
                        Outcome::Done(callable) => if is_absent(callable) {
                            (Outcome::Done(callable), spent)
                        } else {
                            let (applied, more) = apply(
                                callable,
                                call_fields,
                                environment,
                                host,
                                stack,
                                left(rest, spent),
                            );
                            (applied, spent + more)
                        },
                        stopped => (stopped, spent),
                    }
                },
            },
        };
        (outcome, spent + 1)
    }
}

/// Applying a callable to a call's arguments: a closure binds its parameters
/// and evaluates its body; a value naming a cell calls through the cell;
/// anything else isn't callable.
pub open spec fn apply(
    callable: Value,
    call_fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 3nat, 0nat,
{
    match closure(callable) {
        Some((params, body, captured)) => bind(
            params,
            body,
            captured,
            call_fields,
            environment,
            host,
            stack,
            fuel,
        ),
        None => match named_cell(callable) {
            Some(cell) => through_cell(cell, call_fields, environment, host, stack, fuel),
            None => (Outcome::Done(absent(NOT_CALLABLE, ABSENT_VALUE, callable)), 0),
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
    captured: Map<u128, Value>,
    call_fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 2nat, params.len(),
{
    if params.len() == 0 {
        eval(body, captured, host, stack, fuel)
    } else {
        let parameter = params[0];
        if call_fields.contains_key(parameter) {
            let (evaluated, spent) = eval(call_fields[parameter], environment, host, stack, fuel);
            match evaluated {
                Outcome::Done(argument) => {
                    let (outcome, more) = bind(
                        params.drop_first(),
                        body,
                        captured.insert(parameter, argument),
                        call_fields,
                        environment,
                        host,
                        stack,
                        left(fuel, spent),
                    );
                    (outcome, spent + more)
                },
                stopped => (stopped, spent),
            }
        } else {
            bind(
                params.drop_first(),
                body,
                captured.insert(parameter, missing_argument(parameter)),
                call_fields,
                environment,
                host,
                stack,
                fuel,
            )
        }
    }
}

/// A primitive's operand: the call's argument under `label`, evaluated where
/// the call is, or, missing, its absence, at no cost.
pub open spec fn operand(
    label: u128,
    call_fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 1nat, 0nat,
{
    if call_fields.contains_key(label) {
        eval(call_fields[label], environment, host, stack, fuel)
    } else {
        (Outcome::Done(missing_argument(label)), 0)
    }
}

/// Calling a primitive: its operands in order, then its operation on them.
/// Running out of fuel or leaving the core on an operand stops the call
/// there.
pub open spec fn primitive_call(
    primitive: Primitive,
    call_fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (Outcome, nat)
    decreases fuel, 2nat, 0nat,
{
    match primitive {
        Primitive::Binary { operation, left: first_label, right: second_label } => {
            let (first, spent) = operand(first_label, call_fields, environment, host, stack, fuel);
            match first {
                Outcome::Done(first) => {
                    let (second, more) = operand(
                        second_label,
                        call_fields,
                        environment,
                        host,
                        stack,
                        left(fuel, spent),
                    );
                    match second {
                        Outcome::Done(second) => (
                            Outcome::Done(binary(operation, first_label, first, second_label, second)),
                            spent + more,
                        ),
                        stopped => (stopped, spent + more),
                    }
                },
                stopped => (stopped, spent),
            }
        },
        Primitive::Unary { operation, operand: label } => {
            let (value, spent) = operand(label, call_fields, environment, host, stack, fuel);
            match value {
                Outcome::Done(value) => (Outcome::Done(unary(operation, label, value)), spent),
                stopped => (stopped, spent),
            }
        },
    }
}

/// A binary operation on its operands' values, under their labels: its
/// answer when both are numbers of its type, and otherwise "invalid
/// arguments", carrying why each that isn't couldn't be used.
pub open spec fn binary(
    operation: BinaryOperation,
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
            seq![(second_label, not_a_number(number, second))],
        ),
        (None, Some(_)) => caused_by_all(
            INVALID_ARGUMENTS,
            seq![(first_label, not_a_number(number, first))],
        ),
        (None, None) => caused_by_all(
            INVALID_ARGUMENTS,
            seq![
                (first_label, not_a_number(number, first)),
                (second_label, not_a_number(number, second)),
            ],
        ),
    }
}

/// A unary operation on its operand's value, under its label.
pub open spec fn unary(operation: UnaryOperation, label: u128, value: Value) -> Value {
    let number = unary_number(operation);
    match number_bytes(number, value) {
        Some(bytes) => answer_value(number, unary_answer(operation, bytes)),
        None => caused_by_all(INVALID_ARGUMENTS, seq![(label, not_a_number(number, value))]),
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
/// as it is; anything else is `{absent: not <type>, value}`.
pub open spec fn not_a_number(number: Number, value: Value) -> Value {
    if is_absent(value) {
        value
    } else {
        absent(not_reason(number), ABSENT_VALUE, value)
    }
}

/// An answer as a value: `{<type>: bytes}`, `{bool: true}` or `{bool:
/// false}`, or `{absent: reason}`.
pub open spec fn answer_value(number: Number, answer: Answer) -> Value {
    match answer {
        Answer::Number(bytes) => Value::Record(map![tag(number) => Value::Blob(bytes)]),
        Answer::Truth(truth) => Value::Record(
            map![BOOL => Value::Cell(if truth { TRUE } else { FALSE })],
        ),
        Answer::Failure(reason) => Value::Record(map![ABSENT => Value::Cell(reason)]),
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

/// The longest chain of causes an absent grows: past it, an absent passes on
/// unchanged instead of gaining another layer.
pub const CHAIN_LIMIT: usize = 8;

/// An absent for `reason`, carrying each detail under its field: its causes
/// are those that are absent. An absent the same as one kept before it isn't
/// repeated. When the longest chain of causes among them holds
/// `CHAIN_LIMIT` absents or more, that detail passes on unchanged instead:
/// the last of the longest.
pub open spec fn caused_by_all(reason: u128, details: Seq<(u128, Value)>) -> Value {
    let kept = distinct(details);
    if kept.len() > 0 && chain_at_least(kept[longest(kept)].1, CHAIN_LIMIT as nat) {
        kept[longest(kept)].1
    } else {
        Value::Record(to_map(seq![(ABSENT, Value::Cell(reason))] + kept))
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
/// other than its reason holds the rest.
pub open spec fn chain_at_least(value: Value, n: nat) -> bool
    decreases n,
{
    if n == 0 {
        true
    } else {
        match value {
            Value::Record(fields) => is_absent(value) && (n == 1 || exists|field: u128|
                field != ABSENT && #[trigger] fields.contains_key(field) && chain_at_least(
                    fields[field],
                    (n - 1) as nat,
                )),
            _ => false,
        }
    }
}

/// A program evaluated with `fuel`: its result, the fuel left, and whether it
/// completed, or `None` when it calls a foreign function. A program that
/// runs out comes to `{absent: fuel exhausted}`, with none left.
pub open spec fn evaluate(expression: Value, host: Seq<(u128, Definition)>, fuel: nat) -> Option<
    (Value, nat, bool),
> {
    let (outcome, spent) = eval(expression, Map::empty(), host, Seq::empty(), fuel);
    match outcome {
        Outcome::Done(result) => Some((result, left(fuel, spent), true)),
        Outcome::OutOfFuel => Some((fuel_exhausted(), 0, false)),
        Outcome::Beyond => None,
    }
}

} // verus!
