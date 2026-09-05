use crate::*;
use gid::new_cell_id;

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
    definitions: impl Fn(CellId) -> Vec<(Resolution, Definition)>,
    initial: Value,
    fuel: usize,
) -> (Evaluation, Value) {
    let effects = RefCell::new(initial);
    let invoke = new_cell_id();
    let functions = [operations.read, operations.write, invoke];
    let foreign = |cell, context: &mut Context<'_>, expression, environment: &Environment| {
        if cell == operations.write {
            let argument = context.field(expression, operations.value).unwrap();
            let value = context.eval(argument, environment)?;
            Ok(context.effect(|| {
                *effects.borrow_mut() = value.clone();
                value
            }))
        } else if cell == operations.read {
            Ok(effects.borrow().clone())
        } else {
            let argument = context.field(expression, operations.value).unwrap();
            let callable = context.prepare_callable(argument, environment)?;
            context.call_prepared(&callable, [])
        }
    };
    let overlay = ForeignOverlay::new(&functions, &foreign);
    let result = match invocation {
        Invocation::Expression => evaluate_scoped(&call(function, []), definitions, &overlay, fuel),
        Invocation::Applied => apply_scoped(&function, [], definitions, &overlay, fuel),
        Invocation::Prepared => evaluate_scoped(
            &call(invoke.into(), [(operations.value, function)]),
            definitions,
            &overlay,
            fuel,
        ),
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
    let evaluation = evaluate(&absent, |_| Vec::new(), 10);
    assert_eq!(evaluation.result, absent);
    assert!(evaluation.completed);
}

#[test]
fn effects_in_arguments_and_nested_calls_prevent_fallthrough() {
    let operations = Operations::new();
    let function = new_cell_id();
    let library = Resolution::Library(new_cell_id());
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
                    vec![
                        (
                            Resolution::Document,
                            Definition::Value(lambda([], after(effect.clone(), absent::decline()))),
                        ),
                        (
                            library,
                            Definition::ForeignFunction(ForeignFunction::new(|_, _, _| {
                                panic!("effectful decline must stop before fallback")
                            })),
                        ),
                    ]
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
fn ordinary_absent_returns_keep_effects_and_stop_dispatch() {
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
                vec![
                    (
                        Resolution::Document,
                        Definition::Value(lambda(
                            [],
                            after(operations.write(written.clone()), result.clone()),
                        )),
                    ),
                    (
                        Resolution::Library(new_cell_id()),
                        Definition::ForeignFunction(ForeignFunction::new(|_, _, _| {
                            panic!("ordinary absence is definitive")
                        })),
                    ),
                ]
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
fn pure_declines_keep_ordered_details() {
    let operations = Operations::new();
    let function = new_cell_id();
    let first = absent::with_detail(
        absent::DECLINED,
        absent::VALUE,
        Value::from(b"first".to_vec()),
    );
    let second = absent::with_detail(
        absent::DECLINED,
        absent::VALUE,
        Value::from(b"second".to_vec()),
    );
    for count in [1, 2] {
        let (evaluation, state) = run(
            Invocation::Expression,
            function.into(),
            &operations,
            |_| {
                [first.clone(), second.clone()]
                    .into_iter()
                    .take(count)
                    .map(|decline| {
                        (
                            Resolution::Library(new_cell_id()),
                            Definition::Value(lambda([], decline)),
                        )
                    })
                    .collect()
            },
            Value::record([]),
            100,
        );
        assert_eq!(
            evaluation.result,
            if count == 1 {
                first.clone()
            } else {
                absent::with_detail(
                    absent::DECLINED,
                    absent::CAUSES,
                    Value::list([first.clone(), second.clone()]),
                )
            }
        );
        assert_eq!(state, Value::record([]));
    }
}

#[test]
fn halts_stop_without_fallback_or_rolling_back_temporary_state() {
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
                    vec![(
                        Resolution::Document,
                        Definition::Value(lambda([], call(recurse.into(), []))),
                    )]
                } else {
                    assert_eq!(cell, function);
                    vec![
                        (
                            Resolution::Document,
                            Definition::Value(lambda(
                                [],
                                after(
                                    operations.write(Value::from(vec![42])),
                                    call(recurse.into(), []),
                                ),
                            )),
                        ),
                        (
                            Resolution::Library(new_cell_id()),
                            Definition::ForeignFunction(ForeignFunction::new(|_, _, _| {
                                panic!("halt must not fall through")
                            })),
                        ),
                    ]
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
            |_| Vec::new(),
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
fn effectful_preparation_cannot_skip_to_a_callable_candidate() {
    let operations = Operations::new();
    let function = new_cell_id();
    let (evaluation, state) = run(
        Invocation::Expression,
        function.into(),
        &operations,
        |_| {
            vec![
                (
                    Resolution::Document,
                    Definition::Value(after(
                        operations.write(Value::from(vec![42])),
                        Value::record([]),
                    )),
                ),
                (
                    Resolution::Library(new_cell_id()),
                    Definition::Value(lambda([], call(operations.read.into(), []))),
                ),
            ]
        },
        Value::record([]),
        100,
    );
    assert_eq!(
        absent::reason(&evaluation.result),
        Some(absent::EFFECTFUL_DECLINE)
    );
    assert!(!evaluation.completed);
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
                after(operations.write(written.clone()), call(function.into(), [])),
            ),
            &operations,
            |_| {
                vec![
                    (
                        Resolution::Document,
                        Definition::Value(lambda([], absent::decline())),
                    ),
                    (
                        Resolution::Library(new_cell_id()),
                        Definition::Value(lambda([], call(operations.read.into(), []))),
                    ),
                ]
            },
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
                Ok(RuntimeValue::from_value(if effectful {
                    context.effect(absent::decline)
                } else {
                    absent::decline()
                }))
            };
            let foreign = if staged {
                ForeignFunction::staged(move |_, _| Rc::new(implementation))
            } else {
                ForeignFunction::runtime(move |context, _, environment| {
                    implementation(context, environment)
                })
            };
            for invocation in INVOCATIONS {
                let (evaluation, _) = run(
                    invocation,
                    function.into(),
                    &operations,
                    |_| {
                        vec![
                            (
                                Resolution::Document,
                                Definition::ForeignFunction(foreign.clone()),
                            ),
                            (
                                Resolution::Library(new_cell_id()),
                                Definition::ForeignFunction(ForeignFunction::new(
                                    move |_, _, _| {
                                        assert!(
                                            !effectful,
                                            "effectful decline must not fall through"
                                        );
                                        Ok(Value::record([]))
                                    },
                                )),
                            ),
                        ]
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
                    assert_eq!(evaluation.result, Value::record([]));
                }
            }
        }
    }
}
