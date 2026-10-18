use super::*;
use crate::libraries::{line_edit, logic, name};
use ::grap;
use gid::new_cell_id;

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
    crate::libraries::test_evaluate(expression, |_| None, &functions(), 20).result
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
        crate::libraries::test_evaluate(
            &grap::call(
                grap::ffi(vocabulary::UPDATE),
                [
                    (line_edit::vocabulary::CURRENT, current.clone()),
                    (
                        line_edit::vocabulary::INPUT,
                        crate::libraries::text::value(input),
                    ),
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
    assert!(crate::libraries::absent::is_absent(&update("-1")));
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
        evaluate(&call(
            vocabulary::SUM,
            crate::libraries::f64::value(2.0),
            value(3)
        )),
        absent::with_reason(vocabulary::LEFT_NOT_U64)
    );
    assert_eq!(
        evaluate(&call(
            vocabulary::SUM,
            value(2),
            crate::libraries::f32::value(3.0)
        )),
        absent::with_reason(vocabulary::RIGHT_NOT_U64)
    );
}

#[test]
fn library_names_are_ordinary_facts_for_random_identities() {
    let library = library::<crate::Editor>();
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
