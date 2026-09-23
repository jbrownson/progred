use super::*;
use std::{cell::RefCell, rc::Rc};

fn call(function: gid::CellId, fields: impl IntoIterator<Item = (gid::CellId, Value)>) -> Value {
    ::grap::call(Value::from(function), fields)
}

fn range(count: f64) -> Value {
    call(RANGE, [(COUNT, f64::value(count))])
}

#[test]
fn ranges_are_lazy_and_collection_is_explicit() {
    let functions = functions();
    let huge = crate::libraries::test_evaluate(&range(1e12), |_| None, &functions, 30);
    assert!(huge.completed);
    let fields = huge.result.as_record().unwrap();
    assert!(fields.contains_key(&::grap::vocabulary::CLOSURE));
    let saved = crate::libraries::test_evaluate(&range(3.0), |_| None, &functions, 30).result;
    let restored = crate::libraries::test_evaluate(
        &call(COLLECT, [(ITEMS, saved)]),
        |_| None,
        &functions,
        1000,
    );
    assert_eq!(
        restored.result,
        Value::list([0.0, 1.0, 2.0].map(f64::value))
    );
    for count in [0, 1, 5] {
        let result = crate::libraries::test_evaluate(
            &call(COLLECT, [(ITEMS, range(count as f64))]),
            |_| None,
            &functions,
            1000,
        );
        assert!(result.completed);
        assert_eq!(
            result.result,
            Value::list((0..count).map(|n| f64::value(n as f64)))
        );
    }
    for count in [-1.0, 1.5, f64::NAN, f64::INFINITY, 1e20] {
        let result = crate::libraries::test_evaluate(&range(count), |_| None, &functions, 100);
        assert_eq!(absent::reason(&result.result), Some(INVALID));
    }
}

#[test]
fn calling_again_repeats_and_only_the_successor_advances() {
    let functions = functions();
    let first = crate::libraries::test_evaluate(&range(2.0), |_| None, &functions, 100).result;
    let pull = |sequence: &Value| {
        let result = crate::libraries::test_apply(sequence, [], |_| None, &functions, 100);
        assert!(result.completed);
        result.result
    };
    let first_item = pull(&first);
    assert_eq!(first_item, pull(&first));
    assert_eq!(
        first_item.as_record().unwrap().get(&ITEM),
        Some(&f64::value(0.0))
    );
    let second = first_item.as_record().unwrap().get(&NEXT).unwrap();
    let second_item = pull(second);
    assert_eq!(
        second_item.as_record().unwrap().get(&ITEM),
        Some(&f64::value(1.0))
    );
    assert_eq!(first_item, pull(&first));
    let end = second_item.as_record().unwrap().get(&NEXT).unwrap();
    assert_eq!(absent::reason(&pull(end)), Some(FINISHED));
    assert_eq!(pull(end), pull(end));
}

#[test]
fn grap_authored_producer_captures_its_environment_and_can_yield_absent_data() {
    use crate::libraries::control::{self, vocabulary as c};
    use ::grap::vocabulary::EXPRESSION;
    let captured = gid::new_cell_id();
    let unquote = |value| Value::record([(c::UNQUOTE, value)]);
    let quote = |value| call(c::QUOTE, [(EXPRESSION, value)]);
    let producer = ::grap::lambda(
        [captured],
        ::grap::lambda(
            [],
            quote(Value::record([
                (ITEM, unquote(Value::from(captured))),
                (
                    NEXT,
                    unquote(::grap::lambda([], absent::with_reason(FINISHED))),
                ),
            ])),
        ),
    );
    let functions = functions().merge(control::functions());
    for value in [f64::value(42.0), absent::with_reason(INVALID)] {
        let sequence = ::grap::call(producer.clone(), [(captured, value.clone())]);
        let result = crate::libraries::test_evaluate(
            &call(COLLECT, [(ITEMS, sequence)]),
            |_| None,
            &functions,
            1000,
        );
        assert!(result.completed);
        assert_eq!(result.result, Value::list([value]));
    }
}

#[test]
fn pulls_and_actions_interleave_and_action_failure_stops_before_next_pull() {
    let pull = gid::new_cell_id();
    let action = gid::new_cell_id();
    let log = Rc::new(RefCell::new(Vec::new()));
    let failure = absent::with_reason(crate::libraries::list::vocabulary::FINISHED);
    let functions = functions()
        .register(
            pull,
            ForeignFunction::runtime({
                let log = log.clone();
                move |cx, call, env| {
                    let index = evaluated(cx, call, env, INDEX)?.as_f64().unwrap();
                    cx.effect(|| log.borrow_mut().push(("pull", index)));
                    let next = cx.closure(
                        [],
                        ::grap::call(Value::from(pull), [(INDEX, f64::value(index + 1.0))]),
                        env,
                    );
                    Ok(RuntimeValue::record([
                        (ITEM, RuntimeValue::f64(index)),
                        (NEXT, next),
                    ]))
                }
            })
            .tracked(),
        )
        .register(
            action,
            ForeignFunction::runtime({
                let log = log.clone();
                let failure = failure.clone();
                move |cx, call, env| {
                    let index = evaluated(cx, call, env, ITEM)?.as_f64().unwrap();
                    cx.effect(|| log.borrow_mut().push(("action", index)));
                    Ok(if index == 2.0 {
                        failure.clone().into()
                    } else {
                        RuntimeValue::record([])
                    })
                }
            })
            .tracked(),
        );
    let sequence = ::grap::lambda([], call(pull, [(INDEX, f64::value(0.0))]));
    let result = crate::libraries::test_evaluate(
        &call(FOR_EACH, [(ITEMS, sequence), (ACTION, Value::from(action))]),
        |_| None,
        &functions,
        1000,
    );
    assert!(result.completed);
    assert_eq!(result.result, failure);
    assert_eq!(
        *log.borrow(),
        [
            ("pull", 0.0),
            ("action", 0.0),
            ("pull", 1.0),
            ("action", 1.0),
            ("pull", 2.0),
            ("action", 2.0)
        ]
    );
}

#[test]
fn empty_sequence_does_not_call_action_and_metadata_is_ignored() {
    let extra = gid::new_cell_id();
    let result = crate::libraries::test_evaluate(
        &call(
            FOR_EACH,
            [
                (
                    ITEMS,
                    call(
                        RANGE,
                        [(COUNT, f64::value(0.0)), (extra, Value::record([]))],
                    ),
                ),
                (ACTION, ::grap::lambda([ITEM], absent::with_reason(INVALID))),
            ],
        ),
        |_| None,
        &functions(),
        100,
    );
    assert_eq!(result.result, Value::record([]));
}

#[test]
fn producer_failures_malformed_steps_and_fuel_are_not_completion() {
    let step = gid::new_cell_id();
    let sequence = ::grap::lambda([], call(step, []));
    for output in [
        Value::record([]),
        Value::record([(ITEM, f64::value(1.0))]),
        absent::with_reason(::grap::absent::MISSING_CELL),
    ] {
        let functions = functions().register(
            step,
            ForeignFunction::runtime({
                let output = output.clone();
                move |_, _, _| Ok(output.clone().into())
            })
            .tracked(),
        );
        let result = crate::libraries::test_evaluate(
            &call(COLLECT, [(ITEMS, sequence.clone())]),
            |_| None,
            &functions,
            100,
        );
        assert!(result.completed);
        assert_eq!(
            result.result,
            if absent::reason(&output).is_some() {
                output
            } else {
                absent::with_reason(INVALID)
            }
        );
    }
    let result = crate::libraries::test_evaluate(
        &call(
            FOR_EACH,
            [
                (ITEMS, range(1e12)),
                (ACTION, ::grap::lambda([ITEM], Value::record([]))),
            ],
        ),
        |_| None,
        &functions(),
        100,
    );
    assert!(!result.completed);
    assert_eq!(
        absent::reason(&result.result),
        Some(::grap::absent::FUEL_EXHAUSTED)
    );
}
