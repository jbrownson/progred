use super::*;
use crate::libraries::{absent, logic, name};
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
    let library = library::<crate::Editor>();
    assert_eq!(
        library
            .definitions
            .value(vocabulary::SUM)
            .and_then(name::read),
        Some("+")
    );
    assert_eq!(
        library
            .definitions
            .value(vocabulary::EQUAL)
            .and_then(name::read),
        Some("==")
    );
    assert_eq!(
        library
            .definitions
            .value(vocabulary::LEFT_NOT_F32)
            .and_then(name::read),
        Some("left is not f32")
    );
    assert_eq!(
        absent::reason(&absent::with_reason(vocabulary::RIGHT_NOT_F32)),
        Some(vocabulary::RIGHT_NOT_F32)
    );
}
