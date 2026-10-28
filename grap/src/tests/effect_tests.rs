use crate::*;
use gid::new_cell_id;
use std::cell::RefCell;

fn after(effect: Value, result: Value) -> Value {
    let ignored = new_cell_id();
    call(lambda([ignored], result), [(ignored, effect)])
}

struct Operations {
    read: CellId,
    write: CellId,
    value: CellId,
}

impl Operations {
    fn new() -> Self {
        Self {
            read: new_cell_id(),
            write: new_cell_id(),
            value: new_cell_id(),
        }
    }

    fn write(&self, value: Value) -> Value {
        call(self.write.into(), [(self.value, value)])
    }
}

#[derive(Clone, Copy)]
enum Invocation {
    Expression,
    Applied,
    Prepared,
}

fn run(
    invocation: Invocation,
    function: Value,
    operations: &Operations,
    definitions: impl Fn(CellId) -> Option<Definition>,
    initial: Value,
    fuel: usize,
) -> (Evaluation<Value>, Value) {
    let definitions = TestHost(definitions);
    let effects = RefCell::new(initial);
    let invoke = new_cell_id();
    let functions = [operations.read, operations.write, invoke];
    let foreign =
        |cell, context: &mut Context<'_>, expression: &Expression, environment: &Environment| {
            if cell == operations.write {
                let argument = expression.field(operations.value).unwrap();
                let value = context.eval_to_value(argument, environment)?;
                Ok(context.effect(|| {
                    *effects.borrow_mut() = value.clone();
                    value
                }))
            } else if cell == operations.read {
                Ok(effects.borrow().clone())
            } else {
                let argument = expression.field(operations.value).unwrap();
                let callable = context.prepare_callable(argument, environment)?;
                context
                    .call_prepared(&callable, [])
                    .map(RuntimeValue::into_value)
            }
        };
    let overlay = ForeignOverlay::from_value(&functions, &foreign);
    let result = match invocation {
        Invocation::Expression => evaluate_runtime_scoped(
            &RuntimeValue::from(call(function, [])),
            &definitions,
            &overlay,
            fuel,
        )
        .into_value(),
        Invocation::Applied => apply_value(&function, [], &definitions, Some(&overlay), fuel),
        Invocation::Prepared => evaluate_runtime_scoped(
            &RuntimeValue::from(call(invoke.into(), [(operations.value, function)])),
            &definitions,
            &overlay,
            fuel,
        )
        .into_value(),
    };
    (result, effects.into_inner())
}

const INVOCATIONS: [Invocation; 3] = [
    Invocation::Expression,
    Invocation::Applied,
    Invocation::Prepared,
];

#[test]
fn a_returned_halt_shaped_value_is_still_a_completed_result() {
    let absent = absent::value(absent::FUEL_EXHAUSTED);
    let evaluation = evaluate_value(&absent, &TestHost(|_| None), 10);
    assert_eq!(evaluation.result, absent);
    assert!(evaluation.completed);
}

#[test]
fn effects_in_arguments_and_nested_calls_make_a_decline_effectful() {
    let operations = Operations::new();
    let function = new_cell_id();
    let initial = Value::from(b"initial".to_vec());
    for invocation in INVOCATIONS {
        for nested in [false, true] {
            let write = operations.write(Value::from(b"discarded".to_vec()));
            let effect = if nested {
                call(lambda([], after(write, Value::record([]))), [])
            } else {
                write
            };
            let (evaluation, state) = run(
                invocation,
                function.into(),
                &operations,
                |cell| {
                    assert_eq!(cell, function);
                    Some(Definition::from(lambda(
                        [],
                        after(effect.clone(), absent::decline()),
                    )))
                },
                initial.clone(),
                100,
            );
            assert_eq!(
                absent::reason(&evaluation.result),
                Some(absent::EFFECTFUL_DECLINE)
            );
            assert_eq!(state, Value::from(b"discarded".to_vec()));
            assert!(!evaluation.completed);
        }
    }
}

#[test]
fn ordinary_absent_returns_keep_their_effects() {
    let operations = Operations::new();
    let function = new_cell_id();
    let result = absent::value(new_cell_id());
    let written = Value::from(b"written".to_vec());
    for invocation in INVOCATIONS {
        let (evaluation, state) = run(
            invocation,
            function.into(),
            &operations,
            |cell| {
                assert_eq!(cell, function);
                Some(Definition::from(lambda(
                    [],
                    after(operations.write(written.clone()), result.clone()),
                )))
            },
            Value::record([]),
            100,
        );
        assert_eq!(evaluation.result, result);
        assert_eq!(state, written);
        assert!(evaluation.completed);
    }
}

#[test]
fn a_pure_decline_keeps_its_details() {
    let operations = Operations::new();
    let function = new_cell_id();
    let decline = absent::with_detail(
        absent::DECLINED,
        absent::VALUE,
        Value::from(b"details".to_vec()),
    );
    let (evaluation, state) = run(
        Invocation::Expression,
        function.into(),
        &operations,
        |_| Some(Definition::from(lambda([], decline.clone()))),
        Value::record([]),
        100,
    );
    assert_eq!(evaluation.result, decline);
    assert_eq!(state, Value::record([]));
}

#[test]
fn halts_stop_without_rolling_back_temporary_state() {
    let operations = Operations::new();
    let function = new_cell_id();
    let recurse = new_cell_id();
    for invocation in INVOCATIONS {
        let (evaluation, state) = run(
            invocation,
            function.into(),
            &operations,
            |cell| {
                if cell == recurse {
                    Some(Definition::from(lambda([], call(recurse.into(), []))))
                } else {
                    assert_eq!(cell, function);
                    Some(Definition::from(lambda(
                        [],
                        after(
                            operations.write(Value::from(vec![42])),
                            call(recurse.into(), []),
                        ),
                    )))
                }
            },
            Value::record([]),
            50,
        );
        assert_eq!(evaluation.result, absent::value(absent::FUEL_EXHAUSTED));
        assert_eq!(evaluation.remaining_fuel, 0);
        assert!(!evaluation.completed);
        assert_eq!(state, Value::from(vec![42]));
    }
}

#[test]
fn an_effectful_decline_cannot_be_ignored_by_its_caller() {
    let operations = Operations::new();
    let initial = Value::from(b"initial".to_vec());
    for invocation in INVOCATIONS {
        let function = lambda(
            [],
            after(
                call(
                    lambda(
                        [],
                        after(operations.write(Value::from(vec![42])), absent::decline()),
                    ),
                    [],
                ),
                call(operations.read.into(), []),
            ),
        );
        let (evaluation, state) = run(
            invocation,
            function,
            &operations,
            |_| None,
            initial.clone(),
            100,
        );
        assert_eq!(
            absent::reason(&evaluation.result),
            Some(absent::EFFECTFUL_DECLINE)
        );
        assert!(!evaluation.completed);
        assert_eq!(state, Value::from(vec![42]));
    }
}

#[test]
fn a_non_callable_definition_returns_absent() {
    let operations = Operations::new();
    let function = new_cell_id();
    let (evaluation, state) = run(
        Invocation::Expression,
        function.into(),
        &operations,
        |_| {
            Some(Definition::from(after(
                operations.write(Value::from(vec![42])),
                Value::record([]),
            )))
        },
        Value::record([]),
        100,
    );
    assert_eq!(
        absent::reason(&evaluation.result),
        Some(absent::NOT_CALLABLE)
    );
    assert!(evaluation.completed);
    assert_eq!(state, Value::from(vec![42]));
}

#[test]
fn earlier_effects_do_not_prevent_a_later_pure_decline() {
    let operations = Operations::new();
    let function = new_cell_id();
    let written = Value::from(vec![42]);
    for invocation in INVOCATIONS {
        let (evaluation, state) = run(
            invocation,
            lambda(
                [],
                after(
                    operations.write(written.clone()),
                    after(call(function.into(), []), written.clone()),
                ),
            ),
            &operations,
            |_| Some(Definition::from(lambda([], absent::decline()))),
            Value::record([]),
            100,
        );
        assert!(evaluation.completed);
        assert_eq!(evaluation.result, written);
        assert_eq!(state, written);
    }
}

#[test]
fn rust_functions_obey_the_same_decline_contract() {
    for staged in [false, true] {
        for effectful in [false, true] {
            let operations = Operations::new();
            let function = new_cell_id();
            let implementation = move |context: &mut Context<'_>, _: &Environment| {
                Ok(RuntimeValue::from(if effectful {
                    context.effect(absent::decline)
                } else {
                    absent::decline()
                }))
            };
            let foreign = if staged {
                ForeignFunction::staged(move |_| Rc::new(implementation))
            } else {
                ForeignFunction::new(move |context, _, environment| {
                    implementation(context, environment)
                })
            };
            for invocation in INVOCATIONS {
                let (evaluation, _) = run(
                    invocation,
                    function.into(),
                    &operations,
                    |_| {
                        Some(Definition::foreign(
                            function,
                            gid::Record::default(),
                            foreign.clone(),
                        ))
                    },
                    Value::record([]),
                    100,
                );
                assert_eq!(evaluation.completed, !effectful);
                if effectful {
                    assert_eq!(
                        absent::reason(&evaluation.result),
                        Some(absent::EFFECTFUL_DECLINE)
                    );
                } else {
                    assert_eq!(evaluation.result, absent::decline());
                }
            }
        }
    }
}
