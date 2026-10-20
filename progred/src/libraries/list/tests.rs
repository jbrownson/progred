use super::*;
use crate::test_values::call;

#[test]
fn list_operations_are_value_operations() {
    let functions = functions();
    let concatenated = call(
        vocabulary::CONCAT,
        [
            (
                crate::libraries::grap::numbers::vocabulary::LEFT,
                Value::list([Value::from(b"a".to_vec())]),
            ),
            (
                crate::libraries::grap::numbers::vocabulary::RIGHT,
                Value::list([Value::from(b"b".to_vec())]),
            ),
        ],
    );
    let at = call(
        vocabulary::AT,
        [
            (vocabulary::LIST, concatenated),
            (vocabulary::INDEX, f64::value(1.0)),
        ],
    );
    assert_eq!(
        crate::test_support::test_evaluate(&at, |_| None, &functions, 30).result,
        Value::from(b"b".to_vec())
    );
}

fn check_step_results(operation: gid::CellId, results: impl IntoIterator<Item = (Value, Value)>) {
    let initial = Value::from(vec![7]);
    for (result, expected) in results {
        let expression = call(
            operation,
            [
                (vocabulary::INITIAL, initial.clone()),
                (
                    vocabulary::STEP,
                    ::grap::lambda([vocabulary::STATE], result),
                ),
            ],
        );
        let evaluation =
            crate::test_support::test_evaluate(&expression, |_| None, &functions(), 100);
        assert!(evaluation.completed);
        assert_eq!(evaluation.result, expected);
        let exhausted = crate::test_support::test_evaluate(&expression, |_| None, &functions(), 1);
        assert!(!exhausted.completed);
        assert_eq!(
            ::grap::absent::reason(&exhausted.result),
            Some(::grap::absent::FUEL_EXHAUSTED)
        );
    }
}

fn failure() -> Value {
    ::grap::absent::with_detail(gid::new_cell_id(), vocabulary::STATE, Value::from(vec![8]))
}

#[test]
fn iteration_only_consumes_its_finished_reason() {
    let failure = failure();
    check_step_results(
        vocabulary::ITERATE,
        [
            (failure.clone(), failure),
            (
                ::grap::absent::value(vocabulary::FINISHED),
                Value::from(vec![7]),
            ),
        ],
    );
}

#[test]
fn unfold_ends_at_a_step_without_a_state_and_passes_failures_through() {
    let failure = failure();
    let ended = |list| {
        Value::record([
            (vocabulary::LIST, list),
            (vocabulary::STATE, Value::from(vec![7])),
        ])
    };
    let item = Value::from(vec![1]);
    check_step_results(
        vocabulary::UNFOLD,
        [
            (failure.clone(), failure),
            (Value::record([]), ended(Value::list([]))),
            (
                Value::record([(vocabulary::ITEM, item.clone())]),
                ended(Value::list([item.clone()])),
            ),
            (item, ::grap::absent::value(vocabulary::INVALID_STEP)),
        ],
    );
}

#[test]
fn unfold_collects_items_until_a_step_has_no_state() {
    use std::cell::Cell;
    use std::rc::Rc;

    let step = gid::new_cell_id();
    let calls = Rc::new(Cell::new(0u8));
    let step_calls = calls.clone();
    let functions = functions().register(
        step,
        ForeignFunction::from_value(move |_, _, _| {
            let call = step_calls.get();
            step_calls.set(call + 1);
            let item = (vocabulary::ITEM, Value::from(vec![call]));
            Ok(if call == 3 {
                Value::record([item])
            } else {
                Value::record([item, (vocabulary::STATE, Value::from(vec![call]))])
            })
        }),
    );
    let unfolded = call(
        vocabulary::UNFOLD,
        [
            (vocabulary::INITIAL, Value::from(vec![99])),
            (vocabulary::STEP, Value::from(step)),
        ],
    );
    assert_eq!(
        crate::test_support::test_evaluate(&unfolded, |_| None, &functions, 30).result,
        Value::record([
            (
                vocabulary::LIST,
                Value::list((0..4).map(|call| Value::from(vec![call]))),
            ),
            (vocabulary::STATE, Value::from(vec![2])),
        ]),
    );
    assert_eq!(calls.get(), 4);
}

#[test]
fn iterate_drives_a_step_without_collecting_intermediate_values() {
    use std::cell::Cell;
    use std::rc::Rc;

    let step = gid::new_cell_id();
    let calls = Rc::new(Cell::new(0));
    let step_calls = calls.clone();
    let functions = functions().register(
        step,
        ForeignFunction::from_value(move |_, _, _| {
            let call = step_calls.get();
            step_calls.set(call + 1);
            Ok(if call == 3 {
                ::grap::absent::value(vocabulary::FINISHED)
            } else {
                Value::from(vec![call as u8])
            })
        }),
    );
    let iterated = call(
        vocabulary::ITERATE,
        [
            (vocabulary::INITIAL, Value::from(vec![99])),
            (vocabulary::STEP, Value::from(step)),
        ],
    );
    assert_eq!(
        crate::test_support::test_evaluate(&iterated, |_| None, &functions, 30).result,
        Value::from(vec![2]),
    );
    assert_eq!(calls.get(), 4);
}
