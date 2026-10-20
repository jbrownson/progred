//! f32's arithmetic in Grap.

use crate::libraries::f32::*;
use crate::libraries::logic;
use crate::test_support::evaluate_with;
use crate::test_values::binary_call;
use gid::Value;

#[test]
fn rust_supplies_arithmetic_to_grap() {
    let sum = binary_call(
        crate::libraries::grap::numbers::f32::SUM,
        value(2.0),
        value(3.0),
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::MULTIPLY,
                sum,
                value(4.0)
            )
        ),
        value(20.0)
    );
    let difference = binary_call(
        crate::libraries::grap::numbers::f32::SUBTRACT,
        value(7.0),
        value(1.0),
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::DIVIDE,
                difference,
                value(4.0)
            )
        ),
        value(1.5)
    );
}

#[test]
fn comparisons_are_logic_values() {
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::LESS,
                value(1.0),
                value(2.0)
            )
        ),
        logic::value(true)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::LESS,
                value(2.0),
                value(1.0)
            )
        ),
        logic::value(false)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::EQUAL,
                value(2.0),
                value(2.0)
            )
        ),
        logic::value(true)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::EQUAL,
                value(2.0),
                value(2.5)
            )
        ),
        logic::value(false)
    );
}

#[test]
fn operands_of_other_representations_decline() {
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::SUM,
                crate::libraries::f64::value(2.0),
                value(3.0)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::f32::LEFT_NOT_F32)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::f32::SUM,
                value(2.0),
                Value::from(b"three".to_vec())
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::f32::RIGHT_NOT_F32)
    );
}

#[test]
fn f32_from_f64_rounds_at_the_explicit_numeric_boundary() {
    use crate::libraries::f64;
    use crate::libraries::grap::numbers::f32::{F32_FROM_F64, INVALID_F32_INPUT};
    use crate::libraries::grap::numbers::vocabulary::OPERAND;
    use ::core::primitive::f64 as N;
    let evaluate =
        |expression: &Value| evaluate_with(&crate::libraries::grap::library(), expression);
    let convert = |operand| evaluate(&::grap::call(F32_FROM_F64.into(), [(OPERAND, operand)]));
    for number in [0.0, -0.0, 0.1, -3.25, N::MAX, N::MIN_POSITIVE, N::INFINITY] {
        let result = convert(f64::value(number));
        assert_eq!(read(&result).unwrap().to_bits(), (number as f32).to_bits());
    }
    assert!(read(&convert(f64::value(N::NAN))).unwrap().is_nan());
    let invalid = value(1.0);
    assert_eq!(
        convert(invalid.clone()),
        ::grap::absent::with_detail(INVALID_F32_INPUT, OPERAND, invalid)
    );
    assert!(::grap::absent::is_absent(&evaluate(&::grap::call(
        F32_FROM_F64.into(),
        [],
    ))));
}
