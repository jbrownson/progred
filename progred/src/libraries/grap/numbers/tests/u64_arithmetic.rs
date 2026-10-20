//! u64's arithmetic in Grap.

use crate::libraries::logic;
use crate::libraries::u64::*;
use crate::test_support::evaluate_with;
use crate::test_values::binary_call;

#[test]
fn rust_supplies_arithmetic_to_grap() {
    let sum = binary_call(
        crate::libraries::grap::numbers::u64::SUM,
        value(2),
        value(3),
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::MULTIPLY,
                sum,
                value(4)
            )
        ),
        value(20)
    );
    let difference = binary_call(
        crate::libraries::grap::numbers::u64::SUBTRACT,
        value(7),
        value(1),
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::DIVIDE,
                difference,
                value(4)
            )
        ),
        value(1)
    );
}

#[test]
fn unrepresentable_results_are_library_absents() {
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::SUM,
                value(u64::MAX),
                value(1)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::u64::OVERFLOW)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::SUBTRACT,
                value(1),
                value(2)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::u64::OVERFLOW)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::MULTIPLY,
                value(u64::MAX),
                value(2)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::u64::OVERFLOW)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::DIVIDE,
                value(1),
                value(0)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::u64::DIVISION_BY_ZERO)
    );
}

#[test]
fn comparisons_are_logic_values() {
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::LESS,
                value(1),
                value(2)
            )
        ),
        logic::value(true)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::LESS,
                value(2),
                value(2)
            )
        ),
        logic::value(false)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::EQUAL,
                value(2),
                value(2)
            )
        ),
        logic::value(true)
    );
}

#[test]
fn operands_of_other_representations_decline() {
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::SUM,
                crate::libraries::f64::value(2.0),
                value(3)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::u64::LEFT_NOT_U64)
    );
    assert_eq!(
        evaluate_with(
            &crate::libraries::grap::library(),
            &binary_call(
                crate::libraries::grap::numbers::u64::SUM,
                value(2),
                crate::libraries::f32::value(3.0)
            )
        ),
        ::grap::absent::value(crate::libraries::grap::numbers::u64::RIGHT_NOT_U64)
    );
}
