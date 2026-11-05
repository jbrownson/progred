//! The definition as code that runs: an evaluator proven to compute exactly
//! what `definition` says, step for step, for testing Grap's real evaluator
//! against. It shares structure rather than copying, and is otherwise as
//! slow and simple as the definition.

#[cfg(verus_only)]
use crate::definition;
#[cfg(verus_only)]
use crate::definition::{Definition, Outcome};
use crate::definition::{Primitive, Reached};
use crate::number::{binary, unary};
use crate::value::*;
use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A cell's definition, as the evaluator's host holds it.
pub enum ExecDefinition {
    Grap(ExecValue),
    Foreign(ExecValue),
    Primitive(ExecValue, Primitive),
}

/// What evaluating an expression came to, as the evaluator holds it.
pub enum ExecOutcome {
    Done(ExecValue),
    OutOfFuel,
    Beyond,
}

/// A program's result, the fuel left, and whether it completed.
pub struct ExecEvaluation {
    pub result: ExecValue,
    pub remaining_fuel: usize,
    pub completed: bool,
}

pub open spec fn view_definition(definition: ExecDefinition) -> Definition {
    match definition {
        ExecDefinition::Grap(value) => Definition::Grap(view(value)),
        ExecDefinition::Foreign(value) => Definition::Foreign(view(value)),
        ExecDefinition::Primitive(value, primitive) => Definition::Primitive(view(value), primitive),
    }
}

pub open spec fn view_host(host: Seq<(u128, ExecDefinition)>) -> Seq<(u128, Definition)> {
    Seq::new(host.len(), |i: int| (host[i].0, view_definition(host[i].1)))
}

pub open spec fn view_outcome(outcome: ExecOutcome) -> Outcome {
    match outcome {
        ExecOutcome::Done(value) => Outcome::Done(view(value)),
        ExecOutcome::OutOfFuel => Outcome::OutOfFuel,
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

fn definition_value(definition: &ExecDefinition) -> (value: &ExecValue)
    ensures
        view(*value) == definition::definition_value(view_definition(*definition)),
{
    match definition {
        ExecDefinition::Grap(value) => value,
        ExecDefinition::Foreign(description) => description,
        ExecDefinition::Primitive(description, _) => description,
    }
}

fn first_resolving(stack: &[(u128, Reached)], cell: u128) -> (at: Option<usize>)
    ensures
        match at {
            Some(at) => definition::first_resolving(stack@, cell) == Some(at as int) && at
                < stack.len(),
            None => definition::first_resolving(stack@, cell) is None,
        },
    decreases stack.len(),
{
    if stack.len() == 0 {
        None
    } else if stack[0].0 == cell {
        Some(0)
    } else {
        let rest = &stack[1..];
        proof {
            assert(rest@ =~= stack@.drop_first());
        }
        match first_resolving(rest, cell) {
            Some(at) => Some(at + 1),
            None => None,
        }
    }
}

fn referenced_at(stack: &[(u128, Reached)], cell: u128) -> (at: Option<usize>)
    ensures
        match at {
            Some(at) => definition::referenced_at(stack@, cell) == Some(at as int) && at
                < stack.len(),
            None => definition::referenced_at(stack@, cell) is None,
        },
    decreases stack.len(),
{
    if stack.len() == 0 {
        None
    } else if stack[0].0 == cell && matches!(stack[0].1, Reached::Reference) {
        Some(0)
    } else {
        let rest = &stack[1..];
        proof {
            assert(rest@ =~= stack@.drop_first());
        }
        match referenced_at(rest, cell) {
            Some(at) => Some(at + 1),
            None => None,
        }
    }
}

/// The stack with `cell` under resolution too.
fn resolving(stack: &Vec<(u128, Reached)>, cell: u128, reached: Reached) -> (pushed: Vec<
    (u128, Reached),
>)
    ensures
        pushed@ == stack@.push((cell, reached)),
{
    let mut pushed = Vec::new();
    let mut i = 0;
    while i < stack.len()
        invariant
            i <= stack.len(),
            pushed@ == stack@.subrange(0, i as int),
        decreases stack.len() - i,
    {
        let entry = match stack[i].1 {
            Reached::Reference => (stack[i].0, Reached::Reference),
            Reached::Call => (stack[i].0, Reached::Call),
        };
        pushed.push(entry);
        i += 1;
        assert(pushed@ =~= stack@.subrange(0, i as int));
    }
    pushed.push((cell, reached));
    assert(pushed@ =~= stack@.push((cell, reached)));
    pushed
}

fn cycle(stack: &Vec<(u128, Reached)>, at: usize, cell: u128) -> (made: ExecValue)
    requires
        at < stack.len(),
    ensures
        view(made) == definition::cycle(stack@, at as int, cell),
{
    let mut cells: Vec<ExecValue> = Vec::new();
    let mut i = at;
    while i < stack.len()
        invariant
            at <= i <= stack.len(),
            cells@.len() == i - at,
            forall|j: int| 0 <= j < cells@.len() ==> #[trigger] view(cells@[j]) == Value::Cell(
                stack@[at + j].0,
            ),
        decreases stack.len() - i,
    {
        cells.push(ExecValue::Cell(stack[i].0));
        i += 1;
    }
    let ghost before = cells@;
    cells.push(ExecValue::Cell(cell));
    proof {
        let expected = definition::resolving_cells(stack@, at as int).push(Value::Cell(cell));
        assert(view(ExecValue::Cell(cell)) == Value::Cell(cell));
        assert(before.len() == stack.len() - at);
        assert forall|j: int| 0 <= j < cells@.len() implies #[trigger] view_elements(cells@)[j]
            == expected[j] by {
            if j < before.len() {
                assert(cells@[j] == before[j]);
                assert(view(before[j]) == Value::Cell(stack@[at + j].0));
                assert(expected[j] == definition::resolving_cells(stack@, at as int)[j]);
            } else {
                assert(cells@[j] == ExecValue::Cell(cell));
            }
        }
        assert(view_elements(cells@) =~= expected);
    }
    let list = ExecValue::List(Rc::new(cells));
    exec_absent(CELL_CYCLE, CYCLE, list)
}

fn missing_cell(cell: u128) -> (made: ExecValue)
    ensures
        view(made) == definition::missing_cell(cell),
{
    exec_absent(MISSING_CELL, ABSENT_CELL, ExecValue::Cell(cell))
}

fn missing_argument(parameter: u128) -> (made: ExecValue)
    ensures
        view(made) == definition::missing_argument(parameter),
{
    exec_absent(MISSING_ARGUMENT, ABSENT_CELL, ExecValue::Cell(parameter))
}

fn parameters(params: &[ExecValue]) -> (parsed: Result<Vec<u128>, ExecValue>)
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

fn lambda(fields: &Rc<Vec<(u128, ExecValue)>>, environment: &Vec<(u128, ExecValue)>) -> (made:
    ExecValue)
    requires
        view_map(fields@).contains_key(PARAMS),
    ensures
        view(made) == definition::lambda(view_map(fields@), view_map(environment@)),
{
    let params = get(fields, PARAMS).unwrap();
    match params {
        ExecValue::List(elements) => match parameters(elements.as_slice()) {
            Ok(_) => {
                let mut inner = without(fields, BODY_ORIGIN);
                let captured = Rc::new(copy_pairs(environment));
                let ghost before = inner@;
                inner.push((ENVIRONMENT, ExecValue::Record(captured)));
                proof {
                    lemma_view_pairs_push(before, (ENVIRONMENT, ExecValue::Record(captured)));
                    lemma_view_record(captured);
                }
                let inner = Rc::new(inner);
                let mut outer = Vec::new();
                outer.push((CLOSURE, ExecValue::Record(inner)));
                let outer = Rc::new(outer);
                proof {
                    lemma_view_record(inner);
                    lemma_view_record(outer);
                    lemma_view_pairs_push(Seq::empty(), (CLOSURE, ExecValue::Record(inner)));
                    assert(outer@ =~= Seq::<(u128, ExecValue)>::empty().push(
                        (CLOSURE, ExecValue::Record(inner)),
                    ));
                    assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<
                        (u128, Value),
                    >::empty());
                    assert(view_map(outer@) =~= map![CLOSURE => view(ExecValue::Record(inner))]);
                    assert(view_map(inner@) =~= view_map(fields@).remove(BODY_ORIGIN).insert(
                        ENVIRONMENT,
                        Value::Record(view_map(environment@)),
                    ));
                }
                ExecValue::Record(outer)
            },
            Err(parameter) => exec_absent(INVALID_PARAMETER, ABSENT_VALUE, parameter),
        },
        _ => {
            proof {
                lemma_view_record(*fields);
            }
            exec_absent(MALFORMED_LAMBDA, ABSENT_VALUE, ExecValue::Record(Rc::clone(fields)))
        },
    }
}

fn closure(callable: &ExecValue) -> (parts: Option<(Vec<u128>, &ExecValue, &Vec<(u128, ExecValue)>)>)
    ensures
        match (parts, definition::closure(view(*callable))) {
            (Some((params, body, captured)), Some((spec_params, spec_body, spec_captured))) =>
                params@ == spec_params && view(*body) == spec_body && view_map(captured@)
                    == spec_captured,
            (None, None) => true,
            _ => false,
        },
{
    match callable {
        ExecValue::Record(outer) => {
            proof {
                lemma_view_record(*outer);
            }
            match get(outer, CLOSURE) {
                Some(ExecValue::Record(fields)) => {
                    proof {
                        lemma_view_record(*fields);
                    }
                    match (get(fields, PARAMS), get(fields, BODY), get(fields, ENVIRONMENT)) {
                        (Some(ExecValue::List(params)), Some(body), Some(ExecValue::Record(captured))) => {
                            proof {
                                lemma_view_record(*captured);
                            }
                            match parameters(params.as_slice()) {
                                Ok(cells) => Some((cells, body, &**captured)),
                                Err(_) => None,
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

fn named_cell(callable: &ExecValue) -> (cell: Option<u128>)
    ensures
        cell == definition::named_cell(view(*callable)),
{
    match callable {
        ExecValue::Cell(cell) => Some(*cell),
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            match get(fields, FFI) {
                Some(ExecValue::Cell(cell)) => Some(*cell),
                _ => None,
            }
        },
        _ => None,
    }
}

fn eval(
    expression: &ExecValue,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::eval(
            view(*expression),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 0nat, 0nat,
{
    if fuel < 2 {
        (ExecOutcome::OutOfFuel, fuel)
    } else {
        let rest = fuel - 1;
        let (outcome, spent) = match expression {
            ExecValue::Cell(cell) => reference(*cell, environment, host, stack, rest),
            ExecValue::Record(fields) => {
                proof {
                    lemma_view_record(*fields);
                }
                if has(fields, FUNCTION) {
                    call(fields, environment, host, stack, rest)
                } else if has(fields, PARAMS) && has(fields, BODY) {
                    (ExecOutcome::Done(lambda(fields, environment)), 0)
                } else {
                    match get(fields, VALUE) {
                        Some(inner) => eval(inner, environment, host, stack, rest),
                        None => (ExecOutcome::Done(copy(expression)), 0),
                    }
                }
            },
            _ => (ExecOutcome::Done(copy(expression)), 0),
        };
        (outcome, spent + 1)
    }
}

fn reference(
    cell: u128,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::reference(
            cell,
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 1nat, 0nat,
{
    match get(environment, cell) {
        Some(bound) => (ExecOutcome::Done(copy(bound)), 0),
        None => match resolve(host, cell) {
            None => (ExecOutcome::Done(missing_cell(cell)), 0),
            Some(found) => match referenced_at(stack.as_slice(), cell) {
                Some(at) => (ExecOutcome::Done(cycle(stack, at, cell)), 0),
                None => {
                    let pushed = resolving(stack, cell, Reached::Reference);
                    eval(definition_value(found), environment, host, &pushed, fuel)
                },
            },
        },
    }
}

fn call(
    fields: &Vec<(u128, ExecValue)>,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    requires
        view_map(fields@).contains_key(FUNCTION),
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::call(
            view_map(fields@),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 4nat, 0nat,
{
    let function = get(fields, FUNCTION).unwrap();
    match function {
        ExecValue::Cell(cell) => match get(environment, *cell) {
            Some(bound) => if fuel < 2 {
                (ExecOutcome::OutOfFuel, fuel)
            } else {
                let (outcome, spent) = apply(bound, fields, environment, host, stack, fuel - 1);
                (outcome, spent + 1)
            },
            None => through_cell(*cell, fields, environment, host, stack, fuel),
        },
        _ => {
            let (outcome, spent) = eval(function, environment, host, stack, fuel);
            match outcome {
                ExecOutcome::Done(callable) => {
                    let (applied, more) = apply(
                        &callable,
                        fields,
                        environment,
                        host,
                        stack,
                        fuel - spent,
                    );
                    (applied, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
    }
}

fn through_cell(
    cell: u128,
    call_fields: &Vec<(u128, ExecValue)>,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::through_cell(
            cell,
            view_map(call_fields@),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 2nat, 0nat,
{
    if fuel < 2 {
        (ExecOutcome::OutOfFuel, fuel)
    } else {
        let rest = fuel - 1;
        let (outcome, spent) = match resolve(host, cell) {
            None => (ExecOutcome::Done(missing_cell(cell)), 0),
            Some(ExecDefinition::Foreign(_)) => (ExecOutcome::Beyond, 0),
            Some(ExecDefinition::Primitive(_, primitive)) => primitive_call(
                primitive,
                call_fields,
                environment,
                host,
                stack,
                rest,
            ),
            Some(ExecDefinition::Grap(found)) => match first_resolving(stack.as_slice(), cell) {
                Some(at) => (ExecOutcome::Done(cycle(stack, at, cell)), 0),
                None => {
                    let pushed = resolving(stack, cell, Reached::Call);
                    let (evaluated, spent) = eval(found, environment, host, &pushed, rest);
                    match evaluated {
                        ExecOutcome::Done(callable) => if exec_is_absent(&callable) {
                            (ExecOutcome::Done(callable), spent)
                        } else {
                            let (applied, more) = apply(
                                &callable,
                                call_fields,
                                environment,
                                host,
                                stack,
                                rest - spent,
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

fn apply(
    callable: &ExecValue,
    call_fields: &Vec<(u128, ExecValue)>,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::apply(
            view(*callable),
            view_map(call_fields@),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 3nat, 0nat,
{
    match closure(callable) {
        Some((params, body, captured)) => bind(
            params.as_slice(),
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
            None => (ExecOutcome::Done(exec_absent(NOT_CALLABLE, ABSENT_VALUE, copy(callable))), 0),
        },
    }
}

fn bind(
    params: &[u128],
    body: &ExecValue,
    captured: &Vec<(u128, ExecValue)>,
    call_fields: &Vec<(u128, ExecValue)>,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::bind(
            params@,
            view(*body),
            view_map(captured@),
            view_map(call_fields@),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 2nat, params.len(),
{
    if params.len() == 0 {
        eval(body, captured, host, stack, fuel)
    } else {
        let parameter = params[0];
        let rest = &params[1..];
        proof {
            assert(rest@ =~= params@.drop_first());
        }
        match get(call_fields, parameter) {
            Some(argument) => {
                let (evaluated, spent) = eval(argument, environment, host, stack, fuel);
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
                            &bound,
                            call_fields,
                            environment,
                            host,
                            stack,
                            fuel - spent,
                        );
                        (outcome, spent + more)
                    },
                    stopped => (stopped, spent),
                }
            },
            None => {
                let mut bound = copy_pairs(captured);
                let missing = missing_argument(parameter);
                proof {
                    lemma_view_pairs_push(bound@, (parameter, missing));
                }
                bound.push((parameter, missing));
                bind(rest, body, &bound, call_fields, environment, host, stack, fuel)
            },
        }
    }
}

fn operand(
    label: u128,
    call_fields: &Vec<(u128, ExecValue)>,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::operand(
            label,
            view_map(call_fields@),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 1nat, 0nat,
{
    match get(call_fields, label) {
        Some(argument) => eval(argument, environment, host, stack, fuel),
        None => (ExecOutcome::Done(missing_argument(label)), 0),
    }
}

fn primitive_call(
    primitive: &Primitive,
    call_fields: &Vec<(u128, ExecValue)>,
    environment: &Vec<(u128, ExecValue)>,
    host: &[(u128, ExecDefinition)],
    stack: &Vec<(u128, Reached)>,
    fuel: usize,
) -> (evaluated: (ExecOutcome, usize))
    ensures
        evaluated.1 <= fuel,
        (view_outcome(evaluated.0), evaluated.1 as nat) == definition::primitive_call(
            *primitive,
            view_map(call_fields@),
            view_map(environment@),
            view_host(host@),
            stack@,
            fuel as nat,
        ),
    decreases fuel, 2nat, 0nat,
{
    match primitive {
        Primitive::Binary { operation, left, right } => {
            let (first, spent) = operand(*left, call_fields, environment, host, stack, fuel);
            match first {
                ExecOutcome::Done(first) => {
                    let (second, more) = operand(
                        *right,
                        call_fields,
                        environment,
                        host,
                        stack,
                        fuel - spent,
                    );
                    match second {
                        ExecOutcome::Done(second) => (
                            ExecOutcome::Done(binary(operation, *left, &first, *right, &second)),
                            spent + more,
                        ),
                        stopped => (stopped, spent + more),
                    }
                },
                stopped => (stopped, spent),
            }
        },
        Primitive::Unary { operation, operand: label } => {
            let (value, spent) = operand(*label, call_fields, environment, host, stack, fuel);
            match value {
                ExecOutcome::Done(value) => (
                    ExecOutcome::Done(unary(operation, *label, &value)),
                    spent,
                ),
                stopped => (stopped, spent),
            }
        },
    }
}

/// A program evaluated with `fuel` against `host`'s definitions, as
/// `definition::evaluate` says: `None` when it calls a foreign function.
pub fn evaluate(expression: &ExecValue, host: &Vec<(u128, ExecDefinition)>, fuel: usize) -> (evaluation:
    Option<ExecEvaluation>)
    ensures
        match (evaluation, definition::evaluate(view(*expression), view_host(host@), fuel as nat)) {
            (Some(evaluation), Some((result, remaining, completed))) => view(evaluation.result)
                == result && evaluation.remaining_fuel as nat == remaining && evaluation.completed
                == completed,
            (None, None) => true,
            _ => false,
        },
{
    let environment: Vec<(u128, ExecValue)> = Vec::new();
    let stack: Vec<(u128, Reached)> = Vec::new();
    proof {
        assert(view_pairs(environment@) =~= Seq::<(u128, Value)>::empty());
        assert(stack@ =~= Seq::<(u128, Reached)>::empty());
    }
    let (outcome, spent) = eval(expression, &environment, host.as_slice(), &stack, fuel);
    match outcome {
        ExecOutcome::Done(result) => Some(
            ExecEvaluation { result, remaining_fuel: fuel - spent, completed: true },
        ),
        ExecOutcome::OutOfFuel => Some(
            ExecEvaluation { result: exec_fuel_exhausted(), remaining_fuel: 0, completed: false },
        ),
        ExecOutcome::Beyond => None,
    }
}

} // verus!
