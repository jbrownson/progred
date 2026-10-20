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

#[test]
fn iteration_and_unfold_only_consume_their_finished_reason() {
    let state = vocabulary::STATE;
    for operation in [vocabulary::ITERATE, vocabulary::UNFOLD] {
        let initial = Value::from(vec![7]);
        let failure = ::grap::absent::with_detail(
            gid::new_cell_id(),
            vocabulary::STATE,
            Value::from(vec![8]),
        );
        for result in [failure.clone(), ::grap::absent::value(vocabulary::FINISHED)] {
            let expression = call(
                operation,
                [
                    (vocabulary::INITIAL, initial.clone()),
                    (vocabulary::STEP, ::grap::lambda([state], result.clone())),
                ],
            );
            let evaluation =
                crate::test_support::test_evaluate(&expression, |_| None, &functions(), 100);
            assert!(evaluation.completed);
            assert_eq!(
                evaluation.result,
                if result == failure {
                    failure.clone()
                } else if operation == vocabulary::ITERATE {
                    initial.clone()
                } else {
                    Value::record([
                        (vocabulary::LIST, Value::list([])),
                        (vocabulary::STATE, initial.clone()),
                    ])
                }
            );
            let exhausted =
                crate::test_support::test_evaluate(&expression, |_| None, &functions(), 1);
            assert!(!exhausted.completed);
            assert_eq!(
                ::grap::absent::reason(&exhausted.result),
                Some(::grap::absent::FUEL_EXHAUSTED)
            );
        }
    }
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
