//! The definition as code that runs: an evaluator proven to compute exactly
//! what `definition` says, step for step, for testing Grap's real evaluator
//! against. It shares structure rather than copying, and is otherwise as
//! slow and simple as the definition.

#[cfg(verus_only)]
use crate::definition;
#[cfg(verus_only)]
use crate::definition::{Definition, Outcome};
use crate::definition::{Halt, Implementation, MAX_DEPTH, Primitive};
use crate::number::{binary, unary};
#[cfg(verus_only)]
use crate::origin::view_at;
use crate::origin::{At, child, defined_at, input, origin_value, push_called, read_origin};
use crate::value::*;
use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A cell's definition, as the evaluator's host holds it.
pub struct ExecDefinition {
    pub value: ExecValue,
    pub implementation: Option<Implementation>,
}

/// What evaluating an expression came to, as the evaluator holds it.
pub enum ExecOutcome {
    Done(ExecValue),
    OutOfFuel,
    OutOfDepth,
    Beyond,
}

/// A program's result, the fuel left, and why it halted, if it did.
pub struct ExecEvaluation {
    pub result: ExecValue,
    pub remaining_fuel: usize,
    pub halt: Option<Halt>,
}

pub open spec fn view_definition(definition: ExecDefinition) -> Definition {
    Definition { value: view(definition.value), implementation: definition.implementation }
}

pub open spec fn view_host(host: Seq<(u128, ExecDefinition)>) -> Seq<(u128, Definition)> {
    Seq::new(host.len(), |i: int| (host[i].0, view_definition(host[i].1)))
}

pub open spec fn view_outcome(outcome: ExecOutcome) -> Outcome {
    match outcome {
        ExecOutcome::Done(value) => Outcome::Done(view(value)),
        ExecOutcome::OutOfFuel => Outcome::OutOfFuel,
        ExecOutcome::OutOfDepth => Outcome::OutOfDepth,
        ExecOutcome::Beyond => Outcome::Beyond,
    }
}

fn resolve(host: &[(u128, ExecDefinition)], cell: u128) -> (found: Option<&ExecDefinition>)
    ensures
        match found {
            Some(definition) => definition::resolve(view_host(host@), cell) == Some(
                view_definition(*definition),
            ),
            None => definition::resolve(view_host(host@), cell) is None,
        },
    decreases host.len(),
{
    if host.len() == 0 {
        None
    } else if host[0].0 == cell {
        Some(&host[0].1)
    } else {
        let rest = &host[1..];
        proof {
            assert(view_host(rest@) =~= view_host(host@).drop_first());
        }
        resolve(rest, cell)
    }
}

pub(crate) fn missing_cell(cell: u128) -> (made: ExecValue)
    ensures
        view(made) == definition::missing_cell(cell),
{
    exec_with_detail(MISSING_CELL, ABSENT_CELL, ExecValue::Cell(cell))
}

pub(crate) fn missing_argument(parameter: u128, call: &At) -> (made: ExecValue)
    ensures
        view(made) == definition::missing_argument(parameter, view_at(*call)),
{
    let mut details: Vec<(u128, ExecValue)> = Vec::new();
    let pair = (ABSENT_CELL, ExecValue::Cell(parameter));
    proof {
        lemma_view_pairs_push(details@, pair);
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
    }
    details.push(pair);
    proof {
        assert(view_pairs(details@) =~= seq![(ABSENT_CELL, Value::Cell(parameter))]);
    }
    push_called(&mut details, call);
    exec_absent(MISSING_ARGUMENT, &details)
}

pub(crate) fn missing_field(field: u128) -> (made: ExecValue)
    ensures
        view(made) == definition::missing_field(field),
{
    exec_with_detail(MISSING_FIELD, ABSENT_CELL, ExecValue::Cell(field))
}

pub(crate) fn not_callable(callable: &ExecValue) -> (made: ExecValue)
    ensures
        view(made) == definition::not_callable(view(*callable)),
{
    if exec_is_absent(callable) {
        copy(callable)
    } else {
        exec_with_detail(NOT_CALLABLE, ABSENT_VALUE, copy(callable))
    }
}

/// The code under a field a form's kind requires, or why it's missing.
pub(crate) fn required(fields: &Vec<(u128, ExecValue)>, field: u128) -> (found: ExecValue)
    ensures
        view(found) == definition::required(view_map(fields@), field),
{
    match get(fields, field) {
        Some(value) => copy(value),
        None => missing_field(field),
    }
}

pub(crate) fn parameters(params: &[ExecValue]) -> (parsed: Result<Vec<u128>, ExecValue>)
    ensures
        match (parsed, definition::parameters(view_elements(params@))) {
            (Ok(cells), Ok(spec_cells)) => cells@ == spec_cells,
            (Err(first), Err(spec_first)) => view(first) == spec_first,
            _ => false,
        },
    decreases params.len(),
{
    if params.len() == 0 {
        proof {
            assert(view_elements(params@) =~= Seq::<Value>::empty());
        }
        Ok(Vec::new())
    } else {
        let rest = &params[1..];
        proof {
            assert(rest@ =~= params@.drop_first());
            assert(view_elements(rest@) =~= view_elements(params@).drop_first());
        }
        match &params[0] {
            ExecValue::Cell(cell) => match parameters(rest) {
                Ok(cells) => {
                    let mut all = Vec::new();
                    all.push(*cell);
                    let mut i = 0;
                    while i < cells.len()
                        invariant
                            i <= cells.len(),
                            all@ == seq![*cell] + cells@.subrange(0, i as int),
                        decreases cells.len() - i,
                    {
                        all.push(cells[i]);
                        i += 1;
                        assert(all@ =~= seq![*cell] + cells@.subrange(0, i as int));
                    }
                    assert(cells@.subrange(0, cells.len() as int) =~= cells@);
                    Ok(all)
                },
                Err(first) => Err(first),
            },
            first => Err(copy(first)),
        }
    }
}

/// A closure record, as `definition::closure_record` says.
pub(crate) fn closure_record(
    lambda: Rc<Vec<(u128, ExecValue)>>,
    environment: Rc<Vec<(u128, ExecValue)>>,
    body_origin: &At,
) -> (made: ExecValue)
    ensures
        view(made) == definition::closure_record(
            view_map(lambda@),
            view_map(environment@),
            view_at(*body_origin),
        ),
{
    let mut fields: Vec<(u128, ExecValue)> = Vec::new();
    proof {
        lemma_view_record(lambda);
        lemma_view_record(environment);
        assert(view_pairs(fields@) =~= Seq::<(u128, Value)>::empty());
    }
    let kind = (ISA, ExecValue::Cell(KIND_CLOSURE));
    proof {
        lemma_view_pairs_push(fields@, kind);
    }
    fields.push(kind);
    let written = (LAMBDA, ExecValue::Record(lambda));
    proof {
        lemma_view_pairs_push(fields@, written);
    }
    fields.push(written);
    let captured = (ENVIRONMENT, ExecValue::Record(environment));
    proof {
        lemma_view_pairs_push(fields@, captured);
    }
    fields.push(captured);
    let ghost base = view_map(fields@);
    proof {
        assert(base =~= map![ISA => Value::Cell(KIND_CLOSURE), LAMBDA => Value::Record(view_map(lambda@)), ENVIRONMENT => Value::Record(view_map(environment@))]);
    }
    match body_origin {
        Some(origin) => {
            let made = origin_value(origin);
            proof {
                lemma_view_pairs_push(fields@, (BODY_ORIGIN, made));
            }
            fields.push((BODY_ORIGIN, made));
        },
        None => {},
    }
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
    }
    ExecValue::Record(fields)
}

fn lambda(
    fields: &Rc<Vec<(u128, ExecValue)>>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
) -> (made: ExecValue)
    ensures
        view(made) == definition::lambda(view_map(fields@), view_at(*origin), view_map(environment@)),
{
    proof {
        lemma_view_record(*fields);
    }
    match get(fields, PARAMS) {
        Some(ExecValue::List(elements)) => match parameters(elements.as_slice()) {
            Ok(_) => {
                let mut written = copy_pairs(fields);
                let body = required(fields, BODY);
                proof {
                    lemma_view_pairs_push(written@, (BODY, body));
                }
                written.push((BODY, body));
                closure_record(
                    Rc::new(written),
                    Rc::new(copy_pairs(environment)),
                    &child(origin, BODY),
                )
            },
            Err(parameter) => exec_with_detail(INVALID_PARAMETER, ABSENT_VALUE, parameter),
        },
        _ => exec_with_detail(MALFORMED_LAMBDA, ABSENT_VALUE, ExecValue::Record(Rc::clone(fields))),
    }
}

pub(crate) fn body_origin(fields: &Vec<(u128, ExecValue)>) -> (at: At)
    ensures
        view_at(at) == definition::body_origin(view_map(fields@)),
{
    match get(fields, BODY_ORIGIN) {
        Some(value) => read_origin(value),
        None => None,
    }
}

/// A closure record's parts, as `definition::closure` reads them.
pub(crate) fn closure(callable: &ExecValue) -> (parts: Option<
    (Vec<u128>, &ExecValue, At, &Vec<(u128, ExecValue)>),
>)
    ensures
        match (parts, definition::closure(view(*callable))) {
            (
                Some((params, body, at, captured)),
                Some((spec_params, spec_body, spec_at, spec_captured)),
            ) => params@ == spec_params && view(*body) == spec_body && view_at(at) == spec_at
                && view_map(captured@) == spec_captured,
            (None, None) => true,
            _ => false,
        },
{
    match callable {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            if !exec_is_kind(fields, KIND_CLOSURE) {
                return None;
            }
            match get(fields, LAMBDA) {
                Some(ExecValue::Record(written)) => {
                    proof {
                        lemma_view_record(*written);
                    }
                    match get(fields, ENVIRONMENT) {
                        Some(ExecValue::Record(captured)) => {
                            proof {
                                lemma_view_record(*captured);
                            }
                            match get(written, PARAMS) {
                                Some(ExecValue::List(params)) => match get(written, BODY) {
                                    Some(body) => match parameters(params.as_slice()) {
                                        Ok(cells) => Some(
                                            (cells, body, body_origin(fields), &**captured),
                                        ),
                                        Err(_) => None,
                                    },
                                    None => None,
                                },
                                _ => None,
                            }
                        },
                        _ => None,
                    }
                },
                _ => None,
            }
        },
        _ => None,
    }
}

pub(crate) fn implementation_cell(value: &ExecValue) -> (cell: Option<u128>)
    ensures
        cell == definition::implementation_cell(view(*value)),
{
    match value {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            if exec_is_kind(fields, KIND_FOREIGN_FUNCTION) {
                match get(fields, IMPLEMENTATION) {
                    Some(ExecValue::Cell(cell)) => Some(*cell),
                    _ => None,
                }
            } else {
                None
            }
        },
        _ => None,
    }
}

pub(crate) fn callee_cell(callable: &ExecValue) -> (cell: Option<u128>)
    ensures
        cell == definition::callee_cell(view(*callable)),
{
    match callable {
        ExecValue::Cell(cell) => Some(*cell),
        _ => implementation_cell(callable),
    }
}

/// Whether calling `cell`, defined as `value`, runs what the host
/// implements under it.
pub(crate) fn implemented_here(value: &ExecValue, cell: u128) -> (here: bool)
    ensures
        here == (definition::implementation_cell(view(*value)) == Some(cell)),
{
    match implementation_cell(value) {
        Some(implemented) => implemented == cell,
        None => false,
    }
}

fn eval(
    expression: &ExecValue,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::eval(
            view(*expression),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 0nat, 0nat,
{
    if fuel < 2 {
        return (ExecOutcome::OutOfFuel, fuel);
    }
    if depth == 0 {
        return (ExecOutcome::OutOfDepth, 1);
    }
    let rest = fuel - 1;
    let within = depth - 1;
    let (outcome, spent) = match expression {
        ExecValue::Cell(cell) => reference(*cell, environment, host, rest, within),
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            if exec_is_kind(fields, KIND_CALL) {
                call(fields, origin, environment, host, rest, within)
            } else if exec_is_kind(fields, KIND_LAMBDA) {
                (ExecOutcome::Done(lambda(fields, origin, environment)), 0)
            } else if exec_is_kind(fields, KIND_VALUE_WRAPPER) {
                let inner = required(fields, VALUE);
                eval(&inner, &child(origin, VALUE), environment, host, rest, within)
            } else {
                (ExecOutcome::Done(copy(expression)), 0)
            }
        },
        _ => (ExecOutcome::Done(copy(expression)), 0),
    };
    (outcome, spent + 1)
}

fn reference(
    cell: u128,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::reference(
            cell,
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 1nat, 0nat,
{
    match get(environment, cell) {
        Some(bound) => (ExecOutcome::Done(copy(bound)), 0),
        None => match resolve(host, cell) {
            None => (ExecOutcome::Done(missing_cell(cell)), 0),
            Some(found) => eval(&found.value, &defined_at(cell), environment, host, fuel, depth),
        },
    }
}

fn call(
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::call(
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 4nat, 0nat,
{
    let function = required(fields, FUNCTION);
    match &function {
        ExecValue::Cell(cell) => match get(environment, *cell) {
            Some(bound) => if fuel < 2 {
                (ExecOutcome::OutOfFuel, fuel)
            } else {
                let (outcome, spent) = apply(
                    bound,
                    fields,
                    origin,
                    environment,
                    host,
                    fuel - 1,
                    depth,
                );
                (outcome, spent + 1)
            },
            None => call_cell(*cell, fields, origin, environment, host, fuel, depth),
        },
        _ => {
            let (outcome, spent) = eval(
                &function,
                &child(origin, FUNCTION),
                environment,
                host,
                fuel,
                depth,
            );
            match outcome {
                ExecOutcome::Done(callable) => {
                    let (applied, more) = apply(
                        &callable,
                        fields,
                        origin,
                        environment,
                        host,
                        fuel - spent,
                        depth,
                    );
                    (applied, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
    }
}

fn call_cell(
    cell: u128,
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::call_cell(
            cell,
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 2nat, 0nat,
{
    if fuel < 2 {
        return (ExecOutcome::OutOfFuel, fuel);
    }
    let rest = fuel - 1;
    let (outcome, spent) = match resolve(host, cell) {
        None => (ExecOutcome::Done(missing_cell(cell)), 0),
        Some(found) => if implemented_here(&found.value, cell) {
            match &found.implementation {
                Some(Implementation::Primitive(primitive)) => primitive_call(
                    primitive,
                    fields,
                    origin,
                    environment,
                    host,
                    rest,
                    depth,
                ),
                Some(Implementation::Foreign) => (ExecOutcome::Beyond, 0),
                None => (ExecOutcome::Done(not_callable(&found.value)), 0),
            }
        } else {
            call_code(&found.value, cell, fields, origin, environment, host, rest, depth)
        },
    };
    (outcome, spent + 1)
}

fn call_code(
    value: &ExecValue,
    cell: u128,
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::call_code(
            view(*value),
            cell,
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 5nat, 0nat,
{
    let (evaluated, spent) = eval(value, &defined_at(cell), environment, host, fuel, depth);
    match evaluated {
        ExecOutcome::Done(callable) => if exec_is_absent(&callable) {
            (ExecOutcome::Done(callable), spent)
        } else {
            let (applied, more) = apply(
                &callable,
                fields,
                origin,
                environment,
                host,
                fuel - spent,
                depth,
            );
            (applied, spent + more)
        },
        stopped => (stopped, spent),
    }
}

fn apply(
    callable: &ExecValue,
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::apply(
            view(*callable),
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 3nat, 0nat,
{
    match closure(callable) {
        Some((params, body, at, captured)) => bind(
            params.as_slice(),
            body,
            &at,
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
            None => (ExecOutcome::Done(not_callable(callable)), 0),
        },
    }
}

fn bind(
    params: &[u128],
    body: &ExecValue,
    body_origin: &At,
    captured: &Vec<(u128, ExecValue)>,
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::bind(
            params@,
            view(*body),
            view_at(*body_origin),
            view_map(captured@),
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 2nat, params.len(),
{
    if params.len() == 0 {
        eval(body, body_origin, captured, host, fuel, depth)
    } else {
        let parameter = params[0];
        let rest = &params[1..];
        proof {
            assert(rest@ =~= params@.drop_first());
        }
        match get(fields, parameter) {
            Some(argument) => {
                let (evaluated, spent) = eval(
                    argument,
                    &child(origin, parameter),
                    environment,
                    host,
                    fuel,
                    depth,
                );
                match evaluated {
                    ExecOutcome::Done(value) => {
                        let mut bound = copy_pairs(captured);
                        proof {
                            lemma_view_pairs_push(bound@, (parameter, value));
                        }
                        bound.push((parameter, value));
                        let (outcome, more) = bind(
                            rest,
                            body,
                            body_origin,
                            &bound,
                            fields,
                            origin,
                            environment,
                            host,
                            fuel - spent,
                            depth,
                        );
                        (outcome, spent + more)
                    },
                    stopped => (stopped, spent),
                }
            },
            None => {
                let mut bound = copy_pairs(captured);
                let missing = missing_argument(parameter, origin);
                proof {
                    lemma_view_pairs_push(bound@, (parameter, missing));
                }
                bound.push((parameter, missing));
                bind(
                    rest,
                    body,
                    body_origin,
                    &bound,
                    fields,
                    origin,
                    environment,
                    host,
                    fuel,
                    depth,
                )
            },
        }
    }
}

fn operand(
    label: u128,
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::operand(
            label,
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 1nat, 0nat,
{
    match get(fields, label) {
        Some(argument) => eval(argument, &child(origin, label), environment, host, fuel, depth),
        None => (ExecOutcome::Done(missing_argument(label, origin)), 0),
    }
}

fn primitive_call(
    primitive: &Primitive,
    fields: &Vec<(u128, ExecValue)>,
    origin: &At,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    fuel: usize,
    depth: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::primitive_call(
            *primitive,
            view_map(fields@),
            view_at(*origin),
            view_map(environment@),
            view_host(host@),
            fuel as nat,
            depth as nat,
        ),
    decreases fuel, 2nat, 0nat,
{
    match primitive {
        Primitive::Binary { operation, left, right } => {
            let (first, spent) = operand(*left, fields, origin, environment, host, fuel, depth);
            match first {
                ExecOutcome::Done(first) => {
                    let (second, more) = operand(
                        *right,
                        fields,
                        origin,
                        environment,
                        host,
                        fuel - spent,
                        depth,
                    );
                    match second {
                        ExecOutcome::Done(second) => (
                            ExecOutcome::Done(
                                binary(operation, origin, *left, &first, *right, &second),
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
            let (value, spent) = operand(*label, fields, origin, environment, host, fuel, depth);
            match value {
                ExecOutcome::Done(value) => (
                    ExecOutcome::Done(unary(operation, origin, *label, &value)),
                    spent,
                ),
                stopped => (stopped, spent),
            }
        },
    }
}

/// A program, handed to an evaluation as its input, evaluated with `fuel`
/// against `host`'s definitions, as `definition::evaluate` says: `None` when
/// it calls a foreign function.
pub fn evaluate(expression: &ExecValue, host: &Vec<(u128, ExecDefinition)>, fuel: usize) -> (evaluation:
    Option<ExecEvaluation>)
    ensures
        match (evaluation, definition::evaluate(view(*expression), view_host(host@), fuel as nat)) {
            (Some(evaluation), Some((result, remaining, halt))) => view(evaluation.result) == result
                && evaluation.remaining_fuel as nat == remaining && evaluation.halt == halt,
            (None, None) => true,
            _ => false,
        },
{
    let environment: Vec<(u128, ExecValue)> = Vec::new();
    proof {
        assert(view_pairs(environment@) =~= Seq::<(u128, Value)>::empty());
    }
    let (outcome, spent) = eval(
        expression,
        &input(),
        &environment,
        host.as_slice(),
        fuel,
        MAX_DEPTH,
    );
    match outcome {
        ExecOutcome::Done(result) => Some(
            ExecEvaluation { result, remaining_fuel: fuel - spent, halt: None },
        ),
        ExecOutcome::OutOfFuel => Some(
            ExecEvaluation {
                result: exec_bare_absent(FUEL_EXHAUSTED),
                remaining_fuel: 0,
                halt: Some(Halt::FuelExhausted),
            },
        ),
        ExecOutcome::OutOfDepth => Some(
            ExecEvaluation {
                result: exec_bare_absent(DEPTH_EXHAUSTED),
                remaining_fuel: fuel - spent,
                halt: Some(Halt::DepthExhausted),
            },
        ),
        ExecOutcome::Beyond => None,
    }
}

} // verus!
