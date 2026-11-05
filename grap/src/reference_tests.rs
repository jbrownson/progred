//! Grap's evaluator against `grap-reference`, the definition of Grap's core:
//! each program here must come to the same result, fuel left and completion
//! under both, unless it calls a foreign function the definition leaves out.
//! The number operations are registered as the numbers library registers
//! them. Results are compared without the source origins closures record for
//! click-to-source, which the core doesn't define yet.

use super::*;
use gid::new_cell_id;
use grap_reference::{self as reference, ExecDefinition, ExecValue};
use primitive::{Binary, Unary};

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

/// A host definition: Grap's own, a foreign function's description, or a
/// number operation, with its description, as the definition names it and as
/// Grap runs it.
enum Defined {
    Grap(Value),
    Foreign(Value),
    Primitive(Value, reference::Primitive, primitive::Primitive),
}

impl Defined {
    fn grap(&self) -> Definition {
        match self {
            Self::Grap(value) => Definition::Value(value.clone()),
            Self::Foreign(value) => Definition::Foreign(Rc::new(ForeignDefinition {
                value: value.clone(),
                implementation: ForeignFunction::new(|_, _, _| {
                    Ok(RuntimeValue::from(Value::record([])))
                }),
            })),
            Self::Primitive(description, _, primitive) => {
                Definition::foreign(description.clone(), primitive.function())
            }
        }
    }

    /// The reference's definition, describing itself as Grap's `definition`
    /// does, with the parameters a function declares.
    fn reference(&self, definition: &Definition) -> ExecDefinition {
        let value = to_reference(definition.value());
        match self {
            Self::Grap(_) => ExecDefinition::Grap(value),
            Self::Foreign(_) => ExecDefinition::Foreign(value),
            Self::Primitive(_, primitive, _) => ExecDefinition::Primitive(value, *primitive),
        }
    }
}

/// Evaluates under both and checks they agree; `None` when the program
/// calls a foreign function, so the definition says nothing about it.
fn agree(expression: &Value, host: &[(CellId, Defined)], fuel: usize) -> Option<Evaluation<Value>> {
    let definitions: Vec<_> = host
        .iter()
        .map(|(cell, defined)| (*cell, defined.grap()))
        .collect();
    let reference_host: Vec<_> = host
        .iter()
        .zip(&definitions)
        .map(|((cell, defined), (_, definition))| {
            (reference_cell(*cell), defined.reference(definition))
        })
        .collect();
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
                (fast.remaining_fuel, fast.completed),
                (expected.remaining_fuel, expected.completed),
                "fast fuel and completion of {expression:?} with fuel {fuel}"
            );
        }
        (None, None) => {}
        _ => panic!(
            "only one of the reference and the fast evaluator left the definition for {expression:?}"
        ),
    }
    let expected = expected?;
    let resolver = TestHost(|cell| {
        definitions
            .iter()
            .filter(|(defined_cell, _)| *defined_cell == cell)
            .map(|(_, definition)| (Resolution::Document, definition.clone()))
            .collect()
    });
    let actual = evaluate_value(expression, &resolver, fuel);
    assert_eq!(
        canonical_nans(&without_origins(&actual.result)),
        canonical_nans(&from_reference(&expected.result)),
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
                    Defined::Primitive(Value::record([(*cell, blob("name"))]), *named, *operation),
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
            Value::record([
                (absent::ABSENT, Value::from(absent::DECLINED)),
                (crate::f64::F64, Value::from(4.0f64.to_le_bytes().to_vec())),
                (crate::f32::F32, Value::from(4.0f32.to_le_bytes().to_vec())),
                (crate::u64::U64, Value::from(4u64.to_le_bytes().to_vec())),
            ]),
            absent::value(absent::DECLINED),
        ])
        .collect()
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
        (reference::INVALID_ARGUMENTS, absent::INVALID_ARGUMENTS),
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
    // operation's type, or already absent, passed on as it is.
    let missing =
        |label: CellId| absent::with_detail(absent::MISSING_ARGUMENT, absent::CELL, label.into());
    let invalid =
        |details: Vec<(CellId, Value)>| absent::caused_by_all(absent::INVALID_ARGUMENTS, details);
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
        Value::record([
            (absent::ABSENT, Value::from(absent::INVALID_ARGUMENTS)),
            (
                left,
                absent::with_detail(absent::MISSING_CELL, absent::CELL, undefined.into())
            ),
        ])
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
            Value::record([
                (absent::ABSENT, Value::from(absent::INVALID_ARGUMENTS)),
                (left, cause),
                (right, blob(tag)),
            ])
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
    // A primitive called through a binding to `{ffi: cell}`, through
    // `{ffi: cell}` itself, and by reference to its cell, which evaluates its
    // description.
    let f = new_cell_id();
    let operands = [
        (left, crate::f64::value(1.0)),
        (right, crate::f64::value(2.0)),
    ];
    let through_binding = super::call(
        super::lambda([f], super::call(Value::from(f), operands.clone())),
        [(f, super::ffi(sum))],
    );
    let evaluation = agree(&through_binding, &host, 20).unwrap();
    assert_eq!(evaluation.result, crate::f64::value(3.0));
    let through_ffi = super::call(super::ffi(sum), operands);
    assert_eq!(
        agree(&through_ffi, &host, 20).unwrap().result,
        crate::f64::value(3.0)
    );
    let described = agree(&Value::from(sum), &host, 20).unwrap();
    assert_eq!(
        described.result,
        Value::record([
            (sum, blob("name")),
            (
                vocabulary::PARAMS,
                Value::list([Value::from(left), Value::from(right)])
            ),
        ])
    );
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
    /// The number operations programs call, and operands for them, when
    /// they use numbers.
    numbers: Option<(Numbers, Vec<Value>)>,
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
        if self.numbers.is_some() && self.below(3) == 0 {
            return self.operation_call(depth);
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
                Value::record([
                    (absent::ABSENT, Value::from(absent::INVALID_ARGUMENTS)),
                    (label, cause),
                ])
            })
        } else {
            let index = self.below(count);
            self.numbers.as_ref().unwrap().1[index].clone()
        }
    }

    /// A call to a number operation, by its cell, by another, or through
    /// `{ffi: cell}`, with most of its operands and sometimes another. An
    /// operand can be another call even at the last level, so failures can
    /// nest past the chain limit; fewer than one such operand per call on
    /// average keeps the nesting finite.
    fn operation_call(&mut self, depth: usize) -> Value {
        let operations = self.operations();
        let (cell, operation) = operations[self.below(operations.len())];
        let labels = self.numbers.as_ref().unwrap().0.labels(&operation);
        let function = match self.below(8) {
            0 => super::ffi(cell),
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
        if let Some((numbers, _)) = &self.numbers {
            host.extend(numbers.host());
        }
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

#[test]
fn the_reference_agrees_on_random_number_programs() {
    let mut programs = Programs::with_numbers(0x0dd_ba11_5eed);
    let (mut compared, mut completed) = (0, 0);
    let (mut numbers, mut invalid, mut limited) = (0, 0, 0);
    for _ in 0..20000 {
        let host = programs.host();
        let depth = 1 + programs.below(4);
        let expression = programs.expression(depth);
        let fuel = match programs.below(4) {
            0 => programs.below(8),
            _ => programs.below(120),
        };
        if let Some(evaluation) = agree(&expression, &host, fuel) {
            compared += 1;
            completed += usize::from(evaluation.completed);
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
    // Enough of each outcome to mean something.
    assert!(compared > 12000, "compared {compared}");
    assert!(
        completed > 6000 && compared - completed > 1000,
        "completed {completed}"
    );
    assert!(
        numbers > 500 && invalid > 2000 && limited > 300,
        "numbers {numbers}, invalid arguments {invalid}, at the chain limit {limited}"
    );
}

/// A host for timing that finds definitions as an editor's does, by hash,
/// rather than by the tests' search.
struct MapHost(std::collections::HashMap<CellId, Definition>);

impl Host for MapHost {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)> {
        self.0
            .get(&cell)
            .map(|definition| (Resolution::Document, definition.clone()))
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
        "recursion: a cell calling itself until 3,000 fuel runs out",
        super::call(Value::from(looping), [(n, number(0.0))]),
        host,
        3_000,
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
            let definitions: Vec<_> = host
                .iter()
                .map(|(cell, defined)| (*cell, defined.grap()))
                .collect();
            let reference_host: Vec<_> = host
                .iter()
                .zip(&definitions)
                .map(|((cell, defined), (_, definition))| {
                    (reference_cell(*cell), defined.reference(definition))
                })
                .collect();
            let grap_host = MapHost(definitions.into_iter().collect());
            let reference_expression = to_reference(&expression);
            let expected = agree(&expression, &host, fuel).unwrap();
            let grap = median_micros(20, || {
                std::hint::black_box(evaluate_value(&expression, &grap_host, fuel));
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
                "{name:<60} {grap:>10.1} {reference:>10.1} {fast:>10.1}   (fuel left {}, completed {})",
                expected.remaining_fuel, expected.completed
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
