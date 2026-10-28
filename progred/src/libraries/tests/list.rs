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
        crate::test_support::test_evaluate(&at, |_| None, &functions, 30)
            .result
            .to_value(),
        Value::from(b"b".to_vec())
    );
}

/// The elements an operation keeps keep their positions, so a path into its
/// result still names the same element.
#[test]
fn prepend_and_tail_keep_their_elements_positions() {
    let list = crate::test_values::typed_list([Value::from(vec![1]), Value::from(vec![2])]);
    let positions = crate::test_values::positions(&list);
    let evaluate = |expression: &Value| {
        crate::test_support::test_evaluate(expression, |_| None, &functions(), 30).result
    };
    let prepended = evaluate(&call(
        vocabulary::PREPEND,
        [
            (vocabulary::LIST, list.clone()),
            (vocabulary::ITEM, Value::from(vec![0])),
        ],
    ));
    assert_eq!(
        prepended.to_value(),
        Value::list([0, 1, 2].map(|byte| Value::from(vec![byte])))
    );
    assert_eq!(
        crate::test_values::positions(prepended.as_value())[1..],
        positions
    );
    let tail = evaluate(&call(vocabulary::TAIL, [(vocabulary::LIST, list)]));
    assert_eq!(tail.to_value(), Value::list([Value::from(vec![2])]));
    assert_eq!(
        crate::test_values::positions(tail.as_value()),
        positions[1..]
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
        assert_eq!(evaluation.halt, None);
        assert_eq!(evaluation.result.to_value(), expected);
        let exhausted = crate::test_support::test_evaluate(&expression, |_| None, &functions(), 1);
        assert!(exhausted.halt.is_some());
        assert_eq!(
            exhausted.result.absent_reason(),
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
        crate::test_support::test_evaluate(&unfolded, |_| None, &functions, 30)
            .result
            .to_value(),
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
        crate::test_support::test_evaluate(&iterated, |_| None, &functions, 30)
            .result
            .to_value(),
        Value::from(vec![2]),
    );
    assert_eq!(calls.get(), 4);
}
