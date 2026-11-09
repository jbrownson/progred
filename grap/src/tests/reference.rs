//! Grap's evaluator against `grap-reference`, the definition of Grap's core:
//! each program here must come to the same result, fuel left and halt under
//! both, unless it calls a foreign function the definition leaves out. The
//! number operations are registered as the numbers library registers them.
//! Results are compared whole, with the origins closures and some absents
//! carry, which the definition defines too.

use super::*;
use gid::{Position, Record, Step, new_cell_id};
use grap_reference::{self as reference, ExecDefinition, ExecValue};
use primitive::{Binary, Unary};
use vocabulary::kind;

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

/// `value` with every float NaN made the same NaN. Rust leaves a NaN
/// result's sign and payload unspecified, even between two places that run
/// the same operation, so Grap's evaluator and the reference can differ in
/// them; the definition names float operations rather than defining them for
/// that reason.
fn canonical_nans(value: &Value) -> Value {
    let canonical = |number: &Value, width: usize, nan: Vec<u8>| -> Option<Value> {
        let bytes = number.as_blob()?;
        let is_nan = match width {
            8 => f64::from_le_bytes(bytes.try_into().ok()?).is_nan(),
            _ => f32::from_le_bytes(bytes.try_into().ok()?).is_nan(),
        };
        is_nan.then(|| Value::from(nan))
    };
    match value {
        Value::List(elements) => Value::list(elements.values().map(canonical_nans)),
        Value::Record(fields) => Value::record(fields.iter().map(|(field, value)| {
            let replaced = match *field {
                crate::f64::F64 => canonical(value, 8, f64::NAN.to_le_bytes().to_vec()),
                crate::f32::F32 => canonical(value, 4, f32::NAN.to_le_bytes().to_vec()),
                _ => None,
            };
            (*field, replaced.unwrap_or_else(|| canonical_nans(value)))
        })),
        _ => value.clone(),
    }
}

/// A host definition: a value with nothing implemented under its cell; a
/// foreign function the definition leaves out, or a number operation, as it
/// describes itself; or a value that isn't a foreign function under a cell
/// the host implements anyway, which calling the cell evaluates.
enum Defined {
    Grap(Value),
    Foreign(Record),
    Primitive(Record, reference::Primitive, primitive::Primitive),
    Shadowed(Value),
}

fn description(fields: impl IntoIterator<Item = (CellId, Value)>) -> Record {
    fields.into_iter().collect()
}

/// A foreign function's code the definition leaves out.
fn foreign_code() -> ForeignFunction<'static> {
    ForeignFunction::new(|_, _, _| Ok(RuntimeValue::from(Value::record([]))))
}

/// Each cell's value and implementation, as Grap's host gives them.
type Entries = Vec<(CellId, Value, Option<ForeignFunction<'static>>)>;

fn entries(host: &[(CellId, Defined)]) -> Entries {
    host.iter()
        .map(|(cell, defined)| match defined {
            Defined::Grap(value) => (*cell, value.clone(), None),
            Defined::Foreign(description) => {
                let code = foreign_code();
                let definition = TestDefinition::foreign(*cell, description.clone(), code.clone());
                (*cell, definition.value().clone(), Some(code))
            }
            Defined::Primitive(description, _, operation) => {
                let code = operation.function();
                let definition = TestDefinition::foreign(*cell, description.clone(), code.clone());
                (*cell, definition.value().clone(), Some(code))
            }
            Defined::Shadowed(value) => (*cell, value.clone(), Some(foreign_code())),
        })
        .collect()
}

/// The same host as the reference holds it.
fn reference_host(host: &[(CellId, Defined)], entries: &Entries) -> Vec<(u128, ExecDefinition)> {
    host.iter()
        .zip(entries)
        .map(|((cell, defined), (_, value, _))| {
            let implementation = match defined {
                Defined::Grap(_) => None,
                Defined::Foreign(_) | Defined::Shadowed(_) => {
                    Some(reference::Implementation::Foreign)
                }
                Defined::Primitive(_, named, _) => {
                    Some(reference::Implementation::Primitive(*named))
                }
            };
            (
                reference_cell(*cell),
                ExecDefinition {
                    value: to_reference(value),
                    implementation,
                },
            )
        })
        .collect()
}

/// A host answering with each cell's first entry, as the reference resolves
/// a cell.
struct ListedHost(Entries);

impl Host for ListedHost {
    fn definition(&self, cell: CellId) -> Option<Value> {
        let (_, value, _) = self.0.iter().find(|(defined, _, _)| *defined == cell)?;
        Some(value.clone())
    }

    fn implementation(&self, cell: CellId) -> Option<ForeignFunction<'static>> {
        let (_, _, code) = self.0.iter().find(|(defined, _, _)| *defined == cell)?;
        code.clone()
    }
}

fn halt(halt: reference::Halt) -> Halt {
    match halt {
        reference::Halt::FuelExhausted => Halt::FuelExhausted,
        reference::Halt::DepthExhausted => Halt::DepthExhausted,
    }
}

/// What a program came to under every evaluator.
struct Agreed {
    result: Value,
    remaining_fuel: usize,
    halt: Option<Halt>,
}

/// Evaluates under each and checks they agree; `None` when the program calls
/// a foreign function, so the definition says nothing about it.
fn agree(expression: &Value, host: &[(CellId, Defined)], fuel: usize) -> Option<Agreed> {
    let entries = entries(host);
    let reference_host = reference_host(host, &entries);
    let expected = reference::evaluate(&to_reference(expression), &reference_host, fuel);
    let fast = reference::fast::evaluate(&to_reference(expression), &reference_host, fuel);
    match (&expected, &fast) {
        (Some(expected), Some(fast)) => {
            assert_eq!(
                canonical_nans(&from_reference(&fast.result)),
                canonical_nans(&from_reference(&expected.result)),
                "fast result of {expression:?} with fuel {fuel}"
            );
            assert_eq!(
                (fast.remaining_fuel, fast.halt),
                (expected.remaining_fuel, expected.halt),
                "fast fuel and halt of {expression:?} with fuel {fuel}"
            );
        }
        (None, None) => {}
        _ => panic!(
            "only one of the reference and the fast evaluator left the definition for {expression:?}"
        ),
    }
    let expected = expected?;
    let host = ListedHost(entries);
    let actual = Evaluator::new(&host, fuel).evaluate(expression);
    let result = actual.result.into_value();
    assert_eq!(
        canonical_nans(&result),
        canonical_nans(&from_reference(&expected.result)),
        "result of {expression:?} with fuel {fuel}"
    );
    assert_eq!(
        actual.remaining_fuel, expected.remaining_fuel,
        "fuel left by {expression:?} with fuel {fuel}"
    );
    assert_eq!(
        actual.halt,
        expected.halt.map(halt),
        "halt of {expression:?} with fuel {fuel}"
    );
    Some(Agreed {
        result,
        remaining_fuel: actual.remaining_fuel,
        halt: actual.halt,
    })
}

/// Runs `test` on a stack deep enough for the reference evaluators, which
/// recurse further than Grap's for each evaluation nested.
fn deep(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

/// The numbers library's operations, registered as it registers them, under
/// cells of their own, with their labels and failure reasons.
struct Numbers {
    left: CellId,
    right: CellId,
    operand: CellId,
    overflow: CellId,
    division_by_zero: CellId,
    operations: Vec<(CellId, reference::Primitive, primitive::Primitive)>,
}

impl Numbers {
    fn new() -> Self {
        let (left, right, operand) = (new_cell_id(), new_cell_id(), new_cell_id());
        let (overflow, division_by_zero) = (new_cell_id(), new_cell_id());
        let binary = |reference_operation, operation| {
            (
                new_cell_id(),
                reference::Primitive::Binary {
                    operation: reference_operation,
                    left: reference_cell(left),
                    right: reference_cell(right),
                },
                primitive::Primitive::Binary {
                    operation,
                    left,
                    right,
                },
            )
        };
        let unary = |reference_operation, operation| {
            (
                new_cell_id(),
                reference::Primitive::Unary {
                    operation: reference_operation,
                    operand: reference_cell(operand),
                },
                primitive::Primitive::Unary { operation, operand },
            )
        };
        use primitive::BinaryOperation::{F32, F64, U64};
        use reference::BinaryOperation as Named;
        let (overflows, divides_by_zero) =
            (reference_cell(overflow), reference_cell(division_by_zero));
        let operations = vec![
            binary(Named::F64Sum, F64(Binary::Arithmetic(|a, b| a + b))),
            binary(Named::F64Multiply, F64(Binary::Arithmetic(|a, b| a * b))),
            binary(Named::F64Subtract, F64(Binary::Arithmetic(|a, b| a - b))),
            binary(Named::F64Divide, F64(Binary::Arithmetic(|a, b| a / b))),
            binary(Named::F64Min, F64(Binary::Arithmetic(|a, b| a.min(b)))),
            binary(Named::F64Max, F64(Binary::Arithmetic(|a, b| a.max(b)))),
            binary(Named::F64Hypot, F64(Binary::Arithmetic(|a, b| a.hypot(b)))),
            binary(Named::F64Less, F64(Binary::Comparison(|a, b| a < b))),
            binary(Named::F64Equal, F64(Binary::Comparison(|a, b| a == b))),
            binary(Named::F32Sum, F32(Binary::Arithmetic(|a, b| a + b))),
            binary(Named::F32Subtract, F32(Binary::Arithmetic(|a, b| a - b))),
            binary(Named::F32Multiply, F32(Binary::Arithmetic(|a, b| a * b))),
            binary(Named::F32Divide, F32(Binary::Arithmetic(|a, b| a / b))),
            binary(Named::F32Less, F32(Binary::Comparison(|a, b| a < b))),
            binary(Named::F32Equal, F32(Binary::Comparison(|a, b| a == b))),
            binary(
                Named::U64Sum(overflows),
                U64(Binary::Checked(|a, b| a.checked_add(b), overflow)),
            ),
            binary(
                Named::U64Subtract(overflows),
                U64(Binary::Checked(|a, b| a.checked_sub(b), overflow)),
            ),
            binary(
                Named::U64Multiply(overflows),
                U64(Binary::Checked(|a, b| a.checked_mul(b), overflow)),
            ),
            binary(
                Named::U64Divide(divides_by_zero),
                U64(Binary::Checked(|a, b| a.checked_div(b), division_by_zero)),
            ),
            binary(Named::U64Less, U64(Binary::Comparison(|a, b| a < b))),
            binary(Named::U64Equal, U64(Binary::Comparison(|a, b| a == b))),
            unary(
                reference::UnaryOperation::F64Sin,
                primitive::UnaryOperation::F64(Unary::Arithmetic(|a| a.sin())),
            ),
            unary(
                reference::UnaryOperation::F64Cos,
                primitive::UnaryOperation::F64(Unary::Arithmetic(|a| a.cos())),
            ),
            unary(
                reference::UnaryOperation::F64Floor,
                primitive::UnaryOperation::F64(Unary::Arithmetic(|a| a.floor())),
            ),
            unary(
                reference::UnaryOperation::F64Ceil,
                primitive::UnaryOperation::F64(Unary::Arithmetic(|a| a.ceil())),
            ),
            unary(
                reference::UnaryOperation::F64IsFinite,
                primitive::UnaryOperation::F64(Unary::Predicate(|a| a.is_finite())),
            ),
        ];
        Self {
            left,
            right,
            operand,
            overflow,
            division_by_zero,
            operations,
        }
    }

    fn host(&self) -> Vec<(CellId, Defined)> {
        self.operations
            .iter()
            .map(|(cell, named, operation)| {
                (
                    *cell,
                    Defined::Primitive(description([(*cell, blob("name"))]), *named, *operation),
                )
            })
            .collect()
    }

    /// The operation's cell.
    fn cell(&self, named: impl Fn(&reference::Primitive) -> bool) -> CellId {
        self.operations
            .iter()
            .find(|(_, operation, _)| named(operation))
            .map(|(cell, _, _)| *cell)
            .unwrap()
    }

    /// The cell of the binary operation of `operation`'s kind, whatever
    /// reason it names.
    fn binary(&self, operation: reference::BinaryOperation) -> CellId {
        self.cell(|named| {
            matches!(named, reference::Primitive::Binary { operation: candidate, .. }
                if std::mem::discriminant(candidate) == std::mem::discriminant(&operation))
        })
    }

    fn unary(&self, operation: reference::UnaryOperation) -> CellId {
        self.cell(|named| {
            matches!(named, reference::Primitive::Unary { operation: candidate, .. } if *candidate == operation)
        })
    }

    /// The labels the operation reads.
    fn labels(&self, operation: &reference::Primitive) -> Vec<CellId> {
        match operation {
            reference::Primitive::Binary { .. } => vec![self.left, self.right],
            reference::Primitive::Unary { .. } => vec![self.operand],
        }
    }

    fn call(&self, cell: CellId, operands: impl IntoIterator<Item = (CellId, Value)>) -> Value {
        super::call(Value::from(cell), operands)
    }
}

/// Operands for every operation: each type's edge values, including NaNs
/// with payloads and of both signs, infinities, zeros of both signs and
/// subnormals, and values of the wrong type or none.
fn number_operands() -> Vec<Value> {
    let f64s = [
        0.0,
        -0.0,
        1.0,
        -1.5,
        2.25,
        0.1,
        1e300,
        -1e-300,
        5e-324,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001),
        f64::from_bits(0x7ff8_dead_beef_0001),
    ];
    let f32s = [
        0.0,
        -0.0,
        1.0,
        -1.5,
        3.4e38,
        1e-45,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0x7fa0_0001),
    ];
    let u64s = [0, 1, 2, 3, 7, 1 << 32, 1 << 63, u64::MAX - 1, u64::MAX];
    f64s.into_iter()
        .map(crate::f64::value)
        .chain(f32s.into_iter().map(crate::f32::value))
        .chain(u64s.into_iter().map(crate::u64::value))
        .chain([
            crate::logic::value(true),
            blob("not a number"),
            Value::record([(crate::f64::F64, Value::from(vec![0u8; 4]))]),
            Value::record([(crate::u64::U64, blob("eight by"))]),
            Value::record([
                (crate::f64::F64, Value::from(2.5f64.to_le_bytes().to_vec())),
                (crate::u64::U64, Value::from(9u64.to_le_bytes().to_vec())),
                (crate::f32::F32, Value::from(0.5f32.to_le_bytes().to_vec())),
            ]),
            absent::absent(
                absent::DECLINED,
                [
                    (crate::f64::F64, Value::from(4.0f64.to_le_bytes().to_vec())),
                    (crate::f32::F32, Value::from(4.0f32.to_le_bytes().to_vec())),
                    (crate::u64::U64, Value::from(4u64.to_le_bytes().to_vec())),
                ],
            ),
            absent::value(absent::DECLINED),
        ])
        .collect()
}

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

/// Where the input an evaluation was handed is.
fn input() -> SourceOrigin {
    SourceOrigin::Input(Vec::new())
}

/// `{isa: value wrapper, value}`.
fn wrapped(value: Value) -> Value {
    isa::record(kind::VALUE_WRAPPER, [(vocabulary::VALUE, value)])
}

#[test]
fn the_reference_names_grap_cells() {
    let named = [
        (reference::ISA, isa::ISA),
        (reference::KIND_CALL, kind::CALL),
        (reference::KIND_LAMBDA, kind::LAMBDA),
        (reference::KIND_CLOSURE, kind::CLOSURE),
        (reference::KIND_VALUE_WRAPPER, kind::VALUE_WRAPPER),
        (reference::KIND_FOREIGN_FUNCTION, kind::FOREIGN_FUNCTION),
        (reference::KIND_ABSENT, kind::ABSENT),
        (reference::FUNCTION, vocabulary::FUNCTION),
        (reference::PARAMS, vocabulary::PARAMS),
        (reference::BODY, vocabulary::BODY),
        (reference::VALUE, vocabulary::VALUE),
        (reference::LAMBDA, vocabulary::LAMBDA),
        (reference::ENVIRONMENT, vocabulary::ENVIRONMENT),
        (reference::IMPLEMENTATION, vocabulary::IMPLEMENTATION),
        (reference::BODY_ORIGIN, source::vocabulary::BODY_ORIGIN),
        (reference::REASON, absent::REASON),
        (reference::ABSENT_CELL, absent::CELL),
        (reference::ABSENT_VALUE, absent::VALUE),
        (reference::ABSENT_CALL, absent::CALL),
        (reference::FUEL_EXHAUSTED, absent::FUEL_EXHAUSTED),
        (reference::DEPTH_EXHAUSTED, absent::DEPTH_EXHAUSTED),
        (reference::MISSING_CELL, absent::MISSING_CELL),
        (reference::MALFORMED_LAMBDA, absent::MALFORMED_LAMBDA),
        (reference::INVALID_PARAMETER, absent::INVALID_PARAMETER),
        (reference::NOT_CALLABLE, absent::NOT_CALLABLE),
        (reference::MISSING_ARGUMENT, absent::MISSING_ARGUMENT),
        (reference::MISSING_FIELD, absent::MISSING_FIELD),
        (reference::INVALID_ARGUMENTS, absent::INVALID_ARGUMENTS),
        (reference::ORIGIN_INPUT, source::vocabulary::INPUT),
        (reference::ORIGIN_ROOT, source::vocabulary::ROOT),
        (reference::ORIGIN_CELL, source::vocabulary::CELL),
        (reference::ORIGIN_PATH, source::vocabulary::PATH),
        (reference::STEP_KEY, path::vocabulary::KEY),
        (reference::STEP_ELEMENT, path::vocabulary::ELEMENT),
        (reference::STEP_FOLLOW, path::vocabulary::FOLLOW),
        (reference::STEP_INDEXABLE, path::vocabulary::INDEXABLE),
        (reference::F64, crate::f64::F64),
        (reference::NOT_F64, crate::f64::NOT_F64),
        (reference::F32, crate::f32::F32),
        (reference::NOT_F32, crate::f32::NOT_F32),
        (reference::U64, crate::u64::U64),
        (reference::NOT_U64, crate::u64::NOT_U64),
        (reference::BOOL, crate::logic::BOOL),
        (reference::TRUE, crate::logic::TRUE),
        (reference::FALSE, crate::logic::FALSE),
    ];
    for (reference, grap) in named {
        assert_eq!(CellId::from_u128(reference), grap);
    }
    assert_eq!(reference::CHAIN_LIMIT, absent::CHAIN_LIMIT);
    assert_eq!(reference::MAX_DEPTH, MAX_DEPTH);
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
    let host = [(foreign, Defined::Foreign(description([])))];
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

/// A record is a form only when its `isa` says so; what a form lacks it
/// answers why; and what closures and their absents carry.
#[test]
fn the_reference_agrees_on_what_records_say_they_are() {
    let (x, y) = (new_cell_id(), new_cell_id());
    let lambda = super::lambda([x], Value::from(x));
    // Without `isa`, or with one that isn't a cell or names no form, a
    // record is data, whatever its fields.
    for data in [
        Value::record([
            (vocabulary::FUNCTION, lambda.clone()),
            (x, blob("argument")),
        ]),
        Value::record([
            (vocabulary::PARAMS, Value::list([Value::from(x)])),
            (vocabulary::BODY, Value::from(x)),
        ]),
        Value::record([(vocabulary::VALUE, Value::from(x))]),
        Value::record([
            (isa::ISA, blob("call")),
            (vocabulary::FUNCTION, lambda.clone()),
        ]),
        isa::record(kind::CLOSURE, [(vocabulary::LAMBDA, lambda.clone())]),
        crate::foreign_function(x),
        absent::value(absent::DECLINED),
    ] {
        let evaluation = agree(&data, &[], 10).unwrap();
        assert_eq!((evaluation.result, evaluation.remaining_fuel), (data, 9));
    }
    // A form lacking a field its kind requires answers why where the field
    // would be, which costs a unit as the field would.
    let missing = |field| absent::missing_field(field);
    let evaluation = agree(&isa::record(kind::CALL, [(x, blob("a"))]), &[], 10).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (missing(vocabulary::FUNCTION), 8)
    );
    let evaluation = agree(&isa::record(kind::VALUE_WRAPPER, []), &[], 10).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (missing(vocabulary::VALUE), 8)
    );
    let bodiless = isa::record(
        kind::LAMBDA,
        [(vocabulary::PARAMS, Value::list([Value::from(x)]))],
    );
    let evaluation = agree(&super::call(bodiless.clone(), []), &[], 10).unwrap();
    assert_eq!(evaluation.result, missing(vocabulary::BODY));
    // A lambda without a list of parameters, or with one that isn't a cell.
    let paramless = isa::record(kind::LAMBDA, [(vocabulary::BODY, Value::from(x))]);
    assert_eq!(
        agree(&paramless, &[], 10).unwrap().result,
        absent::with_detail(absent::MALFORMED_LAMBDA, absent::VALUE, paramless.clone())
    );
    let invalid = super::lambda([], Value::from(x));
    let invalid = match invalid {
        Value::Record(mut fields) => {
            fields.set(vocabulary::PARAMS, Value::list([blob("not a cell")]));
            Value::Record(fields)
        }
        _ => unreachable!(),
    };
    assert_eq!(
        agree(&invalid, &[], 10).unwrap().result,
        absent::with_detail(absent::INVALID_PARAMETER, absent::VALUE, blob("not a cell"))
    );
    // A closure holds its lambda as written, with why its body is missing if
    // it is, what it captured, and where its body was written.
    let evaluation = agree(
        &super::call(
            super::lambda([y], bodiless.clone()),
            [(y, blob("captured"))],
        ),
        &[],
        10,
    )
    .unwrap();
    let body_at = |path: Vec<Step>| source::value(&SourceOrigin::Input(path));
    assert_eq!(
        evaluation.result,
        isa::record(
            kind::CLOSURE,
            [
                (
                    vocabulary::LAMBDA,
                    isa::record(
                        kind::LAMBDA,
                        [
                            (vocabulary::PARAMS, Value::list([Value::from(x)])),
                            (vocabulary::BODY, missing(vocabulary::BODY)),
                        ]
                    ),
                ),
                (
                    vocabulary::ENVIRONMENT,
                    Value::record([(y, blob("captured"))])
                ),
                (
                    source::vocabulary::BODY_ORIGIN,
                    body_at(vec![
                        Step::Key(vocabulary::FUNCTION),
                        Step::Key(vocabulary::BODY),
                        Step::Key(vocabulary::BODY),
                    ])
                ),
            ]
        )
    );
    // A missing argument names where its call was written, which a closure
    // record whose body origin doesn't read as one doesn't know.
    let inner = super::call(super::lambda([x], Value::from(x)), []);
    let evaluation = agree(&super::call(super::lambda([], inner.clone()), []), &[], 20).unwrap();
    assert_eq!(
        evaluation.result,
        absent::missing_argument(
            x,
            Some(&SourceOrigin::Input(vec![
                Step::Key(vocabulary::FUNCTION),
                Step::Key(vocabulary::BODY),
            ]))
        )
    );
    let record = |body_origin: Value| {
        isa::record(
            kind::CLOSURE,
            [
                (vocabulary::LAMBDA, super::lambda([], inner.clone())),
                (vocabulary::ENVIRONMENT, Value::record([])),
                (source::vocabulary::BODY_ORIGIN, body_origin),
            ],
        )
    };
    let unreadable = Value::record([
        (source::vocabulary::INPUT, Value::list([])),
        (source::vocabulary::ROOT, Value::list([])),
    ]);
    let evaluation = agree(&super::call(record(unreadable), []), &[], 20).unwrap();
    assert_eq!(evaluation.result, absent::missing_argument(x, None));
    let elsewhere = SourceOrigin::Cell {
        cell: y,
        path: vec![
            Step::Element(Position::from_bytes(vec![0x80]).unwrap()),
            Step::Follow,
        ],
    };
    let evaluation = agree(&super::call(record(source::value(&elsewhere)), []), &[], 20).unwrap();
    assert_eq!(
        evaluation.result,
        absent::missing_argument(x, Some(&elsewhere))
    );
}

/// Cases the definition spells out because the evaluator's paths differ.
#[test]
fn the_reference_agrees_on_the_edges_it_spells_out() {
    deep(|| {
        let (f, g, x) = (new_cell_id(), new_cell_id(), new_cell_id());
        let missing = absent::with_detail(absent::MISSING_CELL, absent::CELL, x.into());
        // Calling a cell whose definition is an absent, or a binding to one,
        // answers that absent as it is.
        let host = [(f, Defined::Grap(Value::from(x)))];
        let through_cell = agree(&super::call(Value::from(f), []), &host, 20).unwrap();
        assert_eq!(through_cell.result, missing);
        let through_binding = super::call(
            super::lambda([g], super::call(Value::from(g), [])),
            [(g, Value::from(x))],
        );
        assert_eq!(agree(&through_binding, &[], 20).unwrap().result, missing);
        // A definition that refers to itself runs until its fuel does, or
        // until evaluations nest too deeply: each reference nests one, each
        // spending a unit, so 257 run, the last halting.
        let host = [(f, Defined::Grap(Value::from(f)))];
        let evaluation = agree(&Value::from(f), &host, 20).unwrap();
        assert_eq!(
            (evaluation.halt, evaluation.remaining_fuel),
            (Some(Halt::FuelExhausted), 0)
        );
        let evaluation = agree(&Value::from(f), &host, 1000).unwrap();
        assert_eq!(
            (
                evaluation.result,
                evaluation.halt,
                evaluation.remaining_fuel
            ),
            (
                absent::value(absent::DEPTH_EXHAUSTED),
                Some(Halt::DepthExhausted),
                1000 - (MAX_DEPTH + 1)
            )
        );
        let host = [(f, Defined::Grap(super::call(Value::from(f), [])))];
        agree(&super::call(Value::from(f), []), &host, 2000).unwrap();
        agree(&Value::from(f), &host, 2000).unwrap();
        // A foreign function without its implementation isn't callable; one
        // implemented under another cell calls that cell; and a value under
        // a cell the host implements is called as code unless it's a
        // foreign function implemented there.
        let host = [(f, Defined::Grap(crate::foreign_function(f)))];
        assert_eq!(
            agree(&super::call(Value::from(f), []), &host, 20)
                .unwrap()
                .result,
            absent::with_detail(
                absent::NOT_CALLABLE,
                absent::VALUE,
                crate::foreign_function(f)
            )
        );
        let host = [
            (f, Defined::Grap(crate::foreign_function(g))),
            (g, Defined::Grap(super::lambda([], blob("g")))),
        ];
        let evaluation = agree(&super::call(Value::from(f), []), &host, 20).unwrap();
        assert_eq!(
            (evaluation.result, evaluation.remaining_fuel),
            (blob("g"), 14)
        );
        let host = [(f, Defined::Shadowed(super::lambda([], blob("code"))))];
        assert_eq!(
            agree(&super::call(Value::from(f), []), &host, 20)
                .unwrap()
                .result,
            blob("code")
        );
        let host = [(f, Defined::Foreign(description([])))];
        assert!(agree(&super::call(Value::from(f), []), &host, 20).is_none());
        // Every budget from nothing up, for a program that spends a few units.
        let program = super::call(super::lambda([x], Value::from(x)), [(x, blob("bound"))]);
        for fuel in 0..8 {
            agree(&program, &[], fuel).unwrap();
        }
    });
}

/// Every number operation on every pair of operands from the spread, and the
/// rules around them spelled out with their results.
#[test]
fn the_reference_agrees_on_number_operations() {
    let numbers = Numbers::new();
    let host = numbers.host();
    let operands = number_operands();
    for (cell, operation, _) in &numbers.operations {
        match numbers.labels(operation)[..] {
            [left, right] => {
                for first in &operands {
                    for second in &operands {
                        let call =
                            numbers.call(*cell, [(left, first.clone()), (right, second.clone())]);
                        agree(&call, &host, 20).unwrap();
                    }
                }
            }
            [operand] => {
                for value in &operands {
                    agree(&numbers.call(*cell, [(operand, value.clone())]), &host, 20).unwrap();
                }
            }
            _ => unreachable!(),
        }
    }

    use reference::BinaryOperation as Named;
    let (left, right) = (numbers.left, numbers.right);
    let sum = numbers.binary(Named::F64Sum);
    let numbered =
        |cell, first: Value, second: Value| numbers.call(cell, [(left, first), (right, second)]);
    // A call, its cell, then each operand: four units.
    let evaluation = agree(
        &numbered(sum, crate::f64::value(1.5), crate::f64::value(2.25)),
        &host,
        20,
    )
    .unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (crate::f64::value(3.75), 16)
    );
    for fuel in 0..6 {
        agree(
            &numbered(sum, crate::f64::value(1.0), crate::f64::value(2.0)),
            &host,
            fuel,
        )
        .unwrap();
    }
    let u64 = |number| crate::u64::value(number);
    let cases = [
        (
            numbered(numbers.binary(Named::U64Sum(0)), u64(u64::MAX), u64(1)),
            absent::value(numbers.overflow),
        ),
        (
            numbered(numbers.binary(Named::U64Subtract(0)), u64(3), u64(5)),
            absent::value(numbers.overflow),
        ),
        (
            numbered(
                numbers.binary(Named::U64Multiply(0)),
                u64(1 << 32),
                u64(1 << 32),
            ),
            absent::value(numbers.overflow),
        ),
        (
            numbered(numbers.binary(Named::U64Divide(0)), u64(7), u64(0)),
            absent::value(numbers.division_by_zero),
        ),
        (
            numbered(numbers.binary(Named::U64Divide(0)), u64(7), u64(2)),
            u64(3),
        ),
        (
            numbered(
                numbers.binary(Named::F64Less),
                crate::f64::value(1.0),
                crate::f64::value(2.0),
            ),
            crate::logic::value(true),
        ),
        (
            numbers.call(
                numbers.unary(reference::UnaryOperation::F64Floor),
                [(numbers.operand, crate::f64::value(-2.5))],
            ),
            crate::f64::value(-3.0),
        ),
        (
            numbers.call(
                numbers.unary(reference::UnaryOperation::F64IsFinite),
                [(numbers.operand, crate::f64::value(f64::NAN))],
            ),
            crate::logic::value(false),
        ),
    ];
    for (call, result) in cases {
        assert_eq!(agree(&call, &host, 20).unwrap().result, result);
    }
    // Why each operand couldn't be used, under its label: missing, not the
    // operation's type, or already absent, passed on as it is; all naming
    // where the call was written.
    let at = input();
    let missing = |label: CellId| absent::missing_argument(label, Some(&at));
    let invalid = |details: Vec<(CellId, Value)>| {
        absent::caused_by_all(absent::INVALID_ARGUMENTS, Some(&at), details)
    };
    let evaluation = agree(&numbers.call(sum, []), &host, 20).unwrap();
    assert_eq!(
        (evaluation.result, evaluation.remaining_fuel),
        (
            invalid(vec![(left, missing(left)), (right, missing(right))]),
            18
        )
    );
    let single = crate::f32::value(1.0);
    let evaluation = agree(
        &numbered(sum, single.clone(), crate::f64::value(1.0)),
        &host,
        20,
    )
    .unwrap();
    assert_eq!(
        evaluation.result,
        invalid(vec![(
            left,
            absent::with_detail(crate::f64::NOT_F64, absent::VALUE, single)
        )])
    );
    // The same absent twice is kept once.
    let undefined = new_cell_id();
    let evaluation = agree(
        &numbered(sum, Value::from(undefined), Value::from(undefined)),
        &host,
        20,
    )
    .unwrap();
    assert_eq!(
        evaluation.result,
        absent::absent(
            absent::INVALID_ARGUMENTS,
            [
                (absent::CALL, source::value(&at)),
                (
                    left,
                    absent::with_detail(absent::MISSING_CELL, absent::CELL, undefined.into())
                ),
            ]
        )
    );
    // Failures nested deeper than the chain limit stop growing it.
    let mut nested = numbers.call(sum, []);
    for _ in 0..12 {
        nested = numbered(sum, nested, crate::f64::value(1.0));
    }
    let evaluation = agree(&nested, &host, 100).unwrap();
    assert_eq!(
        absent::chain_length(&evaluation.result),
        absent::CHAIN_LIMIT
    );
    // Of two operands whose chains reach the limit, the longer passes on,
    // and of two as long, the later.
    let chain = |length: usize, tag: &str| {
        (1..length).fold(absent::value(absent::DECLINED), |cause, _| {
            absent::absent(
                absent::INVALID_ARGUMENTS,
                [(left, cause), (right, blob(tag))],
            )
        })
    };
    for (first, second, expected) in [
        (chain(9, "a"), chain(10, "b"), chain(10, "b")),
        (chain(10, "a"), chain(9, "b"), chain(10, "a")),
        (chain(9, "a"), chain(9, "b"), chain(9, "b")),
        (chain(8, "a"), chain(3, "b"), chain(8, "a")),
        (
            chain(7, "a"),
            chain(7, "b"),
            invalid(vec![(left, chain(7, "a")), (right, chain(7, "b"))]),
        ),
    ] {
        let evaluation = agree(&numbered(sum, first, second), &host, 20).unwrap();
        assert_eq!(evaluation.result, expected);
    }
    // A primitive called through a binding to its foreign function, through
    // the foreign function itself, and by reference to its cell, which
    // evaluates its description.
    let f = new_cell_id();
    let operands = [
        (left, crate::f64::value(1.0)),
        (right, crate::f64::value(2.0)),
    ];
    let through_binding = super::call(
        super::lambda([f], super::call(Value::from(f), operands.clone())),
        [(f, crate::foreign_function(sum))],
    );
    let evaluation = agree(&through_binding, &host, 20).unwrap();
    assert_eq!(evaluation.result, crate::f64::value(3.0));
    let through_foreign = super::call(crate::foreign_function(sum), operands);
    assert_eq!(
        agree(&through_foreign, &host, 20).unwrap().result,
        crate::f64::value(3.0)
    );
    let described = agree(&Value::from(sum), &host, 20).unwrap();
    assert_eq!(
        described.result,
        isa::record(
            kind::FOREIGN_FUNCTION,
            [
                (sum, blob("name")),
                (vocabulary::IMPLEMENTATION, Value::from(sum)),
                (
                    vocabulary::PARAMS,
                    Value::list([Value::from(left), Value::from(right)])
                ),
            ]
        )
    );
    // A number operand halts where evaluations nest too deeply, however it's
    // read: here, a literal nested in wrappers to just past the limit.
    deep(move || {
        let numbers = Numbers::new();
        let host = numbers.host();
        let sum = numbers.binary(Named::F64Sum);
        let (left, right) = (numbers.left, numbers.right);
        for wrappers in [MAX_DEPTH - 2, MAX_DEPTH - 1, MAX_DEPTH] {
            let mut program = numbers.call(
                sum,
                [
                    (left, crate::f64::value(1.0)),
                    (right, crate::f64::value(2.0)),
                ],
            );
            for _ in 0..wrappers {
                program = wrapped(program);
            }
            let evaluation = agree(&program, &host, 1000).unwrap();
            assert_eq!(
                evaluation.halt.is_some(),
                wrappers >= MAX_DEPTH - 1,
                "with {wrappers} wrappers"
            );
        }
    });
}

/// Random core programs: lambdas with good and bad parameters and missing
/// fields, calls through lambdas, bindings, cells, closure records and
/// foreign functions, value wrappers, records that only look like forms,
/// data, and definitions that refer to each other and themselves, with
/// budgets small enough to run out and large enough to nest too deeply.
struct Programs {
    state: u64,
    variables: Vec<CellId>,
    defined: Vec<CellId>,
    foreign: Vec<CellId>,
    missing: Vec<CellId>,
    /// The number operations programs call, and operands for them, when
    /// they use numbers.
    numbers: Option<(Numbers, Vec<Value>)>,
    /// Whether definitions mostly call or refer to each other, so programs
    /// that use them recurse.
    recursive: bool,
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
            numbers: None,
            recursive: false,
        }
    }

    /// Programs that also call the number operations, on numbers and on
    /// anything else.
    fn with_numbers(seed: u64) -> Self {
        Self {
            numbers: Some((Numbers::new(), number_operands())),
            ..Self::new(seed)
        }
    }

    /// Programs over definitions that mostly recurse, using numbers too.
    fn recursive(seed: u64) -> Self {
        Self {
            recursive: true,
            ..Self::with_numbers(seed)
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
        if self.numbers.is_some() && self.below(8) == 0 {
            let operations = self.operations();
            return operations[self.below(operations.len())].0;
        }
        match self.below(10) {
            0..=4 => self.pick(&self.variables.clone()),
            5..=7 => self.pick(&self.defined.clone()),
            8 => self.pick(&self.foreign.clone()),
            _ => self.pick(&self.missing.clone()),
        }
    }

    fn leaf(&mut self) -> Value {
        if self.numbers.is_some() && self.below(3) == 0 {
            return self.number();
        }
        match self.below(6) {
            0 => blob(["a", "b", "c"][self.below(3)]),
            1 => Value::record([]),
            2 => Value::list([blob("item")]),
            3 => absent::value(absent::DECLINED),
            _ => Value::from(self.cell()),
        }
    }

    fn expression(&mut self, depth: usize) -> Value {
        if depth == 0 {
            return self.leaf();
        }
        if self.numbers.is_some() && self.below(3) == 0 {
            return self.operation_call(depth);
        }
        match self.below(14) {
            0 | 1 => self.leaf(),
            2 | 3 => self.lambda(depth),
            4..=7 => self.call(depth),
            8 => {
                if self.below(8) == 0 {
                    isa::record(kind::VALUE_WRAPPER, [])
                } else {
                    wrapped(self.expression(depth - 1))
                }
            }
            9 => self.closure_record(depth),
            10 => crate::foreign_function(match self.below(3) {
                0 => self.pick(&self.defined.clone()),
                1 => self.cell(),
                _ => self.pick(&self.foreign.clone()),
            }),
            11 => Value::list([self.expression(depth - 1), self.leaf()]),
            _ => self.lookalike(depth),
        }
    }

    /// A record with a form's fields but not its kind: without `isa`, with
    /// one that isn't a cell, or naming another kind.
    fn lookalike(&mut self, depth: usize) -> Value {
        let fields = [
            (vocabulary::FUNCTION, self.expression(depth - 1)),
            (vocabulary::PARAMS, self.parameters()),
            (vocabulary::BODY, self.expression(depth - 1)),
            (vocabulary::VALUE, self.leaf()),
        ];
        let named = match self.below(4) {
            0 => None,
            1 => Some(blob("call")),
            2 => Some(Value::from(kind::ABSENT)),
            _ => Some(Value::from(self.pick(&self.missing.clone()))),
        };
        Value::record(
            fields
                .into_iter()
                .chain(named.map(|named| (isa::ISA, named))),
        )
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
        let mut fields = vec![(isa::ISA, Value::from(kind::LAMBDA))];
        if self.below(12) != 0 {
            fields.push((vocabulary::PARAMS, self.parameters()));
        }
        if self.below(10) != 0 {
            fields.push((vocabulary::BODY, self.expression(depth - 1)));
        }
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
        if self.below(16) == 0 {
            isa::record(kind::CALL, arguments)
        } else {
            super::call(function, arguments)
        }
    }

    /// Where a body was written, as GID data: legibly, or not.
    fn origin(&mut self) -> Value {
        let position = |byte: u8| Step::Element(Position::from_bytes(vec![0x40, byte]).unwrap());
        let cell = self.pick(&self.missing.clone());
        match self.below(7) {
            0 => source::value(&SourceOrigin::Input(vec![Step::Key(cell)])),
            1 => source::value(&SourceOrigin::Root(vec![position(0x80), Step::Follow])),
            2 => source::value(&SourceOrigin::Cell {
                cell,
                path: vec![Step::Key(vocabulary::BODY), position(0x01)],
            }),
            3 => Value::record([
                (source::vocabulary::INPUT, Value::list([])),
                (source::vocabulary::ROOT, Value::list([])),
            ]),
            4 => Value::record([(
                source::vocabulary::INPUT,
                Value::list([Value::record([(
                    path::vocabulary::ELEMENT,
                    Value::record([(path::vocabulary::INDEXABLE, Value::from(vec![1u8, 0]))]),
                )])]),
            )]),
            5 => Value::record([
                (
                    source::vocabulary::INPUT,
                    Value::list([Value::record([(path::vocabulary::FOLLOW, blob("through"))])]),
                ),
                (cell, blob("more")),
            ]),
            _ => blob("nowhere"),
        }
    }

    fn closure_record(&mut self, depth: usize) -> Value {
        let lambda = match self.below(8) {
            0 => blob("not a lambda"),
            _ => self.lambda(depth),
        };
        let mut fields = vec![
            (isa::ISA, Value::from(kind::CLOSURE)),
            (vocabulary::LAMBDA, lambda),
        ];
        match self.below(8) {
            0 => {}
            1 => fields.push((vocabulary::ENVIRONMENT, blob("not an environment"))),
            _ => fields.push((
                vocabulary::ENVIRONMENT,
                Value::record([(self.pick(&self.variables.clone()), self.leaf())]),
            )),
        }
        if self.below(2) == 0 {
            fields.push((source::vocabulary::BODY_ORIGIN, self.origin()));
        }
        Value::record(fields)
    }

    fn operations(&self) -> Vec<(CellId, reference::Primitive)> {
        let (numbers, _) = self.numbers.as_ref().unwrap();
        numbers
            .operations
            .iter()
            .map(|(cell, operation, _)| (*cell, *operation))
            .collect()
    }

    /// An operand from the spread, or an absent whose chain of causes is
    /// up to a dozen long.
    fn number(&mut self) -> Value {
        let (_, operands) = self.numbers.as_ref().unwrap();
        let count = operands.len();
        if self.below(5) == 0 {
            let length = 1 + self.below(12);
            let label = self.pick(&self.variables.clone());
            (1..length).fold(absent::value(absent::DECLINED), |cause, _| {
                absent::absent(absent::INVALID_ARGUMENTS, [(label, cause)])
            })
        } else {
            let index = self.below(count);
            self.numbers.as_ref().unwrap().1[index].clone()
        }
    }

    /// A call to a number operation, by its cell, by another, or through its
    /// foreign function, with most of its operands and sometimes another. An
    /// operand can be another call even at the last level, so failures can
    /// nest past the chain limit; fewer than one such operand per call on
    /// average keeps the nesting finite.
    fn operation_call(&mut self, depth: usize) -> Value {
        let operations = self.operations();
        let (cell, operation) = operations[self.below(operations.len())];
        let labels = self.numbers.as_ref().unwrap().0.labels(&operation);
        let function = match self.below(8) {
            0 => crate::foreign_function(cell),
            1 => Value::from(self.cell()),
            _ => Value::from(cell),
        };
        let mut arguments = Vec::new();
        for label in labels {
            if self.below(8) != 0 {
                let operand = match self.below(5) {
                    0 | 1 => self.number(),
                    2 => self.operation_call(depth.saturating_sub(1).max(1)),
                    _ => self.expression(depth - 1),
                };
                arguments.push((label, operand));
            }
        }
        if self.below(10) == 0 {
            arguments.push((self.pick(&self.variables.clone()), self.leaf()));
        }
        super::call(function, arguments)
    }

    /// A definition of `cell` that calls itself or another, and so can
    /// recurse until fuel or depth runs out.
    fn looping(&mut self, cell: CellId, depth: usize) -> Value {
        let variable = self.pick(&self.variables.clone());
        let callee = match self.below(2) {
            0 => cell,
            _ => self.pick(&self.defined.clone()),
        };
        let argument = self.expression(depth - 1);
        super::lambda(
            [variable],
            super::call(Value::from(callee), [(variable, argument)]),
        )
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
                    let definition = if self.recursive && self.below(4) != 0 {
                        self.looping(cell, depth)
                    } else {
                        match self.below(10) {
                            0 | 1 => self.lambda(depth),
                            2 => crate::foreign_function(match self.below(2) {
                                0 => cell,
                                _ => self.cell(),
                            }),
                            3 | 4 => self.looping(cell, depth),
                            5 => Value::from(match self.below(2) {
                                0 => cell,
                                _ => self.pick(&self.defined.clone()),
                            }),
                            _ => self.expression(depth),
                        }
                    };
                    Some((
                        cell,
                        match self.below(8) {
                            0 => Defined::Shadowed(definition),
                            _ => Defined::Grap(definition),
                        },
                    ))
                }
            })
            .collect();
        host.extend(
            self.foreign
                .clone()
                .into_iter()
                .map(|cell| (cell, Defined::Foreign(description([(cell, blob("name"))])))),
        );
        if let Some((numbers, _)) = &self.numbers {
            host.extend(numbers.host());
        }
        host
    }

    /// A budget: one small enough to run out, or one large enough for a
    /// program that keeps calling to nest too deeply first.
    fn fuel(&mut self, small: usize, usual: usize) -> usize {
        match self.below(8) {
            0 | 1 => self.below(small),
            2 | 3 => 300 + self.below(5000),
            _ => self.below(usual),
        }
    }
}

/// How many programs compared, and how many came to a value, ran out of
/// fuel, and nested too deeply.
#[derive(Default, Debug)]
struct Tally {
    compared: usize,
    completed: usize,
    fuel: usize,
    depth: usize,
}

impl Tally {
    fn count(&mut self, evaluation: &Agreed) {
        self.compared += 1;
        match evaluation.halt {
            None => self.completed += 1,
            Some(Halt::FuelExhausted) => self.fuel += 1,
            Some(Halt::DepthExhausted) => self.depth += 1,
        }
    }
}

#[test]
fn the_reference_agrees_on_random_core_programs() {
    deep(|| {
        let mut programs = Programs::new(0x5eed_0f_9a5_cafe);
        let mut tally = Tally::default();
        for _ in 0..20000 {
            let host = programs.host();
            let depth = 1 + programs.below(4);
            let expression = programs.expression(depth);
            let fuel = programs.fuel(6, 80);
            if let Some(evaluation) = agree(&expression, &host, fuel) {
                tally.count(&evaluation);
            }
        }
        // Enough of every outcome to mean something.
        assert!(
            tally.compared > 12000
                && tally.completed > 6000
                && tally.fuel > 1000
                && tally.depth > 10,
            "{tally:?}"
        );
    });
}

#[test]
fn the_reference_agrees_on_random_number_programs() {
    deep(|| {
        let mut programs = Programs::with_numbers(0x0dd_ba11_5eed);
        let mut tally = Tally::default();
        let (mut numbers, mut invalid, mut limited) = (0, 0, 0);
        for _ in 0..20000 {
            let host = programs.host();
            let depth = 1 + programs.below(4);
            let expression = programs.expression(depth);
            let fuel = programs.fuel(8, 120);
            if let Some(evaluation) = agree(&expression, &host, fuel) {
                tally.count(&evaluation);
                let result = &evaluation.result;
                numbers += usize::from(
                    crate::f64::read(result).is_some()
                        || crate::f32::read(result).is_some()
                        || crate::u64::read(result).is_some()
                        || crate::logic::read(result).is_some(),
                );
                invalid += usize::from(absent::reason(result) == Some(absent::INVALID_ARGUMENTS));
                limited += usize::from(absent::chain_length(result) >= absent::CHAIN_LIMIT);
            }
        }
        // Enough of every outcome to mean something.
        assert!(
            tally.compared > 12000
                && tally.completed > 6000
                && tally.fuel > 1000
                && tally.depth > 10,
            "{tally:?}"
        );
        assert!(
            numbers > 500 && invalid > 2000 && limited > 300,
            "numbers {numbers}, invalid arguments {invalid}, at the chain limit {limited}"
        );
    });
}

/// Programs that enter definitions calling or referring to each other, with
/// budgets mostly large enough that evaluations nest too deeply first, which
/// can happen anywhere: in an argument, a body, a definition or an operand.
#[test]
fn the_reference_agrees_on_random_recursive_programs() {
    deep(|| {
        let mut programs = Programs::recursive(0xdee9_5eed);
        let mut tally = Tally::default();
        for _ in 0..4000 {
            let host = programs.host();
            let depth = 1 + programs.below(3);
            let defined = programs.pick(&programs.defined.clone());
            let expression = match programs.below(4) {
                0 => Value::from(defined),
                1 => programs.expression(depth),
                _ => {
                    let variable = programs.pick(&programs.variables.clone());
                    let argument = programs.expression(depth);
                    super::call(Value::from(defined), [(variable, argument)])
                }
            };
            let fuel = match programs.below(4) {
                0 => programs.below(300),
                _ => 300 + programs.below(5000),
            };
            if let Some(evaluation) = agree(&expression, &host, fuel) {
                tally.count(&evaluation);
            }
        }
        assert!(
            tally.compared > 2500 && tally.completed > 300 && tally.fuel > 300 && tally.depth > 500,
            "{tally:?}"
        );
    });
}

/// A host for timing that finds definitions as an editor's does, by hash,
/// rather than by the tests' search.
struct MapHost(std::collections::HashMap<CellId, (Value, Option<ForeignFunction<'static>>)>);

impl MapHost {
    fn new(entries: Entries) -> Self {
        let mut map = std::collections::HashMap::new();
        for (cell, value, code) in entries {
            map.entry(cell).or_insert((value, code));
        }
        Self(map)
    }
}

impl Host for MapHost {
    fn definition(&self, cell: CellId) -> Option<Value> {
        self.0.get(&cell).map(|(value, _)| value.clone())
    }

    fn implementation(&self, cell: CellId) -> Option<ForeignFunction<'static>> {
        self.0.get(&cell).and_then(|(_, code)| code.clone())
    }
}

/// Programs in the core and the number operations, for timing the
/// evaluators: name, expression, host and fuel.
fn timing_programs(numbers: &Numbers) -> Vec<(&'static str, Value, Vec<(CellId, Defined)>, usize)> {
    use reference::BinaryOperation as Named;
    let (left, right) = (numbers.left, numbers.right);
    let operations = [
        numbers.binary(Named::F64Sum),
        numbers.binary(Named::F64Multiply),
        numbers.binary(Named::F64Subtract),
    ];
    // A balanced tree of arithmetic over `leaves`, `depth` deep.
    fn tree(
        operations: &[CellId; 3],
        left: CellId,
        right: CellId,
        leaves: &[CellId],
        depth: usize,
        at: &mut usize,
    ) -> Value {
        if depth == 0 {
            *at += 1;
            Value::from(leaves[*at % leaves.len()])
        } else {
            let first = tree(operations, left, right, leaves, depth - 1, at);
            let second = tree(operations, left, right, leaves, depth - 1, at);
            super::call(
                Value::from(operations[depth % 3]),
                [(left, first), (right, second)],
            )
        }
    }
    let number = |n: f64| crate::f64::value(n);
    let mut programs = Vec::new();

    let parameters: Vec<CellId> = (0..4).map(|_| new_cell_id()).collect();
    let body = tree(&operations, left, right, &parameters, 10, &mut 0);
    programs.push((
        "arithmetic: 1,023 operations on four bindings",
        super::call(
            super::lambda(parameters.clone(), body),
            parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| (*parameter, number(index as f64 + 0.5))),
        ),
        numbers.host(),
        1_000_000,
    ));

    let x = new_cell_id();
    let cells: Vec<CellId> = (0..=10).map(|_| new_cell_id()).collect();
    let mut host = numbers.host();
    host.push((
        cells[0],
        Defined::Grap(super::lambda(
            [x],
            numbers.call(
                operations[0],
                [(left, Value::from(x)), (right, number(1.0))],
            ),
        )),
    ));
    for level in 1..cells.len() {
        let inner = super::call(Value::from(cells[level - 1]), [(x, Value::from(x))]);
        host.push((
            cells[level],
            Defined::Grap(super::lambda(
                [x],
                super::call(Value::from(cells[level - 1]), [(x, inner)]),
            )),
        ));
    }
    programs.push((
        "calls: 2,047 closures called through cells",
        super::call(Value::from(cells[10]), [(x, number(0.0))]),
        host,
        1_000_000,
    ));

    let variables: Vec<CellId> = (0..20).map(|_| new_cell_id()).collect();
    let mut program = tree(&operations, left, right, &variables, 7, &mut 0);
    for (index, variable) in variables.iter().enumerate().rev() {
        program = super::call(
            super::lambda([*variable], program),
            [(*variable, number(index as f64))],
        );
    }
    programs.push((
        "environments: 255 operations under 20 frames",
        program,
        numbers.host(),
        1_000_000,
    ));

    let y = new_cell_id();
    let repeated: Vec<CellId> = (0..=6).map(|_| new_cell_id()).collect();
    let mut host = numbers.host();
    host.push((
        repeated[0],
        Defined::Grap(super::lambda(
            [y],
            tree(&operations, left, right, &[y], 8, &mut 0),
        )),
    ));
    for level in 1..repeated.len() {
        let inner = super::call(Value::from(repeated[level - 1]), [(y, Value::from(y))]);
        host.push((
            repeated[level],
            Defined::Grap(super::lambda(
                [y],
                super::call(Value::from(repeated[level - 1]), [(y, inner)]),
            )),
        ));
    }
    programs.push((
        "repeated: 255 operations run 64 times",
        super::call(Value::from(repeated[6]), [(y, number(0.5))]),
        host,
        1_000_000,
    ));

    let n = new_cell_id();
    let looping = new_cell_id();
    let mut host = numbers.host();
    host.push((
        looping,
        Defined::Grap(super::lambda(
            [n],
            super::call(
                Value::from(looping),
                [(
                    n,
                    numbers.call(
                        operations[0],
                        [(left, Value::from(n)), (right, number(1.0))],
                    ),
                )],
            ),
        )),
    ));
    programs.push((
        "recursion: a cell calling itself until it nests too deeply",
        super::call(Value::from(looping), [(n, number(0.0))]),
        host,
        1_000_000,
    ));
    programs
}

/// The median time of `runs`, in microseconds, over batches of `batch`.
fn median_micros(batch: usize, mut run: impl FnMut()) -> f64 {
    for _ in 0..batch {
        run();
    }
    let mut samples: Vec<f64> = (0..15)
        .map(|_| {
            let start = std::time::Instant::now();
            for _ in 0..batch {
                run();
            }
            start.elapsed().as_secs_f64() * 1e6 / batch as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// How fast each evaluator runs the timing programs. Run with
/// `./tools/linux-cargo test --profile perf -p grap --lib evaluator_speed --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn evaluator_speed() {
    let run = || {
        let numbers = Numbers::new();
        println!(
            "{:<60} {:>10} {:>10} {:>10}",
            "program", "grap µs", "reference", "fast"
        );
        for (name, expression, host, fuel) in timing_programs(&numbers) {
            let entries = entries(&host);
            let reference_host = reference_host(&host, &entries);
            let grap_host = MapHost::new(entries);
            let reference_expression = to_reference(&expression);
            let expected = agree(&expression, &host, fuel).unwrap();
            let grap = median_micros(20, || {
                std::hint::black_box(Evaluator::new(&grap_host, fuel).evaluate(&expression));
            });
            let reference = median_micros(5, || {
                std::hint::black_box(reference::evaluate(
                    &reference_expression,
                    &reference_host,
                    fuel,
                ));
            });
            let fast = median_micros(20, || {
                std::hint::black_box(reference::fast::evaluate(
                    &reference_expression,
                    &reference_host,
                    fuel,
                ));
            });
            println!(
                "{name:<60} {grap:>10.1} {reference:>10.1} {fast:>10.1}   (fuel left {}, halt {:?})",
                expected.remaining_fuel, expected.halt
            );
        }
    };
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
