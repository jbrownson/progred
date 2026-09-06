//! An f64 Grap library. The number representation is ordinary Grap
//! data; arithmetic is supplied to the evaluator as Rust foreign
//! functions.

use crate::{Library, absent, line_edit, logic, name, number};
use gid::{CellId, Cells, Step, Value};

pub const ID: CellId = CellId::from_u128(0x1fdb573a2c56a7063546c195318214bc);
#[cfg(test)]
use grap_runtime as grap;
use grap_runtime::vocabulary::FUNCTION;
use grap_runtime::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use progred_display::{Delim, Layout, ProjectionInput, bracket, overlay_value, row};

pub mod vocabulary {
    use gid::CellId;

    pub use crate::number::vocabulary::{LEFT, OPERAND, RIGHT};
    pub use grap_runtime::f64::F64;
    pub const SUM: CellId = CellId::from_u128(0x201af445eb7e2c270bb5ead10b781fc1);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd6f384c439d9d69996d545df422efd79);
    pub const SUBTRACT: CellId = CellId::from_u128(0x08d1ebc7fd4ce62efec9671f73e9b645);
    pub const DIVIDE: CellId = CellId::from_u128(0xb08dd4c44eeea43ab3c7593ddeedd742);
    pub const SIN: CellId = CellId::from_u128(0x9f62e0f56d92be74f6ae36378d889262);
    pub const COS: CellId = CellId::from_u128(0xfb05b6c9b1565ca4add3732e8ddbe2b4);
    pub const LESS: CellId = CellId::from_u128(0xed44dbf5b4cdf5c952e1ef00f219b655);
    pub const EQUAL: CellId = CellId::from_u128(0x22ab9aa3e7ce4f4f79a7039e1cc23773);
    pub const FLOOR: CellId = CellId::from_u128(0xd007814c5f6a6c38b025605b399473d4);
    pub const LERP: CellId = CellId::from_u128(0x432ad7a31ef129e353e251419e690ca1);
    pub const PI: CellId = CellId::from_u128(0x9cd591f37312e563f52b7374a6cef5c0);
    pub const START: CellId = CellId::from_u128(0x4b4fb6349d2fd798e7aafca85a2deca8);
    pub const END: CellId = CellId::from_u128(0x3a816b0af0160bc77ba1948b122b3f29);
    pub const AMOUNT: CellId = CellId::from_u128(0x55e66fc5eb91699cf12833fb0a15d4b6);
    /// The f64 line's write-back rule: parse the typed spelling,
    /// other fields carried; unparseable input declines.
    pub const UPDATE: CellId = CellId::from_u128(0x6b95d2e04c7a1f38b1a08e57d24c96fb);
    pub const LEFT_NOT_F64: CellId = CellId::from_u128(0x50c0d2fd8fe0325a8e0e41f79ce86eff);
    pub const RIGHT_NOT_F64: CellId = CellId::from_u128(0xcab77cffe8c38745dd8e748ece331409);
    pub const OPERAND_NOT_F64: CellId = CellId::from_u128(0x9c2a4845e1c67df6dd43ac1116e76441);
    pub const START_NOT_F64: CellId = CellId::from_u128(0x809d7bba33afaf8a673846f0c44cd2e6);
    pub const END_NOT_F64: CellId = CellId::from_u128(0x17081dd43c4af46c54408cd125eaf3e2);
    pub const AMOUNT_NOT_F64: CellId = CellId::from_u128(0xe1977104f3574cd37a99ef9083dde01f);
}

// The convention itself lives in the evaluator, which privileges it
// as an accelerator; this library remains its owner in vocabulary,
// functions, and projections.
pub use grap_runtime::f64::{read, value};

impl number::Scrubbable for f64 {
    fn magnitude(self) -> f64 {
        self
    }

    fn minimum_precision() -> f64 {
        0.0
    }

    fn scrubbable(self) -> bool {
        self.is_finite()
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        number::rounded(start + offset, precision)
    }

    fn spelling(self, precision: f64) -> String {
        if precision >= 1.0 {
            self.round().to_string()
        } else {
            let decimal_places = (-precision.log10()).round().clamp(0.0, 16.0) as usize;
            format!("{self:.decimal_places$}")
        }
    }
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let number = read(input.value)?;
    Some(number::layout(
        input,
        number,
        "f64",
        vocabulary::UPDATE,
        value,
    ))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Comparison,
    Sum,
    Product,
}

fn precedence(function: CellId) -> Option<Precedence> {
    match function {
        vocabulary::SUM | vocabulary::SUBTRACT => Some(Precedence::Sum),
        vocabulary::MULTIPLY | vocabulary::DIVIDE => Some(Precedence::Product),
        vocabulary::LESS | vocabulary::EQUAL => Some(Precedence::Comparison),
        _ => None,
    }
}

fn expression_precedence(value: &Value) -> Option<Precedence> {
    let fields = value.as_record()?;
    (fields.len() == 3).then_some(())?;
    fields.get(&vocabulary::LEFT)?;
    fields.get(&vocabulary::RIGHT)?;
    fields.get(&FUNCTION)?.as_cell().and_then(precedence)
}

fn operand<World: 'static, Hover: Clone + 'static>(
    field: CellId,
    value: &Value,
    parent: Precedence,
) -> Layout<World, Hover> {
    let child = crate::grap::expression_descend(Step::Key(field));
    match expression_precedence(value) {
        Some(child_precedence)
            if child_precedence < parent
                || (child_precedence == parent
                    && (field == vocabulary::RIGHT || parent == Precedence::Comparison)) =>
        {
            bracket(Delim::Paren, child)
        }
        _ => child,
    }
}

pub fn binary_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&FUNCTION)?;
    let precedence = precedence(function.as_cell()?)?;
    let left = fields.get(&vocabulary::LEFT)?;
    let right = fields.get(&vocabulary::RIGHT)?;
    (fields.len() == 3).then_some(row(
        6.0,
        [
            operand(vocabulary::LEFT, left, precedence),
            crate::grap::shallow_descend(Step::Key(FUNCTION)),
            operand(vocabulary::RIGHT, right, precedence),
        ],
    ))
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::UPDATE,
            ForeignFunction::new(|context, call, environment| {
                let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
                    return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
                };
                let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                    return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
                };
                let current = context.eval(current, environment)?;
                let input = context.eval(input, environment)?;
                Ok(crate::text::read(&input)
                    .and_then(|text| text.trim().parse::<f64>().ok())
                    .map(|number| overlay_value(&current, value(number)))
                    .unwrap_or_else(crate::absent::value))
            }),
        )
        .register(
            vocabulary::SUM,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left + right)
            }),
        )
        .register(
            vocabulary::MULTIPLY,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left * right)
            }),
        )
        .register(
            vocabulary::SUBTRACT,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left - right)
            }),
        )
        .register(
            vocabulary::DIVIDE,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left / right)
            }),
        )
        .register(
            vocabulary::SIN,
            ForeignFunction::runtime(|context, call, environment| {
                unary(context, call, environment, f64::sin)
            }),
        )
        .register(
            vocabulary::COS,
            ForeignFunction::runtime(|context, call, environment| {
                unary(context, call, environment, f64::cos)
            }),
        )
        .register(
            vocabulary::FLOOR,
            ForeignFunction::runtime(|context, call, environment| {
                unary(context, call, environment, f64::floor)
            }),
        )
        .register(vocabulary::LERP, ForeignFunction::runtime(lerp))
        .register(
            vocabulary::LESS,
            ForeignFunction::runtime(|context, call, environment| {
                binary_value(context, call, environment, |left, right| {
                    RuntimeValue::from_value(logic::value(left < right))
                })
            }),
        )
        .register(
            vocabulary::EQUAL,
            ForeignFunction::runtime(|context, call, environment| {
                binary_value(context, call, environment, |left, right| {
                    RuntimeValue::from_value(logic::value(left == right))
                })
            }),
        )
}

fn lerp(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(start) = context.field(call, vocabulary::START) else {
        return Ok(context.missing_runtime_argument(vocabulary::START));
    };
    let Some(end) = context.field(call, vocabulary::END) else {
        return Ok(context.missing_runtime_argument(vocabulary::END));
    };
    let Some(amount) = context.field(call, vocabulary::AMOUNT) else {
        return Ok(context.missing_runtime_argument(vocabulary::AMOUNT));
    };
    let start = context.eval_f64(start, environment)?;
    let end = context.eval_f64(end, environment)?;
    let amount = context.eval_f64(amount, environment)?;
    Ok(match (start, end, amount) {
        (Some(start), Some(end), Some(amount)) => RuntimeValue::f64(start + (end - start) * amount),
        (None, _, _) => absent::with_reason(vocabulary::START_NOT_F64).into(),
        (_, None, _) => absent::with_reason(vocabulary::END_NOT_F64).into(),
        (_, _, None) => absent::with_reason(vocabulary::AMOUNT_NOT_F64).into(),
    })
}

fn binary(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<RuntimeValue, Halt> {
    binary_value(context, call, environment, |left, right| {
        RuntimeValue::f64(operation(left, right))
    })
}

fn binary_value(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f64, f64) -> RuntimeValue,
) -> Result<RuntimeValue, Halt> {
    let Some(left) = context.field(call, vocabulary::LEFT) else {
        return Ok(context.missing_runtime_argument(vocabulary::LEFT));
    };
    let Some(right) = context.field(call, vocabulary::RIGHT) else {
        return Ok(context.missing_runtime_argument(vocabulary::RIGHT));
    };
    let left = context.eval_f64(left, environment)?;
    let right = context.eval_f64(right, environment)?;
    Ok(match (left, right) {
        (Some(left), Some(right)) => operation(left, right),
        (None, _) => absent::with_reason(vocabulary::LEFT_NOT_F64).into(),
        (_, None) => absent::with_reason(vocabulary::RIGHT_NOT_F64).into(),
    })
}

fn unary(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f64) -> f64,
) -> Result<RuntimeValue, Halt> {
    let Some(operand) = context.field(call, vocabulary::OPERAND) else {
        return Ok(context.missing_runtime_argument(vocabulary::OPERAND));
    };
    Ok(context
        .eval_f64(operand, environment)?
        .map(|operand| RuntimeValue::f64(operation(operand)))
        .unwrap_or_else(|| absent::with_reason(vocabulary::OPERAND_NOT_F64).into()))
}

pub fn completions(query: &str) -> Vec<progred_display::Completion> {
    number::completions(query, "f64", value)
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::F64, "f64"),
        (vocabulary::UPDATE, "f64 update"),
        (vocabulary::SUM, "+"),
        (vocabulary::MULTIPLY, "*"),
        (vocabulary::SUBTRACT, "-"),
        (vocabulary::DIVIDE, "/"),
        (vocabulary::SIN, "sin"),
        (vocabulary::COS, "cos"),
        (vocabulary::LESS, "<"),
        (vocabulary::EQUAL, "=="),
        (vocabulary::FLOOR, "floor"),
        (vocabulary::LERP, "lerp"),
        (vocabulary::START, "start"),
        (vocabulary::END, "end"),
        (vocabulary::AMOUNT, "amount"),
    ] {
        cells.set_value(cell, name::record(name, []));
    }
    for (cell, name) in [
        (vocabulary::LEFT_NOT_F64, "left is not f64"),
        (vocabulary::RIGHT_NOT_F64, "right is not f64"),
        (vocabulary::OPERAND_NOT_F64, "operand is not f64"),
        (vocabulary::START_NOT_F64, "start is not f64"),
        (vocabulary::END_NOT_F64, "end is not f64"),
        (vocabulary::AMOUNT_NOT_F64, "amount is not f64"),
    ] {
        cells.set_value(cell, absent::named_reason(name));
    }
    cells.set_value(
        vocabulary::PI,
        overlay_value(&value(std::f64::consts::PI), name::record("π", [])),
    );
    Library::named(
        ID,
        "f64",
        crate::Definitions::from_parts(cells, functions()),
        progred_display::compose_partials([
            progred_display::partial(binary_display::<World, Hover>),
            progred_display::partial(display::<World, Hover>),
        ]),
    )
    .with_completions(|request| {
        (request.scope == progred_display::CompletionScope::Everything
            && request.kind == progred_display::CompletionKind::Value)
            .then(|| completions(request.query))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    struct TestEnv;

    impl progred_display::Env for TestEnv {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            unreachable!()
        }
    }

    fn target(_: Vec<Step>) -> progred_display::ProjectionTarget<(), ()> {
        progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: (),
        }
    }

    fn projection_input(value: &Value) -> ProjectionInput<'_, (), ()> {
        ProjectionInput {
            env: &TestEnv,
            value,
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&target),
        }
    }

    fn call(function: CellId, left: Value, right: Value) -> Value {
        grap::call(
            Value::from(function),
            [(vocabulary::LEFT, left), (vocabulary::RIGHT, right)],
        )
    }

    #[test]
    fn representation_is_library_data() {
        assert_eq!(read(&value(2.5)), Some(2.5));
        assert_eq!(read(&Value::from(b"2.5".to_vec())), None);

        let extra = new_cell_id();
        let with_extra = Value::record(
            value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"degrees".to_vec())),
        );
        assert_eq!(read(&with_extra), Some(2.5));
        let update = |input: &str| {
            crate::test_evaluate(
                &grap::call(
                    grap::ffi(vocabulary::UPDATE),
                    [
                        (line_edit::vocabulary::CURRENT, with_extra.clone()),
                        (line_edit::vocabulary::INPUT, crate::text::value(input)),
                    ],
                ),
                |_| None,
                &functions(),
                100,
            )
            .result
        };
        // Unparseable input returns an absent — the line editor drops
        // the write whole.
        assert!(crate::absent::is_absent(&update("junk")));
        assert_eq!(
            update("3"),
            Value::record(
                value(3.0)
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(b"degrees".to_vec())),
            )
        );
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let sum = call(vocabulary::SUM, value(2.0), value(3.0));
        let multiply = call(vocabulary::MULTIPLY, sum, value(4.0));
        assert_eq!(
            crate::test_evaluate(&multiply, |_| None, &functions(), 20).result,
            value(20.0)
        );
    }

    #[test]
    fn rust_supplies_lerp_to_grap() {
        let expression = grap::call(
            Value::from(vocabulary::LERP),
            [
                (vocabulary::START, value(10.0)),
                (vocabulary::END, value(20.0)),
                (vocabulary::AMOUNT, value(0.25)),
            ],
        );
        assert_eq!(
            crate::test_evaluate(&expression, |_| None, &functions(), 20).result,
            value(12.5)
        );
    }

    #[test]
    fn runtime_lowering_preserves_an_enriched_value_through_a_binding() {
        let parameter = new_cell_id();
        let metadata = new_cell_id();
        let enriched = Value::Record(
            value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(metadata, Value::from(b"kept".to_vec())),
        );
        let expression = grap::call(
            grap::lambda([parameter], Value::from(parameter)),
            [(parameter, enriched.clone())],
        );

        assert_eq!(
            crate::test_evaluate(&expression, |_| None, &functions(), 20).result,
            enriched
        );
    }

    #[test]
    fn binary_notation_descends_through_source_fields_and_preserves_precedence() {
        let product = call(vocabulary::MULTIPLY, value(2.0), value(3.0));
        let sum = call(vocabulary::SUM, value(1.0), product);
        let layout = binary_display(&projection_input(&sum)).unwrap();
        let Layout::Row { children, .. } = layout else {
            panic!("binary notation is a row");
        };
        assert!(matches!(
            &children[0],
            Layout::Descend {
                step: Step::Key(field),
                ..
            } if *field == vocabulary::LEFT
        ));
        assert!(matches!(
            &children[1],
            Layout::Descend {
                step: Step::Key(field),
                projection: Some(projection),
                ..
            } if *field == FUNCTION && projection.len() == 1
        ));
        assert!(matches!(
            &children[2],
            Layout::Descend {
                step: Step::Key(field),
                ..
            } if *field == vocabulary::RIGHT
        ));

        let product = call(
            vocabulary::MULTIPLY,
            call(vocabulary::SUM, value(1.0), value(2.0)),
            value(3.0),
        );
        let layout = binary_display(&projection_input(&product)).unwrap();
        let Layout::Row { children, .. } = layout else {
            panic!("binary notation is a row");
        };
        assert!(matches!(&children[0], Layout::Surround { .. }));
    }

    #[test]
    fn binary_notation_declines_calls_with_unshown_fields() {
        let extra = new_cell_id();
        let call = grap::call(
            Value::from(vocabulary::SUM),
            [
                (vocabulary::LEFT, value(1.0)),
                (vocabulary::RIGHT, value(2.0)),
                (extra, value(3.0)),
            ],
        );
        assert!(binary_display(&projection_input(&call)).is_none());
    }

    #[test]
    fn type_absences_are_library_values() {
        let left = call(vocabulary::SUM, Value::from(b"two".to_vec()), value(3.0));
        let right = call(vocabulary::SUM, value(2.0), Value::from(b"three".to_vec()));
        assert_eq!(
            crate::test_evaluate(&left, |_| None, &functions(), 10).result,
            absent::with_reason(vocabulary::LEFT_NOT_F64)
        );
        assert_eq!(
            crate::test_evaluate(&right, |_| None, &functions(), 10).result,
            absent::with_reason(vocabulary::RIGHT_NOT_F64)
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library::<(), ()>();
        assert_eq!(
            library.value(vocabulary::F64).and_then(name::read),
            Some("f64")
        );
        assert_eq!(
            library.value(vocabulary::SUM).and_then(name::read),
            Some("+")
        );
        assert_eq!(
            library.value(vocabulary::PI).and_then(name::read),
            Some("π")
        );
        assert_eq!(
            library.value(vocabulary::PI).and_then(read),
            Some(std::f64::consts::PI)
        );
        assert_eq!(
            library.value(vocabulary::LEFT_NOT_F64).and_then(name::read),
            Some("left is not f64")
        );
        assert_eq!(
            library
                .value(vocabulary::RIGHT_NOT_F64)
                .and_then(name::read),
            Some("right is not f64")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::LEFT_NOT_F64)),
            Some(vocabulary::LEFT_NOT_F64)
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::RIGHT_NOT_F64)),
            Some(vocabulary::RIGHT_NOT_F64)
        );
        assert!(library.value(vocabulary::SUM).is_some());
    }
}
