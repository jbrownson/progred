use super::*;
use crate::libraries::logic;
use crate::test_support::evaluate_with;
use crate::test_values::binary_call;
use gid::new_cell_id;

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
fn spelling_round_trips_through_the_line_conversion() {
    assert_eq!(
        number::edit("3.5", None, value).as_ref().and_then(read),
        Some(3.5)
    );
}

#[test]
fn rust_supplies_arithmetic_to_grap() {
    let sum = binary_call(vocabulary::SUM, value(2.0), value(3.0));
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::MULTIPLY, sum, value(4.0))
        ),
        value(20.0)
    );
    let difference = binary_call(vocabulary::SUBTRACT, value(7.0), value(1.0));
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::DIVIDE, difference, value(4.0))
        ),
        value(1.5)
    );
}

#[test]
fn comparisons_are_logic_values() {
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::LESS, value(1.0), value(2.0))
        ),
        logic::value(true)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::LESS, value(2.0), value(1.0))
        ),
        logic::value(false)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::EQUAL, value(2.0), value(2.0))
        ),
        logic::value(true)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::EQUAL, value(2.0), value(2.5))
        ),
        logic::value(false)
    );
}

#[test]
fn operands_of_other_representations_decline() {
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(
                vocabulary::SUM,
                crate::libraries::f64::value(2.0),
                value(3.0)
            )
        ),
        ::grap::absent::value(vocabulary::LEFT_NOT_F32)
    );
    assert_eq!(
        evaluate_with(
            &library(),
            &binary_call(vocabulary::SUM, value(2.0), Value::from(b"three".to_vec()))
        ),
        ::grap::absent::value(vocabulary::RIGHT_NOT_F32)
    );
}
