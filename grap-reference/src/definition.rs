//! What Grap's core means: a slow, simple evaluator, written as Verus
//! specification functions, over the GID values of `value`.
//!
//! The core is what Grap's evaluator does itself, not what libraries add:
//! cells, lambdas, calls, value wrappers, closures, the absents those come to,
//! and fuel. Calling a foreign function leaves the core, and evaluation that
//! gets there says so (`Outcome::Beyond`) rather than guessing.
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

/// A cell's definition, as the host resolves it: Grap's own, or a foreign
/// function's. Only a foreign definition's descriptive value is part of the
/// core: referencing the cell evaluates it, while calling the cell leaves the
/// core.
#[cfg(verus_only)]
pub enum Definition {
    Grap(Value),
    Foreign(Value),
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
/// function's leaves the core. Grap's is evaluated where the call is, unless
/// anything already has the cell under resolution, and is applied unless it
/// comes to an absent, which is the call's result as it is.
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
