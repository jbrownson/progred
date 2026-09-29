//! An open f32 convention used by libraries whose host boundary is
//! single precision. It remains ordinary GID data and ordinary Grap
//! library behavior.

use crate::libraries::{Library, name, number};
use gid::{CellId, Value};

pub const ID: CellId = CellId::from_u128(0xf8daecede6e48de724408cfb0e3090f8);
use ::grap::{Context, Environment, Expression, ForeignFunction, Halt};

pub mod vocabulary {
    use gid::CellId;

    pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
    pub const FROM_F64: CellId = CellId::from_u128(0x73bd2225d2091ba893a5570e8e9bacff);
    pub const UPDATE: CellId = CellId::from_u128(0x9c34c242d73e090cbd62de1242ad74ae);
    pub const SUM: CellId = CellId::from_u128(0x257e5967d826e69d28929cf365667ae1);
    pub const SUBTRACT: CellId = CellId::from_u128(0x5cd7408dab067d8b92c7a1bd9cda7b05);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd4df1fe63a95cda53c19ad7220054c24);
    pub const DIVIDE: CellId = CellId::from_u128(0xfbf1ebc4eeeea21f86af583f6ad85e71);
    pub const LESS: CellId = CellId::from_u128(0x3dde12d1f97ccca65c579ae2703bb07a);
    pub const EQUAL: CellId = CellId::from_u128(0xe0637a60944f8dd9afd2fb4c28214dc3);
    pub const LEFT_NOT_F32: CellId = CellId::from_u128(0x2f7a7dfd96df51008a563a36e075e50b);
    pub const RIGHT_NOT_F32: CellId = CellId::from_u128(0xd4a7b035dc57953dca5c1d20cb591d0d);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0x722482f3e369634464ae68add98480e6);
}

pub fn value(number: f32) -> Value {
    Value::record([(vocabulary::F32, Value::from(number.to_le_bytes().to_vec()))])
}

pub fn completions(query: &str) -> Vec<crate::display::Completion> {
    convention().completions(query)
}

pub fn read(value: &Value) -> Option<f32> {
    value
        .as_record()?
        .get(&vocabulary::F32)?
        .as_blob()?
        .try_into()
        .ok()
        .map(f32::from_le_bytes)
}

impl number::Scrubbable for f32 {
    fn magnitude(self) -> f64 {
        self.into()
    }

    fn minimum_precision() -> f64 {
        0.0
    }

    fn scrubbable(self) -> bool {
        self.is_finite()
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        number::rounded(f64::from(start) + offset, precision) as f32
    }

    fn spelling(self, precision: f64) -> String {
        if precision >= 1.0 {
            self.round().to_string()
        } else {
            let decimal_places = (-precision.log10()).round().clamp(0.0, 8.0) as usize;
            format!("{self:.decimal_places$}")
        }
    }
}

pub(crate) fn convention() -> number::Convention<f32> {
    number::Convention {
        name: "f32",
        tag: vocabulary::F32,
        update: vocabulary::UPDATE,
        left_not: vocabulary::LEFT_NOT_F32,
        right_not: vocabulary::RIGHT_NOT_F32,
        invalid_input: vocabulary::INVALID_INPUT,
        encode: value,
        runtime: |number| value(number).into(),
        read: |value| {
            Some(f32::from_le_bytes(
                value.field(vocabulary::F32)?.as_blob()?.try_into().ok()?,
            ))
        },
        eval: |context, expression, environment| {
            Ok(read(&context.eval_to_value(expression, environment)?))
        },
    }
}

fn from_f64(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(operand) = context.field(call, number::vocabulary::OPERAND) else {
        return Ok(context.missing_argument(number::vocabulary::OPERAND));
    };
    let operand = context.eval_to_value(operand, environment)?;
    Ok(crate::libraries::f64::read(&operand)
        .map(|number| value(number as f32))
        .unwrap_or_else(|| {
            ::grap::absent::with_detail(
                vocabulary::INVALID_INPUT,
                number::vocabulary::OPERAND,
                operand,
            )
        }))
}

fn parts() -> number::Parts {
    use number::Operation::{Arithmetic, Comparison};
    let mut parts = convention().parts([
        (vocabulary::SUM, "+", Arithmetic(|left, right| left + right)),
        (
            vocabulary::SUBTRACT,
            "-",
            Arithmetic(|left, right| left - right),
        ),
        (
            vocabulary::MULTIPLY,
            "*",
            Arithmetic(|left, right| left * right),
        ),
        (
            vocabulary::DIVIDE,
            "/",
            Arithmetic(|left, right| left / right),
        ),
        (
            vocabulary::LESS,
            "<",
            Comparison(|left, right| left < right),
        ),
        (
            vocabulary::EQUAL,
            "==",
            Comparison(|left, right| left == right),
        ),
    ]);
    parts
        .cells
        .set_value(vocabulary::FROM_F64, name::record("f32 from f64", []));
    parts.functions = parts.functions.register(
        vocabulary::FROM_F64,
        ForeignFunction::from_value(from_f64).tracked(),
    );
    parts
}

#[cfg(test)]
pub fn functions() -> ::grap::ForeignFunctions {
    parts().functions
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    convention().library(ID, parts(), std::iter::empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::{absent, line_edit, logic};
    use gid::new_cell_id;

    fn call(function: CellId, left: Value, right: Value) -> Value {
        ::grap::call(
            Value::from(function),
            [
                (number::vocabulary::LEFT, left),
                (number::vocabulary::RIGHT, right),
            ],
        )
    }

    fn evaluate(expression: &Value) -> Value {
        crate::libraries::test_evaluate(expression, |_| None, &functions(), 20).result
    }

    #[test]
    fn conversion_from_f64_rounds_at_the_explicit_numeric_boundary() {
        let convert = |operand| {
            evaluate(&::grap::call(
                vocabulary::FROM_F64.into(),
                [(number::vocabulary::OPERAND, operand)],
            ))
        };
        for number in [
            0.0,
            -0.0,
            0.1,
            -3.25,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::INFINITY,
        ] {
            let result = convert(crate::libraries::f64::value(number));
            assert_eq!(read(&result).unwrap().to_bits(), (number as f32).to_bits());
        }
        assert!(
            read(&convert(crate::libraries::f64::value(f64::NAN)))
                .unwrap()
                .is_nan()
        );
        let invalid = value(1.0);
        assert_eq!(
            convert(invalid.clone()),
            ::grap::absent::with_detail(
                vocabulary::INVALID_INPUT,
                number::vocabulary::OPERAND,
                invalid,
            )
        );
        assert!(::grap::absent::is_absent(&evaluate(&::grap::call(
            vocabulary::FROM_F64.into(),
            [],
        ))));
    }

    #[test]
    fn representation_is_open_library_data() {
        assert_eq!(read(&value(1.25)), Some(1.25));
        assert_eq!(read(&Value::from(1.25_f32.to_le_bytes().to_vec())), None);

        let metadata = new_cell_id();
        let enriched = Value::record(
            value(1.25)
                .as_record()
                .unwrap()
                .clone()
                .update(metadata, Value::from(vec![1])),
        );
        assert_eq!(read(&enriched), Some(1.25));
    }

    #[test]
    fn spelling_round_trips_through_the_update_function() {
        let updated = crate::libraries::test_evaluate(
            &::grap::call(
                ::grap::ffi(vocabulary::UPDATE),
                [(
                    line_edit::vocabulary::INPUT,
                    crate::libraries::text::value("3.5"),
                )],
            ),
            |_| None,
            &functions(),
            20,
        );

        assert_eq!(read(&updated.result), Some(3.5));
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let sum = call(vocabulary::SUM, value(2.0), value(3.0));
        assert_eq!(
            evaluate(&call(vocabulary::MULTIPLY, sum, value(4.0))),
            value(20.0)
        );
        let difference = call(vocabulary::SUBTRACT, value(7.0), value(1.0));
        assert_eq!(
            evaluate(&call(vocabulary::DIVIDE, difference, value(4.0))),
            value(1.5)
        );
    }

    #[test]
    fn comparisons_are_logic_values() {
        assert_eq!(
            evaluate(&call(vocabulary::LESS, value(1.0), value(2.0))),
            logic::value(true)
        );
        assert_eq!(
            evaluate(&call(vocabulary::LESS, value(2.0), value(1.0))),
            logic::value(false)
        );
        assert_eq!(
            evaluate(&call(vocabulary::EQUAL, value(2.0), value(2.0))),
            logic::value(true)
        );
        assert_eq!(
            evaluate(&call(vocabulary::EQUAL, value(2.0), value(2.5))),
            logic::value(false)
        );
    }

    #[test]
    fn operands_of_other_representations_decline() {
        assert_eq!(
            evaluate(&call(
                vocabulary::SUM,
                crate::libraries::f64::value(2.0),
                value(3.0)
            )),
            absent::with_reason(vocabulary::LEFT_NOT_F32)
        );
        assert_eq!(
            evaluate(&call(
                vocabulary::SUM,
                value(2.0),
                Value::from(b"three".to_vec())
            )),
            absent::with_reason(vocabulary::RIGHT_NOT_F32)
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library();
        assert_eq!(
            library.value(vocabulary::SUM).and_then(name::read),
            Some("+")
        );
        assert_eq!(
            library.value(vocabulary::EQUAL).and_then(name::read),
            Some("==")
        );
        assert_eq!(
            library.value(vocabulary::LEFT_NOT_F32).and_then(name::read),
            Some("left is not f32")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::RIGHT_NOT_F32)),
            Some(vocabulary::RIGHT_NOT_F32)
        );
    }
}
