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
