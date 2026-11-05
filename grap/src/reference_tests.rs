//! Grap's evaluator against `grap-reference`, the definition of Grap's core:
//! each program here must come to the same result, fuel left and completion
//! under both, unless it calls a foreign function, which the definition
//! leaves out. Results are compared without the source origins closures
//! record for click-to-source, which the core doesn't define yet.

use super::*;
use gid::new_cell_id;
use grap_reference::{self as reference, ExecDefinition, ExecValue};

fn reference_cell(cell: CellId) -> u128 {
    u128::from_be_bytes(*cell.as_bytes())
}

fn to_reference(value: &Value) -> ExecValue {
    match value {
        Value::Cell(cell) => ExecValue::Cell(reference_cell(*cell)),
        Value::Blob(_) => ExecValue::Blob(Rc::new(value.as_blob().unwrap().to_vec())),
        Value::List(elements) => {
            ExecValue::List(Rc::new(elements.values().map(to_reference).collect()))
        }
        Value::Record(fields) => ExecValue::Record(Rc::new(
            fields
                .iter()
                .map(|(field, value)| (reference_cell(*field), to_reference(value)))
                .collect(),
        )),
    }
}

/// Back to GID, which keeps a repeated field's last value, as the
/// reference reads its pairs.
fn from_reference(value: &ExecValue) -> Value {
    match value {
        ExecValue::Cell(cell) => Value::Cell(CellId::from_u128(*cell)),
        ExecValue::Blob(bytes) => Value::from(bytes.to_vec()),
        ExecValue::List(elements) => Value::list(elements.iter().map(from_reference)),
        ExecValue::Record(fields) => Value::record(
            fields
                .iter()
                .map(|(field, value)| (CellId::from_u128(*field), from_reference(value))),
        ),
    }
}

/// `value` without the body origins closures carry for click-to-source.
fn without_origins(value: &Value) -> Value {
    match value {
        Value::List(elements) => Value::list(elements.values().map(without_origins)),
        Value::Record(fields) => Value::record(fields.iter().map(|(field, value)| {
            (
                *field,
                match (field, value) {
                    (&vocabulary::CLOSURE, Value::Record(closure)) => Value::record(
                        closure
                            .iter()
                            .filter(|(field, _)| *field != source::vocabulary::BODY_ORIGIN)
                            .map(|(field, value)| (*field, without_origins(value))),
                    ),
                    _ => without_origins(value),
                },
            )
        })),
        _ => value.clone(),
    }
}

/// A host definition: Grap's own, or a foreign function's description.
enum Defined {
    Grap(Value),
    Foreign(Value),
}

/// Evaluates under both and checks they agree; `None` when the program
/// calls a foreign function, so the definition says nothing about it.
fn agree(expression: &Value, host: &[(CellId, Defined)], fuel: usize) -> Option<Evaluation<Value>> {
    let reference_host: Vec<_> = host
        .iter()
        .map(|(cell, defined)| {
            (
                reference_cell(*cell),
                match defined {
                    Defined::Grap(value) => ExecDefinition::Grap(to_reference(value)),
                    Defined::Foreign(value) => ExecDefinition::Foreign(to_reference(value)),
                },
            )
        })
        .collect();
    let expected = reference::evaluate(&to_reference(expression), &reference_host, fuel)?;
    let resolver = TestHost(|cell| {
        host.iter()
            .filter(|(defined_cell, _)| *defined_cell == cell)
            .map(|(_, defined)| {
                (
                    Resolution::Document,
                    match defined {
                        Defined::Grap(value) => Definition::Value(value.clone()),
                        Defined::Foreign(value) => {
                            Definition::Foreign(Rc::new(ForeignDefinition {
                                value: value.clone(),
                                implementation: ForeignFunction::new(|_, _, _| {
                                    Ok(RuntimeValue::from(Value::record([])))
                                }),
                            }))
                        }
                    },
                )
            })
            .collect()
    });
    let actual = evaluate_value(expression, &resolver, fuel);
    assert_eq!(
        without_origins(&actual.result),
        from_reference(&expected.result),
        "result of {expression:?} with fuel {fuel}"
    );
    assert_eq!(
        actual.remaining_fuel, expected.remaining_fuel,
        "fuel left by {expression:?} with fuel {fuel}"
    );
    assert_eq!(
        actual.completed, expected.completed,
        "completion of {expression:?} with fuel {fuel}"
    );
    Some(actual)
}

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

#[test]
fn the_reference_names_grap_cells() {
    let named = [
        (reference::FUNCTION, vocabulary::FUNCTION),
        (reference::PARAMS, vocabulary::PARAMS),
        (reference::BODY, vocabulary::BODY),
        (reference::VALUE, vocabulary::VALUE),
        (reference::CLOSURE, vocabulary::CLOSURE),
        (reference::ENVIRONMENT, vocabulary::ENVIRONMENT),
        (reference::FFI, vocabulary::FFI),
        (reference::BODY_ORIGIN, source::vocabulary::BODY_ORIGIN),
        (reference::ABSENT, absent::ABSENT),
        (reference::ABSENT_CELL, absent::CELL),
        (reference::ABSENT_VALUE, absent::VALUE),
        (reference::CYCLE, absent::CYCLE),
        (reference::FUEL_EXHAUSTED, absent::FUEL_EXHAUSTED),
        (reference::MISSING_CELL, absent::MISSING_CELL),
        (reference::CELL_CYCLE, absent::CELL_CYCLE),
        (reference::MALFORMED_LAMBDA, absent::MALFORMED_LAMBDA),
        (reference::INVALID_PARAMETER, absent::INVALID_PARAMETER),
        (reference::NOT_CALLABLE, absent::NOT_CALLABLE),
        (reference::MISSING_ARGUMENT, absent::MISSING_ARGUMENT),
    ];
    for (reference, grap) in named {
        assert_eq!(CellId::from_u128(reference), grap);
    }
}

/// The core cases of progred's `conformance.rs`, with their pinned results.
#[test]
fn the_reference_reproduces_the_core_conformance_tuples() {
    let x = new_cell_id();
    let call = super::call(super::lambda([x], Value::from(x)), [(x, blob("bound"))]);
    let evaluation = agree(&call, &[], 10).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (blob("bound"), 6)
    );

    let foreign = new_cell_id();
    let shadowing = super::call(
        super::lambda([foreign], Value::from(foreign)),
        [(foreign, blob("bound"))],
    );
    let host = [(foreign, Defined::Foreign(Value::record([])))];
    let evaluation = agree(&shadowing, &host, 10).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (blob("bound"), 6)
    );

    let (first, second) = (new_cell_id(), new_cell_id());
    let host = [
        (first, Defined::Grap(Value::from(second))),
        (second, Defined::Grap(blob("end"))),
    ];
    let evaluation = agree(&Value::from(first), &host, 10).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (blob("end"), 7)
    );

    let missing = new_cell_id();
    let evaluation = agree(&Value::from(missing), &[], 10).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (
            absent::with_detail(absent::MISSING_CELL, absent::CELL, missing.into()),
            9
        )
    );
}

/// Cases the definition spells out because the evaluator's paths differ.
#[test]
fn the_reference_agrees_on_the_edges_it_spells_out() {
    let (f, g, x) = (new_cell_id(), new_cell_id(), new_cell_id());
    let missing = absent::with_detail(absent::MISSING_CELL, absent::CELL, x.into());
    // Calling a cell whose definition is an absent answers that absent as it
    // is; calling a binding to one answers that it isn't callable.
    let host = [(f, Defined::Grap(Value::from(x)))];
    let through_cell = agree(&super::call(Value::from(f), []), &host, 20).unwrap();
    assert_eq!(through_cell.result, missing);
    let through_binding = super::call(
        super::lambda([g], super::call(Value::from(g), [])),
        [(g, Value::from(x))],
    );
    let evaluation = agree(&through_binding, &[], 20).unwrap();
    assert_eq!(
        evaluation.result,
        absent::with_detail(absent::NOT_CALLABLE, absent::VALUE, missing)
    );
    // A reference notices a cycle only through another reference; a call
    // notices one through anything under resolution. So calling a cell
    // defined as a reference to itself evaluates the definition twice
    // before the reference notices.
    let host = [(f, Defined::Grap(Value::from(f)))];
    agree(&Value::from(f), &host, 20).unwrap();
    let evaluation = agree(&super::call(Value::from(f), []), &host, 20).unwrap();
    assert_eq!(evaluation.remaining_fuel, 16);
    let host = [(f, Defined::Grap(super::call(Value::from(f), [])))];
    agree(&super::call(Value::from(f), []), &host, 20).unwrap();
    agree(&Value::from(f), &host, 20).unwrap();
    // Every budget from nothing up, for a program that spends a few units.
    let program = super::call(super::lambda([x], Value::from(x)), [(x, blob("bound"))]);
    for fuel in 0..8 {
        agree(&program, &[], fuel).unwrap();
    }
}

/// Random core programs: lambdas with good and bad parameters, calls through
/// lambdas, bindings, cells, closure records and `{ffi: cell}`, value
/// wrappers, data, and definitions that refer to each other and themselves,
/// with budgets small enough to run out.
struct Programs {
    state: u64,
    variables: Vec<CellId>,
    defined: Vec<CellId>,
    foreign: Vec<CellId>,
    missing: Vec<CellId>,
}

impl Programs {
    fn new(seed: u64) -> Self {
        let cells = |n| (0..n).map(|_| new_cell_id()).collect::<Vec<_>>();
        Self {
            state: seed,
            variables: cells(4),
            defined: cells(4),
            foreign: cells(1),
            missing: cells(2),
        }
    }

    fn below(&mut self, n: usize) -> usize {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.state >> 33) % n as u64) as usize
    }

    fn pick(&mut self, cells: &[CellId]) -> CellId {
        cells[self.below(cells.len())]
    }

    fn cell(&mut self) -> CellId {
        match self.below(10) {
            0..=4 => self.pick(&self.variables.clone()),
            5..=7 => self.pick(&self.defined.clone()),
            8 => self.pick(&self.foreign.clone()),
            _ => self.pick(&self.missing.clone()),
        }
    }

    fn leaf(&mut self) -> Value {
        match self.below(5) {
            0 => blob(["a", "b", "c"][self.below(3)]),
            1 => Value::record([]),
            2 => Value::list([blob("item")]),
            _ => Value::from(self.cell()),
        }
    }

    fn expression(&mut self, depth: usize) -> Value {
        if depth == 0 {
            return self.leaf();
        }
        match self.below(12) {
            0 | 1 => self.leaf(),
            2 | 3 => self.lambda(depth),
            4..=7 => self.call(depth),
            8 => Value::record([(vocabulary::VALUE, self.expression(depth - 1))]),
            9 => self.closure_record(depth),
            10 => super::ffi(match self.below(2) {
                0 => self.pick(&self.defined.clone()),
                _ => self.pick(&self.foreign.clone()),
            }),
            _ => Value::list([self.expression(depth - 1), self.leaf()]),
        }
    }

    fn parameters(&mut self) -> Value {
        match self.below(10) {
            0 => blob("not a list"),
            1 => Value::list([Value::from(self.cell()), blob("not a cell")]),
            _ => {
                let count = self.below(3);
                Value::list((0..count).map(|_| Value::from(self.pick(&self.variables.clone()))))
            }
        }
    }

    fn lambda(&mut self, depth: usize) -> Value {
        let mut fields = vec![
            (vocabulary::PARAMS, self.parameters()),
            (vocabulary::BODY, self.expression(depth - 1)),
        ];
        if self.below(4) == 0 {
            fields.push((self.pick(&self.missing.clone()), blob("named")));
        }
        Value::record(fields)
    }

    fn call(&mut self, depth: usize) -> Value {
        let function = match self.below(4) {
            0 => self.lambda(depth),
            1 | 2 => Value::from(self.cell()),
            _ => self.expression(depth - 1),
        };
        let count = self.below(3);
        let arguments: Vec<_> = (0..count)
            .map(|_| {
                (
                    self.pick(&self.variables.clone()),
                    self.expression(depth - 1),
                )
            })
            .collect();
        super::call(function, arguments)
    }

    fn closure_record(&mut self, depth: usize) -> Value {
        Value::record([(
            vocabulary::CLOSURE,
            Value::record([
                (vocabulary::PARAMS, self.parameters()),
                (vocabulary::BODY, self.expression(depth - 1)),
                (
                    vocabulary::ENVIRONMENT,
                    Value::record([(self.pick(&self.variables.clone()), self.leaf())]),
                ),
            ]),
        )])
    }

    fn host(&mut self) -> Vec<(CellId, Defined)> {
        let mut host: Vec<_> = self
            .defined
            .clone()
            .into_iter()
            .filter_map(|cell| {
                if self.below(5) == 0 {
                    None
                } else {
                    let depth = 1 + self.below(3);
                    let definition = match self.below(3) {
                        0 => self.lambda(depth),
                        _ => self.expression(depth),
                    };
                    Some((cell, Defined::Grap(definition)))
                }
            })
            .collect();
        host.extend(self.foreign.clone().into_iter().map(|cell| {
            (
                cell,
                Defined::Foreign(Value::record([(cell, blob("name"))])),
            )
        }));
        host
    }
}

#[test]
fn the_reference_agrees_on_random_core_programs() {
    let mut programs = Programs::new(0x5eed_0f_9a5_cafe);
    let mut compared = 0;
    let mut completed = 0;
    for _ in 0..20000 {
        let host = programs.host();
        let depth = 1 + programs.below(4);
        let expression = programs.expression(depth);
        let fuel = match programs.below(4) {
            0 => programs.below(6),
            _ => programs.below(80),
        };
        if let Some(evaluation) = agree(&expression, &host, fuel) {
            compared += 1;
            completed += usize::from(evaluation.completed);
        }
    }
    // Enough of both outcomes to mean something.
    assert!(compared > 12000, "compared {compared}");
    assert!(
        completed > 6000 && compared - completed > 1000,
        "completed {completed}"
    );
}
