use super::*;
use gid::new_cell_id;

fn host(definitions: Vec<(CellId, Definition)>) -> impl Host {
    TestHost(move |cell| {
        definitions
            .iter()
            .find(|(key, _)| *key == cell)
            .map(|(_, value)| (Resolution::Document, value.clone()))
            .into_iter()
            .collect()
    })
}

#[test]
fn retained_closures_keep_code_and_bindings_but_use_the_current_call_stack() {
    let (factory, captured, sink, callback) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let closure = {
        let creator = host(vec![(
            factory,
            Definition::Value(lambda(
                [captured],
                lambda([], call(sink.into(), [(captured, captured.into())])),
            )),
        )]);
        let evaluation = evaluate(
            &call(factory.into(), [(captured, f64::value(42.0))]),
            &creator,
            100,
        );
        assert!(evaluation.completed);
        evaluation.result
    };
    let RuntimeValueKind::Closure(retained) = &closure.0 else {
        panic!("expected a closure")
    };
    let code = Rc::downgrade(&retained.body.0);
    let driver = evaluate(
        &lambda([callback, captured], call(callback.into(), [])),
        &host(vec![]),
        100,
    )
    .result;

    for offset in [1.0, 2.0] {
        let traces = Rc::new(RefCell::new(Vec::new()));
        let output = traces.clone();
        let receiver = host(vec![(
            sink,
            Definition::foreign(
                Value::record([]),
                ForeignFunction::new(move |context, call, environment| {
                    output.borrow_mut().push(context.call_trace().unwrap());
                    let value =
                        context.eval(context.field(call, captured).unwrap(), environment)?;
                    Ok(RuntimeValue::f64(value.as_f64().unwrap() + offset))
                }),
            ),
        )]);
        let result = apply(
            &driver,
            [
                (callback, closure.clone()),
                (captured, RuntimeValue::f64(999.0)),
            ],
            &receiver,
            100,
        );
        assert!(result.completed);
        assert_eq!(result.result.as_f64(), Some(42.0 + offset));
        let traces = traces.borrow();
        let origins: Vec<_> = traces[0].origins().cloned().collect();
        assert_eq!(
            origins,
            [
                SourceOrigin::Cell {
                    cell: factory,
                    source: Resolution::Document,
                    path: vec![
                        gid::Step::Key(vocabulary::BODY),
                        gid::Step::Key(vocabulary::BODY)
                    ]
                },
                SourceOrigin::Input(vec![gid::Step::Key(vocabulary::BODY)]),
            ]
        );
    }
    drop(closure);
    assert!(
        code.upgrade().is_none(),
        "execution caches must not keep code alive after the run"
    );
}

#[test]
fn foreign_arguments_keep_runtime_closures_without_materializing_them() {
    let (identity, argument) = (new_cell_id(), new_cell_id());
    let receiver = host(vec![(
        identity,
        Definition::foreign(
            Value::record([]),
            ForeignFunction::new(move |context, call, environment| {
                let expression = context.field(call, argument).unwrap();
                assert!(expression.0.source.get().is_none());
                context.eval(expression, environment)
            }),
        ),
    )]);
    let original = evaluate(&lambda([], f64::value(7.0)), &receiver, 100).result;
    let RuntimeValueKind::Closure(before) = &original.0 else {
        panic!("expected a closure")
    };
    let result = apply(
        &ffi(identity).into(),
        [(argument, original.clone())],
        &receiver,
        100,
    );
    assert!(result.completed);
    let RuntimeValueKind::Closure(after) = &result.result.0 else {
        panic!("expected a closure")
    };
    assert_eq!(before.body, after.body);
    assert_eq!(
        apply(&result.result, [], &receiver, 100).result.as_f64(),
        Some(7.0)
    );
}

#[test]
fn retained_foreign_references_do_not_keep_the_old_implementation() {
    let (capture, target, argument) = (new_cell_id(), new_cell_id(), new_cell_id());
    let marker = Rc::new(());
    let weak = Rc::downgrade(&marker);
    let callable = {
        let creator = host(vec![
            (
                capture,
                Definition::foreign(
                    Value::record([]),
                    ForeignFunction::new(move |context, call, environment| {
                        let expression = context.field(call, argument).unwrap();
                        Ok(context.prepare_callable(expression, environment)?.value)
                    }),
                ),
            ),
            (
                target,
                Definition::foreign(
                    Value::record([]),
                    ForeignFunction::new(move |_, _, _| {
                        let _ = &marker;
                        Ok(RuntimeValue::f64(1.0))
                    }),
                ),
            ),
        ]);
        evaluate(
            &call(capture.into(), [(argument, target.into())]),
            &creator,
            100,
        )
        .result
    };
    assert!(weak.upgrade().is_none());
    let receiver = host(vec![(
        target,
        Definition::foreign(
            Value::record([]),
            ForeignFunction::new(|_, _, _| Ok(RuntimeValue::f64(2.0))),
        ),
    )]);
    assert_eq!(
        apply(&callable, [], &receiver, 100).result.as_f64(),
        Some(2.0)
    );
    assert!(apply(&callable, [], &host(vec![]), 100).result.is_absent());
}

#[test]
fn retained_callables_use_the_receiving_scoped_capability() {
    let (capture, target, argument) = (new_cell_id(), new_cell_id(), new_cell_id());
    let host = host(vec![(
        capture,
        Definition::foreign(
            Value::record([]),
            ForeignFunction::new(move |context, call, environment| {
                let expression = context.field(call, argument).unwrap();
                Ok(context.prepare_callable(expression, environment)?.value)
            }),
        ),
    )]);
    let callable = {
        let original = RuntimeValue::f64(1.0);
        let capability =
            |_, _: &mut Context<'_>, _: &Expression, _: &Environment| Ok(original.clone());
        evaluate_scoped(
            &call(capture.into(), [(argument, target.into())]),
            &host,
            &ForeignOverlay::new(&[target], &capability),
            100,
        )
        .result
    };
    let replacement = RuntimeValue::f64(2.0);
    let capability =
        |_, _: &mut Context<'_>, _: &Expression, _: &Environment| Ok(replacement.clone());
    let result = apply_scoped(
        &callable,
        [],
        &host,
        &ForeignOverlay::new(&[target], &capability),
        100,
    );
    assert!(result.completed);
    assert_eq!(result.result.as_f64(), Some(2.0));
    assert!(apply(&callable, [], &host, 100).result.is_absent());
}

#[test]
fn runtime_results_distinguish_a_returned_absent_from_a_halt() {
    let host = host(vec![]);
    let absence = absent::value(absent::FUEL_EXHAUSTED);
    let returned = evaluate(&absence, &host, 10);
    let halted = evaluate(&absence, &host, 0);
    assert!(returned.completed);
    assert!(!halted.completed);
    assert_eq!(returned.result.into_value(), halted.result.into_value());
}
