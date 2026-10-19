use super::*;
use crate::libraries::logic;
use crate::test_support::evaluate_with;
use crate::test_values::binary_call;
use gid::new_cell_id;

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
    let update = |input: &str| number::edit(input, Some(&current), value);

    assert_eq!(read(&current), Some(u64::MAX));
    assert_eq!(
        update("42"),
        Some(Value::record(
            value(42)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"metadata".to_vec())),
        ))
    );
    assert_eq!(update("-1"), None);
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
    let sum = binary_call(vocabulary::SUM, value(2), value(3));
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::MULTIPLY, sum, value(4))
        ),
        value(20)
    );
    let difference = binary_call(vocabulary::SUBTRACT, value(7), value(1));
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::DIVIDE, difference, value(4))
        ),
        value(1)
    );
}

#[test]
fn unrepresentable_results_are_library_absents() {
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::SUM, value(u64::MAX), value(1))
        ),
        ::grap::absent::value(vocabulary::OVERFLOW)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::SUBTRACT, value(1), value(2))
        ),
        ::grap::absent::value(vocabulary::OVERFLOW)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::MULTIPLY, value(u64::MAX), value(2))
        ),
        ::grap::absent::value(vocabulary::OVERFLOW)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::DIVIDE, value(1), value(0))
        ),
        ::grap::absent::value(vocabulary::DIVISION_BY_ZERO)
    );
}

#[test]
fn comparisons_are_logic_values() {
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::LESS, value(1), value(2))
        ),
        logic::value(true)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::LESS, value(2), value(2))
        ),
        logic::value(false)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::EQUAL, value(2), value(2))
        ),
        logic::value(true)
    );
}

#[test]
fn operands_of_other_representations_decline() {
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::SUM, crate::libraries::f64::value(2.0), value(3))
        ),
        ::grap::absent::value(vocabulary::LEFT_NOT_U64)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::SUM, value(2), crate::libraries::f32::value(3.0))
        ),
        ::grap::absent::value(vocabulary::RIGHT_NOT_U64)
    );
}
