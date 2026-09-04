//! An open u64 record convention with the stock editable projection.

use crate::{Library, absent, line_edit, logic, name, number};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0xb7212cd0aed055a7a2fbe4036b7f3e51);
use grap_runtime::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{Layout, ProjectionInput, overlay_value};

pub mod vocabulary {
    use gid::CellId;

    pub const U64: CellId = CellId::from_u128(0xfc77282178c2fa31b80aed2b8b6f7888);
    pub const UPDATE: CellId = CellId::from_u128(0xd02b945cdf6210536bdf4d0fcc895f05);
    pub const SUM: CellId = CellId::from_u128(0x2b63a57fb5065d7ae992241b3018fcb5);
    pub const SUBTRACT: CellId = CellId::from_u128(0x74203a2808e4216a6eff871dd39ab313);
    pub const MULTIPLY: CellId = CellId::from_u128(0xeb9073084d92e0ecb2253ff70a27c03e);
    pub const DIVIDE: CellId = CellId::from_u128(0x610171b11dd2d2844119b7f9bc7da17c);
    pub const LESS: CellId = CellId::from_u128(0x425af3c983c925e2eb8becf4389aac58);
    pub const EQUAL: CellId = CellId::from_u128(0xc3931b2321d6c783fee133acaa929738);
    pub const LEFT_NOT_U64: CellId = CellId::from_u128(0x64dd17fd81404f413122ffc00b452e2e);
    pub const RIGHT_NOT_U64: CellId = CellId::from_u128(0x1a13161cd3a0e5fa71455a277839bd32);
    pub const OVERFLOW: CellId = CellId::from_u128(0x184f82a04d9cb7609238b2dd05ad5cd0);
    pub const DIVISION_BY_ZERO: CellId = CellId::from_u128(0x7eef118f55e12f09c8f7324c0fdd765d);
}

pub fn value(value: u64) -> Value {
    Value::record([(vocabulary::U64, Value::from(value.to_le_bytes().to_vec()))])
}

pub fn read(value: &Value) -> Option<u64> {
    value
        .as_record()?
        .get(&vocabulary::U64)
        .and_then(Value::as_blob)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes)
}

impl number::Scrubbable for u64 {
    fn magnitude(self) -> f64 {
        self as f64
    }

    fn minimum_precision() -> f64 {
        1.0
    }

    fn scrubbable(self) -> bool {
        true
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        let step = (precision.round() as u64).max(1);
        let lower = start / step * step;
        let steps = ((start % step) as f64 + offset) / step as f64;
        (lower as i128)
            .saturating_add((steps.round() as i128).saturating_mul(step as i128))
            .clamp(0, u64::MAX as i128) as u64
    }

    fn spelling(self, _: f64) -> String {
        self.to_string()
    }
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(number::layout(
        input,
        read(input.value)?,
        vocabulary::UPDATE,
        value,
    ))
}

fn update(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
        return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
    };
    let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
        return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
    };
    let current = context.eval(current, environment)?;
    let input = context.eval(input, environment)?;
    Ok(crate::text::read(&input)
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|number| overlay_value(&current, value(number)))
        .unwrap_or_else(absent::value))
}

fn binary(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(u64, u64) -> Value,
) -> Result<Value, Halt> {
    let Some(left) = context.field(call, number::vocabulary::LEFT) else {
        return Ok(context.missing_argument(number::vocabulary::LEFT));
    };
    let Some(right) = context.field(call, number::vocabulary::RIGHT) else {
        return Ok(context.missing_argument(number::vocabulary::RIGHT));
    };
    let left = read(&context.eval(left, environment)?);
    let right = read(&context.eval(right, environment)?);
    Ok(match (left, right) {
        (Some(left), Some(right)) => operation(left, right),
        (None, _) => absent::with_reason(vocabulary::LEFT_NOT_U64),
        (_, None) => absent::with_reason(vocabulary::RIGHT_NOT_U64),
    })
}

/// Unsigned arithmetic has no representable overflow or division by
/// zero, so those outcomes are absents rather than wrapped bits.
fn checked(operation: fn(u64, u64) -> Option<u64>, failure: CellId) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        binary(context, call, environment, |left, right| {
            operation(left, right)
                .map(value)
                .unwrap_or_else(|| absent::with_reason(failure))
        })
    })
}

fn comparison(operation: fn(u64, u64) -> bool) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        binary(context, call, environment, |left, right| {
            logic::value(operation(left, right))
        })
    })
}

fn functions() -> ForeignFunctions {
    [
        (
            vocabulary::SUM,
            checked(u64::checked_add, vocabulary::OVERFLOW),
        ),
        (
            vocabulary::SUBTRACT,
            checked(u64::checked_sub, vocabulary::OVERFLOW),
        ),
        (
            vocabulary::MULTIPLY,
            checked(u64::checked_mul, vocabulary::OVERFLOW),
        ),
        (
            vocabulary::DIVIDE,
            checked(u64::checked_div, vocabulary::DIVISION_BY_ZERO),
        ),
        (vocabulary::LESS, comparison(|left, right| left < right)),
        (vocabulary::EQUAL, comparison(|left, right| left == right)),
    ]
    .into_iter()
    .fold(
        ForeignFunctions::default().register(vocabulary::UPDATE, ForeignFunction::new(update)),
        |functions, (cell, function)| functions.register(cell, function),
    )
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::U64, "u64"),
        (vocabulary::UPDATE, "u64 update"),
        (vocabulary::SUM, "+"),
        (vocabulary::SUBTRACT, "-"),
        (vocabulary::MULTIPLY, "*"),
        (vocabulary::DIVIDE, "/"),
        (vocabulary::LESS, "<"),
        (vocabulary::EQUAL, "=="),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, reason) in [
        (vocabulary::LEFT_NOT_U64, "left is not u64"),
        (vocabulary::RIGHT_NOT_U64, "right is not u64"),
        (vocabulary::OVERFLOW, "u64 overflow"),
        (vocabulary::DIVISION_BY_ZERO, "division by zero"),
    ] {
        cells.set_value(cell, absent::named_reason(reason));
    }
    Library::named(
        "u64",
        crate::Definitions::from_parts(cells, functions()),
        vec![progred_display::partial(display::<World, Hover>)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use grap_runtime as grap;

    fn call(function: CellId, left: Value, right: Value) -> Value {
        grap::call(
            Value::from(function),
            [
                (number::vocabulary::LEFT, left),
                (number::vocabulary::RIGHT, right),
            ],
        )
    }

    fn evaluate(expression: &Value) -> Value {
        crate::test_evaluate(expression, |_| None, &functions(), 20).result
    }

    #[test]
    fn representation_is_open_library_data() {
        let extra = new_cell_id();
        let current = Value::record(
            value(u64::MAX)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"metadata".to_vec())),
        );
        let update = |input: &str| {
            crate::test_evaluate(
                &grap::call(
                    grap::ffi(vocabulary::UPDATE),
                    [
                        (line_edit::vocabulary::CURRENT, current.clone()),
                        (line_edit::vocabulary::INPUT, crate::text::value(input)),
                    ],
                ),
                |_| None,
                &functions(),
                100,
            )
            .result
        };

        assert_eq!(read(&current), Some(u64::MAX));
        assert_eq!(
            update("42"),
            Value::record(
                value(42)
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(b"metadata".to_vec())),
            )
        );
        assert!(crate::absent::is_absent(&update("-1")));
    }

    #[test]
    fn scrubbing_is_exact_at_the_unsigned_bounds() {
        assert_eq!(<u64 as number::Scrubbable>::from_offset(0, -100.0, 1.0), 0,);
        assert_eq!(
            <u64 as number::Scrubbable>::from_offset(u64::MAX, 100.0, 1.0),
            u64::MAX,
        );
        assert_eq!(
            <u64 as number::Scrubbable>::from_offset(u64::MAX, -1.0, 1.0),
            u64::MAX - 1,
        );
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let sum = call(vocabulary::SUM, value(2), value(3));
        assert_eq!(
            evaluate(&call(vocabulary::MULTIPLY, sum, value(4))),
            value(20)
        );
        let difference = call(vocabulary::SUBTRACT, value(7), value(1));
        assert_eq!(
            evaluate(&call(vocabulary::DIVIDE, difference, value(4))),
            value(1)
        );
    }

    #[test]
    fn unrepresentable_results_are_library_absents() {
        assert_eq!(
            evaluate(&call(vocabulary::SUM, value(u64::MAX), value(1))),
            absent::with_reason(vocabulary::OVERFLOW)
        );
        assert_eq!(
            evaluate(&call(vocabulary::SUBTRACT, value(1), value(2))),
            absent::with_reason(vocabulary::OVERFLOW)
        );
        assert_eq!(
            evaluate(&call(vocabulary::MULTIPLY, value(u64::MAX), value(2))),
            absent::with_reason(vocabulary::OVERFLOW)
        );
        assert_eq!(
            evaluate(&call(vocabulary::DIVIDE, value(1), value(0))),
            absent::with_reason(vocabulary::DIVISION_BY_ZERO)
        );
    }

    #[test]
    fn comparisons_are_logic_values() {
        assert_eq!(
            evaluate(&call(vocabulary::LESS, value(1), value(2))),
            logic::value(true)
        );
        assert_eq!(
            evaluate(&call(vocabulary::LESS, value(2), value(2))),
            logic::value(false)
        );
        assert_eq!(
            evaluate(&call(vocabulary::EQUAL, value(2), value(2))),
            logic::value(true)
        );
    }

    #[test]
    fn operands_of_other_representations_decline() {
        assert_eq!(
            evaluate(&call(vocabulary::SUM, crate::f64::value(2.0), value(3))),
            absent::with_reason(vocabulary::LEFT_NOT_U64)
        );
        assert_eq!(
            evaluate(&call(vocabulary::SUM, value(2), crate::f32::value(3.0))),
            absent::with_reason(vocabulary::RIGHT_NOT_U64)
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library::<(), ()>();
        assert_eq!(
            library.value(vocabulary::SUM).and_then(name::read),
            Some("+")
        );
        assert_eq!(
            library.value(vocabulary::OVERFLOW).and_then(name::read),
            Some("u64 overflow")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::DIVISION_BY_ZERO)),
            Some(vocabulary::DIVISION_BY_ZERO)
        );
    }
}
