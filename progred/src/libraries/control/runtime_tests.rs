use super::*;
use std::cell::RefCell;

fn control_call(function: CellId, clauses: Value, binder: CellId) -> Value {
    ::grap::call(
        function.into(),
        if function == vocabulary::MATCH {
            [
                (vocabulary::VALUE, Value::record([])),
                (vocabulary::CASES, clauses),
            ]
        } else {
            [
                (vocabulary::BINDINGS, clauses),
                (::grap::vocabulary::EXPRESSION, binder.into()),
            ]
        },
    )
}

fn clause(function: CellId, body: RuntimeValue, binder: CellId) -> RuntimeValue {
    RuntimeValue::record(if function == vocabulary::MATCH {
        [
            (vocabulary::PATTERN, RuntimeValue::record([])),
            (::grap::vocabulary::EXPRESSION, body),
        ]
    } else {
        [
            (vocabulary::BIND, Value::from(binder).into()),
            (vocabulary::VALUE, body),
        ]
    })
}

#[test]
fn computed_clauses_preserve_closure_code_captures_and_origins() {
    let (producer, sink, binder, captured, origin) = (
        gid::new_cell_id(),
        gid::new_cell_id(),
        gid::new_cell_id(),
        gid::new_cell_id(),
        gid::new_cell_id(),
    );
    let foreign = functions();
    let host = crate::libraries::test_host(|_| None, &foreign);
    let closure = ::grap::evaluate_at(
        &::grap::call(
            ::grap::lambda(
                [captured],
                ::grap::lambda([], ::grap::call(sink.into(), [(captured, captured.into())])),
            ),
            [(captured, crate::libraries::f64::value(7.0))],
        ),
        Some(::grap::SourceOrigin::Stored(vec![Step::Key(origin)])),
        &host,
        100,
    )
    .result;
    let expected_origin = ::grap::SourceOrigin::Stored(vec![
        Step::Key(origin),
        Step::Key(::grap::vocabulary::FUNCTION),
        Step::Key(::grap::vocabulary::BODY),
        Step::Key(::grap::vocabulary::BODY),
    ]);
    for function in [vocabulary::MATCH, vocabulary::LET, vocabulary::WHERE] {
        for invoke_inside in [false, true] {
            let body = if invoke_inside {
                RuntimeValue::record([(::grap::vocabulary::FUNCTION, closure.clone())])
            } else {
                closure.clone()
            };
            let clauses = RuntimeValue::list([clause(function, body, binder)]);
            let seen = Rc::new(RefCell::new(None));
            let capture = seen.clone();
            let foreign = functions()
                .register(
                    producer,
                    ForeignFunction::new(move |_, _, _| Ok(clauses.clone())),
                )
                .register(
                    sink,
                    ForeignFunction::new(move |context, call, environment| {
                        *capture.borrow_mut() = context.call_trace();
                        context.eval(context.field(call, captured).unwrap(), environment)
                    }),
                );
            let host = crate::libraries::test_host(|_| None, &foreign);
            let result = ::grap::evaluate(
                &control_call(function, ::grap::call(producer.into(), []), binder),
                &host,
                100,
            );
            assert!(result.completed);
            let result = if invoke_inside {
                result
            } else {
                assert!(
                    result.result.same_result(&closure),
                    "computed clauses must retain the actual closure"
                );
                ::grap::apply(&result.result, [], &host, 100)
            };
            assert!(result.completed);
            assert_eq!(result.result.as_f64(), Some(7.0));
            assert_eq!(
                seen.borrow().as_ref().unwrap().origins().next(),
                Some(&expected_origin)
            );
        }
    }
}

fn runtime_container(value: &Value) -> RuntimeValue {
    match value {
        Value::Record(fields) => RuntimeValue::record(
            fields
                .iter()
                .map(|(key, value)| (*key, runtime_container(value))),
        ),
        Value::List(values) => RuntimeValue::list(values.values().map(runtime_container)),
        _ => value.into(),
    }
}

#[test]
fn computed_controls_agree_on_fuel_errors_and_effect_order_across_representations() {
    let producer = gid::new_cell_id();
    let effect = gid::new_cell_id();
    let binder = gid::new_cell_id();
    let other = gid::new_cell_id();
    let good = RuntimeValue::from(::grap::call(effect.into(), []));
    let ordinary_absent = absent::with_reason(vocabulary::PATTERN_MISMATCH);
    for function in [vocabulary::MATCH, vocabulary::LET, vocabulary::WHERE] {
        let valid = clause(function, good.clone(), binder).to_value();
        let absent_clause = clause(function, ordinary_absent.clone().into(), binder).to_value();
        let mismatch = Value::record([
            (vocabulary::PATTERN, Value::from(vec![99])),
            (
                if function == vocabulary::MATCH {
                    ::grap::vocabulary::EXPRESSION
                } else {
                    vocabulary::VALUE
                },
                ::grap::call(effect.into(), []),
            ),
        ]);
        let invalid_binder = Value::record([
            (
                vocabulary::PATTERN,
                Value::record([(vocabulary::BIND, Value::from(vec![]))]),
            ),
            (
                if function == vocabulary::MATCH {
                    ::grap::vocabulary::EXPRESSION
                } else {
                    vocabulary::VALUE
                },
                ::grap::call(effect.into(), []),
            ),
        ]);
        let malformed = Value::record([]);
        let mut cases = vec![
            Value::from(vec![]),
            Value::list([]),
            Value::list([valid.clone()]),
            Value::list([malformed.clone(), valid.clone()]),
            Value::list([valid.clone(), malformed]),
            Value::list([absent_clause, valid.clone()]),
            Value::list([mismatch, valid.clone()]),
            Value::list([invalid_binder, valid.clone()]),
        ];
        if function != vocabulary::MATCH {
            cases.push(Value::list([
                clause(function, good.clone(), other).to_value(),
                clause(function, Value::from(other).into(), binder).to_value(),
            ]));
            cases.push(Value::list([Value::record([
                (vocabulary::BIND, binder.into()),
                (vocabulary::PATTERN, Value::record([])),
                (vocabulary::VALUE, ::grap::call(effect.into(), [])),
            ])]));
        }
        for clauses in cases {
            for fuel in 0..30 {
                let run = |clauses: RuntimeValue| {
                    let effects = Rc::new(RefCell::new(Vec::new()));
                    let producing = effects.clone();
                    let executing = effects.clone();
                    let foreign = functions()
                        .register(
                            producer,
                            ForeignFunction::new(move |context, _, _| {
                                context.effect(|| producing.borrow_mut().push("produce"));
                                Ok(clauses.clone())
                            }),
                        )
                        .register(
                            effect,
                            ForeignFunction::new(move |context, _, _| {
                                context.effect(|| executing.borrow_mut().push("execute"));
                                Ok(RuntimeValue::f64(7.0))
                            }),
                        );
                    let result = ::grap::evaluate(
                        &control_call(function, ::grap::call(producer.into(), []), binder),
                        &crate::libraries::test_host(|_| None, &foreign),
                        fuel,
                    );
                    let effects = effects.borrow().clone();
                    (
                        result.result.to_value(),
                        result.completed,
                        result.remaining_fuel,
                        effects,
                    )
                };
                assert_eq!(
                    run(runtime_container(&clauses)),
                    run(clauses.clone().into())
                );
            }
        }
    }
}
