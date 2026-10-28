use super::*;
use crate::unboxed::{Native, Unboxed};
use gid::new_cell_id;
use std::cell::RefCell;

/// A call built at runtime from `fields`, its function among them.
fn runtime_call(fields: impl IntoIterator<Item = (CellId, RuntimeValue)>) -> RuntimeValue {
    RuntimeValue::record(
        [(isa::ISA, Value::from(vocabulary::kind::CALL).into())]
            .into_iter()
            .chain(fields),
    )
}

/// A record's unboxed form, as the evaluator makes it on entry.
fn unboxed(fields: gid::Record) -> RuntimeValue {
    RuntimeValue::new(RuntimeValueKind::Unboxed(
        Unboxed::recognize(&fields).expect("the record follows a privileged convention"),
    ))
}

/// Text made natively, with no source record.
fn native_text(text: &str) -> RuntimeValue {
    RuntimeValue::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::Text(
        text.into(),
    ))))
}

#[test]
fn callable_parameters_inspect_runtime_and_stored_forms_without_evaluation() {
    let first = new_cell_id();
    let second = new_cell_id();
    let syntax = lambda([second, first], Value::record([]));
    let closure = evaluate(&syntax, &host(vec![]), 100).result;
    let generated = RuntimeValue::record([
        (isa::ISA, Value::from(vocabulary::kind::LAMBDA).into()),
        (
            vocabulary::PARAMS,
            RuntimeValue::list([Value::from(second).into(), Value::from(first).into()]),
        ),
        (vocabulary::BODY, closure.clone()),
    ]);
    for value in [
        syntax.into(),
        closure.clone(),
        closure.to_value().into(),
        generated,
    ] {
        assert_eq!(value.callable_parameters(), Some(vec![second, first]));
        assert!(value.1.get().is_none());
    }
    let unwritten = RuntimeValue::from(isa::record(
        vocabulary::kind::LAMBDA,
        [(
            vocabulary::PARAMS,
            Value::list([Value::from(second), Value::from(first)]),
        )],
    ));
    assert_eq!(
        unwritten.callable_parameters(),
        Some(vec![second, first]),
        "a lambda has its parameters before its body is written"
    );
    // Neither is a lambda, the first not saying it is.
    for value in [
        RuntimeValue::record([(vocabulary::PARAMS, RuntimeValue::list([]))]),
        RuntimeValue::record([
            (
                vocabulary::PARAMS,
                RuntimeValue::list([RuntimeValue::f64(1.0)]),
            ),
            (vocabulary::BODY, closure.clone()),
        ]),
        RuntimeValue::record([(vocabulary::LAMBDA, RuntimeValue::f64(1.0))]),
        RuntimeValue::f64(1.0),
    ] {
        assert_eq!(value.callable_parameters(), None);
    }
}

#[test]
fn located_runtime_evaluation_retains_embedded_closures_and_current_call_origins() {
    let sink = new_cell_id();
    let creation = new_cell_id();
    let invocation = new_cell_id();
    let closure = evaluate_runtime_at(
        &RuntimeValue::from(lambda([], call(sink.into(), []))),
        Some(SourceOrigin::Stored(vec![gid::Step::Key(creation)])),
        &host(vec![]),
        100,
    )
    .result;
    let traces = Rc::new(RefCell::new(Vec::new()));
    let tracer = Rc::new(Tracer::default());
    let receiver = host(vec![(
        sink,
        Definition::foreign(
            sink,
            Value::record([]),
            ForeignFunction::new({
                let (traces, tracer) = (traces.clone(), tracer.clone());
                move |_, _, _| {
                    traces.borrow_mut().push(tracer.trace().unwrap());
                    Ok(RuntimeValue::f64(7.0))
                }
            }),
        ),
    )]);
    let expression = runtime_call([(vocabulary::FUNCTION, closure.clone())]);
    let result = evaluate_runtime_at(
        &expression,
        Some(SourceOrigin::Stored(vec![gid::Step::Key(invocation)])),
        &Observed::new(&receiver, &*tracer),
        100,
    );
    assert!(result.completed);
    assert_eq!(result.result.as_f64(), Some(7.0));
    assert_eq!(
        traces.borrow()[0].origins().cloned().collect::<Vec<_>>(),
        vec![
            SourceOrigin::Stored(vec![
                gid::Step::Key(creation),
                gid::Step::Key(vocabulary::BODY)
            ]),
            SourceOrigin::Stored(vec![gid::Step::Key(invocation)]),
        ]
    );
    // Returning an embedded callback must not serialize and re-lower it.
    let wrapper = RuntimeValue::record([
        (
            isa::ISA,
            Value::from(vocabulary::kind::VALUE_WRAPPER).into(),
        ),
        (vocabulary::VALUE, closure.clone()),
    ]);
    assert!(
        evaluate_runtime_at(&wrapper, None, &receiver, 100)
            .result
            .same_result(&closure)
    );
}

#[test]
fn runtime_syntax_origins_descend_through_generated_lists_and_lambdas() {
    let field = new_cell_id();
    let root = new_cell_id();
    let syntax = RuntimeValue::record([(
        field,
        RuntimeValue::list([RuntimeValue::record([
            (isa::ISA, Value::from(vocabulary::kind::LAMBDA).into()),
            (vocabulary::PARAMS, RuntimeValue::list([])),
            (vocabulary::BODY, RuntimeValue::record([])),
        ])]),
    )]);
    let receiver = host(vec![]);
    let mut context = context(&receiver, None, 100);
    let origin = SourceOrigin::Stored(vec![gid::Step::Key(root)]);
    let lowered = context.lower_runtime_code_at(
        &syntax,
        Some(OriginId(Rc::new(OriginNode::Root(OriginRoot::Located(
            origin,
        ))))),
    );
    let list = lowered.field(field).unwrap();
    let lambda = list.elements().unwrap()[0].clone();
    let body = lambda.field(vocabulary::BODY).unwrap();
    assert_eq!(
        body.source_origin(),
        Some(SourceOrigin::Stored(vec![
            gid::Step::Key(root),
            gid::Step::Key(field),
            gid::Step::Element(gid::position::spread(1).remove(0)),
            gid::Step::Key(vocabulary::BODY),
        ]))
    );
}

#[test]
fn runtime_evaluation_matches_stored_code_fuel_and_optional_origins() {
    let sink = new_cell_id();
    let location = new_cell_id();
    let traces = Rc::new(RefCell::new(Vec::new()));
    let tracer = Rc::new(Tracer::default());
    let receiver = host(vec![(
        sink,
        Definition::foreign(
            sink,
            Value::record([]),
            ForeignFunction::new({
                let (traces, tracer) = (traces.clone(), tracer.clone());
                move |_, _, _| {
                    traces.borrow_mut().push(tracer.trace());
                    Ok(RuntimeValue::f64(7.0))
                }
            }),
        ),
    )]);
    let runtime = RuntimeValue::record([
        (
            isa::ISA,
            Value::from(vocabulary::kind::VALUE_WRAPPER).into(),
        ),
        (
            vocabulary::VALUE,
            runtime_call([(vocabulary::FUNCTION, Value::from(sink).into())]),
        ),
    ]);
    let stored = runtime.to_value();
    for origin in [
        None,
        Some(SourceOrigin::Stored(vec![gid::Step::Key(location)])),
    ] {
        for fuel in [0, 1, 100] {
            traces.borrow_mut().clear();
            let observed = Observed::new(&receiver, &*tracer);
            let a = evaluate_runtime_at(
                &RuntimeValue::from(&stored),
                origin.clone(),
                &observed,
                fuel,
            );
            let b = evaluate_runtime_at(&runtime, origin.clone(), &observed, fuel);
            assert_eq!(a.completed, b.completed);
            assert_eq!(a.remaining_fuel, b.remaining_fuel);
            assert!(a.result.same_result(&b.result));
            if a.completed {
                let traces = traces.borrow();
                let origins = |index: usize| {
                    traces[index]
                        .as_ref()
                        .map(|trace| trace.origins().cloned().collect::<Vec<_>>())
                };
                assert_eq!(origins(0), origins(1));
            }
        }
    }
}

#[test]
fn runtime_record_keys_and_membership_match_all_gid_shapes() {
    let metadata = new_cell_id();
    let closure = evaluate(&lambda([], f64::value(7.0)), &host(vec![]), 100).result;
    let decorated_number = gid::Record::from_iter([
        (crate::f64::F64, Value::from(3.0_f64.to_le_bytes().to_vec())),
        (metadata, Value::from(vec![9])),
    ]);
    let decorated_text = gid::Record::from_iter([
        (crate::text::UTF8, Value::from(b"three".to_vec())),
        (metadata, Value::from(vec![9])),
    ]);
    for value in [
        RuntimeValue::from(Value::record([(metadata, Value::record([]))])),
        RuntimeValue::record([(metadata, closure.clone())]),
        RuntimeValue::f64(3.0),
        unboxed(decorated_number),
        native_text("three"),
        unboxed(decorated_text),
        closure,
        RuntimeValue::list([]),
        RuntimeValue::from(Value::from(metadata)),
        RuntimeValue::from(Value::from(vec![1, 2])),
    ] {
        let keys = value.record_keys();
        let answers = [
            value.as_blob().is_some(),
            value.as_cell().is_some(),
            value.list_positions().is_some(),
            keys.is_some(),
        ];
        assert_eq!(answers.iter().filter(|answer| **answer).count(), 1);
        match value.shape() {
            crate::Shape::Blob(bytes) => assert_eq!(value.as_blob(), Some(bytes)),
            crate::Shape::Cell(cell) => assert_eq!(value.as_cell(), Some(cell)),
            crate::Shape::List(positions) => assert_eq!(value.list_positions(), Some(positions)),
            crate::Shape::Record(fields) => assert_eq!(keys, Some(fields)),
        }
        for key in [
            metadata,
            crate::f64::F64,
            crate::text::UTF8,
            vocabulary::LAMBDA,
            vocabulary::ENVIRONMENT,
        ] {
            assert_eq!(
                value.contains_field(key),
                keys.as_ref().is_some_and(|keys| keys.contains(&key))
            );
        }
        let gid = value.to_value();
        assert_eq!(
            keys,
            gid.as_record()
                .map(|fields| fields.keys().copied().collect())
        );
    }
}

#[test]
fn runtime_list_positions_preserve_stored_positions_and_generated_children() {
    let closure = evaluate(&lambda([], f64::value(7.0)), &host(vec![]), 100).result;
    let generated = RuntimeValue::list([closure.clone(), RuntimeValue::f64(2.0)]);
    let positions = generated.list_positions().unwrap();
    assert_eq!(
        positions,
        generated
            .to_value()
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    );
    assert!(
        generated
            .list_element(&positions[0])
            .unwrap()
            .same_result(&closure)
    );
    let inserted = gid::position::between(Some(&positions[0]), Some(&positions[1])).unwrap();
    let mut stored = gid::List::new();
    stored.set(positions[0].clone(), Value::record([]));
    stored.set(inserted.clone(), f64::value(1.0));
    stored.set(positions[1].clone(), f64::value(2.0));
    let stored = RuntimeValue::from(Value::List(stored));
    assert_eq!(
        stored.list_positions(),
        Some(vec![
            positions[0].clone(),
            inserted.clone(),
            positions[1].clone()
        ])
    );
    assert_eq!(stored.list_element(&inserted).unwrap().as_f64(), Some(1.0));
    assert_eq!(RuntimeValue::record([]).list_positions(), None);
}

#[test]
fn interpreting_an_accelerated_number_as_code_preserves_syntax_precedence() {
    let interpret = new_cell_id();
    let source = gid::Record::from_iter([
        (crate::f64::F64, Value::from(3.0_f64.to_le_bytes().to_vec())),
        (isa::ISA, Value::from(vocabulary::kind::VALUE_WRAPPER)),
        (vocabulary::VALUE, f64::value(9.0)),
    ]);
    let value = unboxed(source);
    let receiver = host(vec![(
        interpret,
        Definition::foreign(
            interpret,
            Value::record([]),
            ForeignFunction::new(move |context, _, environment| {
                context.eval_runtime_code(&value, environment)
            }),
        ),
    )]);
    let result = evaluate(&call(interpret.into(), []), &receiver, 100);
    assert!(result.completed);
    assert_eq!(result.result.as_f64(), Some(9.0));
}

#[test]
fn runtime_atom_inspection_matches_gid_without_materializing_containers() {
    let metadata = new_cell_id();
    let closure = evaluate(&lambda([], f64::value(7.0)), &host(vec![]), 100).result;
    for bytes in [
        3.5_f64.to_le_bytes().to_vec(),
        vec![],
        vec![0; 7],
        vec![0; 9],
    ] {
        let blob = RuntimeValue::from(Value::from(bytes.clone()));
        assert_eq!(blob.as_blob(), Some(bytes.as_slice()));
        let number = RuntimeValue::record([(crate::f64::F64, blob), (metadata, closure.clone())]);
        assert_eq!(number.as_f64(), crate::f64::read(&number.to_value()));
        assert_eq!(number.as_blob(), None);
        assert_eq!(number.record_len(), Some(2));
        assert!(number.field(metadata).unwrap().same_result(&closure));
        assert!(
            number.1.get().is_none(),
            "inspection must not cache a GID view"
        );
    }
    for other in [
        closure,
        RuntimeValue::list([]),
        RuntimeValue::from(Value::from(metadata)),
        RuntimeValue::f64(1.0),
    ] {
        assert_eq!(other.as_blob(), None);
    }
}

#[test]
fn text_unboxes_on_entry_and_keeps_its_source_record() {
    let (metadata, parameter) = (new_cell_id(), new_cell_id());
    let decorated = Value::record([
        (crate::text::UTF8, Value::from(b"hello".to_vec())),
        (metadata, Value::from(vec![9])),
    ]);
    // A literal, and the same text passed through a closure's parameter,
    // unbox to native text and materialize as their exact source.
    let echo = lambda([parameter], Value::from(parameter));
    for expression in [
        decorated.clone(),
        call(echo, [(parameter, decorated.clone())]),
    ] {
        let result = evaluate(&expression, &host(vec![]), 100).result;
        assert!(matches!(&result.0, RuntimeValueKind::Unboxed(_)));
        assert_eq!(result.as_text(), Some("hello"));
        assert_eq!(
            result.field(metadata).map(|value| value.to_value()),
            Some(Value::from(vec![9]))
        );
        assert_eq!(result.into_value(), decorated);
    }
    // Bytes that aren't UTF-8 stay data.
    let invalid = Value::record([(crate::text::UTF8, Value::from(vec![0xff]))]);
    let result = evaluate(&invalid, &host(vec![]), 100).result;
    assert!(matches!(&result.0, RuntimeValueKind::Data(_)));
    assert_eq!(result.as_text(), None);
    // Text made natively answers as its record.
    let native = native_text("hi");
    assert_eq!(native.as_text(), Some("hi"));
    assert_eq!(native.as_f64(), None);
    assert_eq!(native.to_value(), crate::text::value("hi"));
    assert_eq!(
        native
            .field(crate::text::UTF8)
            .map(|value| value.to_value()),
        Some(Value::from(b"hi".to_vec()))
    );
    assert!(native.same_result(&native_text("hi")));
    assert!(!native.same_result(&native_text("ho")));
    assert!(!native.same_result(&RuntimeValue::f64(1.0)));
}

#[test]
fn interpreting_unboxed_text_as_code_preserves_syntax_precedence() {
    let interpret = new_cell_id();
    let source = gid::Record::from_iter([
        (crate::text::UTF8, Value::from(b"three".to_vec())),
        (isa::ISA, Value::from(vocabulary::kind::VALUE_WRAPPER)),
        (vocabulary::VALUE, f64::value(9.0)),
    ]);
    let value = unboxed(source);
    assert_eq!(value.as_text(), Some("three"));
    let receiver = host(vec![(
        interpret,
        Definition::foreign(
            interpret,
            Value::record([]),
            ForeignFunction::new(move |context, _, environment| {
                context.eval_runtime_code(&value, environment)
            }),
        ),
    )]);
    let result = evaluate(&call(interpret.into(), []), &receiver, 100);
    assert!(result.completed);
    assert_eq!(result.result.as_f64(), Some(9.0));
}

#[test]
fn runtime_text_inspection_matches_gid_without_materializing_containers() {
    let metadata = new_cell_id();
    let closure = evaluate(&lambda([], f64::value(7.0)), &host(vec![]), 100).result;
    for bytes in [b"hello".to_vec(), vec![], vec![0xff]] {
        let blob = RuntimeValue::from(Value::from(bytes.clone()));
        let text = RuntimeValue::record([(crate::text::UTF8, blob), (metadata, closure.clone())]);
        assert_eq!(text.as_text(), crate::text::read(&text.to_value()));
        assert!(
            text.1.get().is_none(),
            "inspection must not cache a GID view"
        );
        let data = RuntimeValue::from(Value::record([(
            crate::text::UTF8,
            Value::from(bytes.clone()),
        )]));
        assert_eq!(data.as_text(), std::str::from_utf8(&bytes).ok());
    }
    // A number whose record also carries text answers as both.
    let both = unboxed(gid::Record::from_iter([
        (crate::f64::F64, Value::from(3.0_f64.to_le_bytes().to_vec())),
        (crate::text::UTF8, Value::from(b"three".to_vec())),
    ]));
    assert_eq!(both.as_f64(), Some(3.0));
    assert_eq!(both.as_text(), Some("three"));
    for other in [
        closure,
        RuntimeValue::list([]),
        RuntimeValue::from(Value::from(metadata)),
        RuntimeValue::f64(1.0),
    ] {
        assert_eq!(other.as_text(), None);
    }
}

#[test]
fn generated_calls_retain_closures_and_their_original_inline_source() {
    let sink = new_cell_id();
    let first = new_cell_id();
    let second = new_cell_id();
    let empty = host(vec![]);
    let make = |field| {
        evaluate_runtime_at(
            &RuntimeValue::from(lambda([], call(sink.into(), []))),
            Some(SourceOrigin::Stored(vec![gid::Step::Key(field)])),
            &empty,
            100,
        )
        .result
    };
    let a = make(first);
    let b = make(second);
    assert!(
        !a.same_result(&b),
        "identical code at different origins is not interchangeable"
    );
    assert_ne!(a.to_value(), b.to_value());
    for (closure, field) in [(a, first), (b, second)] {
        let source = RefCell::new(None);
        let tracer = Tracer::default();
        let capture = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
            *source.borrow_mut() = tracer.trace();
            Ok(RuntimeValue::f64(7.0))
        };
        let ids = [sink];
        let overlay = ForeignOverlay::new(&ids, &capture);
        let expression = runtime_call([(vocabulary::FUNCTION, closure)]);
        let result =
            evaluate_runtime_scoped(&expression, &Observed::new(&empty, &tracer), &overlay, 100);
        assert!(result.completed);
        assert_eq!(result.result.as_f64(), Some(7.0));
        assert_eq!(
            source
                .borrow()
                .as_ref()
                .unwrap()
                .origins()
                .cloned()
                .collect::<Vec<_>>(),
            [SourceOrigin::Stored(vec![
                gid::Step::Key(field),
                gid::Step::Key(vocabulary::BODY)
            ])]
        );
    }
}

fn host(definitions: Vec<(CellId, Definition)>) -> impl Host {
    TestHost(move |cell| {
        definitions
            .iter()
            .find(|(key, _)| *key == cell)
            .map(|(_, value)| value.clone())
    })
}

#[test]
fn expression_application_preserves_raw_foreign_arguments_and_runtime_callbacks() {
    let (identity, argument, data) = (new_cell_id(), new_cell_id(), new_cell_id());
    let receiver = host(vec![
        (data, Definition::Value(f64::value(9.0))),
        (
            identity,
            Definition::foreign(
                identity,
                Value::record([]),
                ForeignFunction::new(move |context, call, environment| {
                    context.eval(call.field(argument).unwrap(), environment)
                }),
            ),
        ),
    ]);
    let source_argument = RuntimeValue::from(Value::from(data));
    let result = apply_expression(
        &Value::from(identity).into(),
        [(argument, source_argument.clone())],
        &receiver,
        None,
        100,
    );
    assert_eq!(result.result.as_f64(), Some(9.0));
    let result = apply(
        &Value::from(identity).into(),
        [(argument, source_argument)],
        &receiver,
        None,
        100,
    );
    assert_eq!(
        result.result.as_cell(),
        Some(data),
        "already-evaluated arguments remain data"
    );

    let closure = evaluate(&lambda([], f64::value(7.0)), &receiver, 100).result;
    let result = apply_expression(
        &Value::from(identity).into(),
        [(argument, closure.clone())],
        &receiver,
        None,
        100,
    );
    assert!(
        result.result.same_result(&closure),
        "the host adapter must retain native closure code"
    );
    let nested = RuntimeValue::list([closure.clone()]);
    let position = gid::position::spread(1).into_iter().next().unwrap();
    assert!(
        nested
            .list_element(&position)
            .unwrap()
            .same_result(&closure)
    );
    assert!(
        nested.1.get().is_none(),
        "following a runtime child must not materialize the container"
    );
}

#[test]
fn runtime_built_callable_records_match_their_gid_forms() {
    let (identity, argument) = (new_cell_id(), new_cell_id());
    let receiver = host(vec![(
        identity,
        Definition::foreign(
            identity,
            Value::record([]),
            ForeignFunction::new(move |context, call, environment| {
                context.eval(call.field(argument).unwrap(), environment)
            }),
        ),
    )]);
    let foreign = RuntimeValue::from(Value::from(identity));
    let closure = RuntimeValue::record([
        (isa::ISA, Value::from(vocabulary::kind::CLOSURE).into()),
        (
            vocabulary::LAMBDA,
            RuntimeValue::record([
                (vocabulary::PARAMS, RuntimeValue::list([])),
                (vocabulary::BODY, Value::from(argument).into()),
            ]),
        ),
        (
            vocabulary::ENVIRONMENT,
            RuntimeValue::record([(argument, RuntimeValue::f64(8.0))]),
        ),
    ]);
    for function in [foreign, closure] {
        let expected = apply_value(
            &function.to_value(),
            [(argument, f64::value(8.0))],
            &receiver,
            None,
            100,
        );
        assert_eq!(expected.result, f64::value(8.0));
        let actual = apply(
            &function,
            [(argument, RuntimeValue::f64(8.0))],
            &receiver,
            None,
            100,
        );
        assert_eq!(actual.result.to_value(), expected.result);
        let expression = runtime_call([
            (vocabulary::FUNCTION, function),
            (argument, RuntimeValue::f64(8.0)),
        ]);
        let noop = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| unreachable!();
        let overlay = ForeignOverlay::new(&[], &noop);
        assert_eq!(
            evaluate_runtime_scoped(&expression, &receiver, &overlay, 100)
                .result
                .to_value(),
            expected.result
        );
    }
}

#[test]
fn unboxed_values_keep_the_callable_facets_of_their_records() {
    let (identity, argument) = (new_cell_id(), new_cell_id());
    let receiver = host(vec![(
        identity,
        Definition::foreign(
            identity,
            Value::record([]),
            ForeignFunction::new(move |context, call, environment| {
                context.eval(call.field(argument).unwrap(), environment)
            }),
        ),
    )]);
    let number = (crate::f64::F64, Value::from(3.0_f64.to_le_bytes().to_vec()));
    let closure = gid::Record::from_iter([
        number,
        (isa::ISA, Value::from(vocabulary::kind::CLOSURE)),
        (
            vocabulary::LAMBDA,
            Value::record([
                (vocabulary::PARAMS, Value::list([])),
                (vocabulary::BODY, Value::from(argument)),
            ]),
        ),
        (
            vocabulary::ENVIRONMENT,
            Value::record([(argument, f64::value(8.0))]),
        ),
    ]);
    let noop = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| unreachable!();
    let overlay = ForeignOverlay::new(&[], &noop);
    for record in [closure] {
        let unboxed = unboxed(record.clone());
        assert_eq!(unboxed.as_f64(), Some(3.0));
        // The number answers each call as its record form does.
        for function in [RuntimeValue::from(Value::Record(record)), unboxed] {
            let arguments = || [(argument, RuntimeValue::f64(8.0))];
            assert_eq!(
                apply(&function, arguments(), &receiver, None, 100)
                    .result
                    .as_f64(),
                Some(8.0)
            );
            assert_eq!(
                apply_expression(&function, arguments(), &receiver, None, 100)
                    .result
                    .as_f64(),
                Some(8.0)
            );
            let expression = runtime_call(
                [(vocabulary::FUNCTION, function.clone())]
                    .into_iter()
                    .chain(arguments()),
            );
            assert_eq!(
                evaluate_runtime_scoped(&expression, &receiver, &overlay, 100)
                    .result
                    .as_f64(),
                Some(8.0)
            );
        }
    }
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

    for (offset, callback_value) in [
        (1.0, closure.clone()),
        (2.0, RuntimeValue::from(closure.to_value())),
    ] {
        let traces = Rc::new(RefCell::new(Vec::new()));
        let output = traces.clone();
        let tracer = Rc::new(Tracer::default());
        let observer = tracer.clone();
        let receiver = host(vec![(
            sink,
            Definition::foreign(
                sink,
                Value::record([]),
                ForeignFunction::new(move |context, call, environment| {
                    output.borrow_mut().push(observer.trace().unwrap());
                    let value = context.eval(call.field(captured).unwrap(), environment)?;
                    Ok(RuntimeValue::f64(value.as_f64().unwrap() + offset))
                }),
            ),
        )]);
        let result = apply(
            &driver,
            [
                (callback, callback_value),
                (captured, RuntimeValue::f64(999.0)),
            ],
            &Observed::new(&receiver, &*tracer),
            None,
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
fn closure_origins_survive_repeated_round_trips_without_rebasing_or_resolving() {
    let (sink, creation, invocation, ancestor) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let empty = host(vec![]);
    let path = vec![
        gid::Step::Key(creation),
        gid::Step::Element(gid::position::between(None, None).unwrap()),
    ];
    for origin in [
        Some(SourceOrigin::Stored(path.clone())),
        Some(SourceOrigin::Cell {
            cell: ancestor,
            path,
        }),
        None,
    ] {
        let native = evaluate_runtime_at(
            &RuntimeValue::from(lambda([], call(sink.into(), []))),
            origin.clone(),
            &empty,
            100,
        )
        .result;
        let expected = source::origin(match &native.0 {
            RuntimeValueKind::Closure(closure) => &closure.body,
            _ => panic!("expected a closure"),
        });
        let mut encoded = native.to_value();
        let trace = RefCell::new(None);
        let tracer = Tracer::default();
        let capture = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
            *trace.borrow_mut() = tracer.trace();
            Ok(RuntimeValue::f64(7.0))
        };
        let ids = [sink];
        let overlay = ForeignOverlay::new(&ids, &capture);
        let observed = Observed::new(&empty, &tracer);
        for _ in 0..3 {
            // The origin's cell is deliberately undefined in the receiving host.
            let mut context = context(&observed, Some(&overlay), 100);
            let closure = RuntimeValue::new(RuntimeValueKind::Closure(
                context.runtime_closure(&encoded.clone().into()).unwrap(),
            ));
            let next = closure.to_value();
            assert_eq!(next, encoded);
            encoded = next;
            let expression = runtime_call([(vocabulary::FUNCTION, closure)]);
            let code = context.lower_runtime_code_at(
                &expression,
                Some(OriginId(Rc::new(OriginNode::Root(OriginRoot::Located(
                    SourceOrigin::Stored(vec![gid::Step::Key(invocation)]),
                ))))),
            );
            let result = context
                .eval(code, &Environment::default())
                .unwrap_or_else(|halt| panic!("{:?}", halt.0));
            assert_eq!(result.as_f64(), Some(7.0));
            assert_eq!(
                trace
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .origins()
                    .cloned()
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .cloned()
                    .chain([SourceOrigin::Stored(vec![gid::Step::Key(invocation)])])
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn generated_or_malformed_closure_origins_fall_back_to_the_invoking_call() {
    let (sink, caller, callback) = (new_cell_id(), new_cell_id(), new_cell_id());
    let empty = host(vec![]);
    let generated = {
        let mut context = context(&empty, None, 100);
        context.closure_value([], call(sink.into(), []), &Environment::default())
    };
    let serialized = generated.to_value();
    let fields = serialized.as_record().unwrap();
    assert_eq!(fields.get(&source::vocabulary::BODY_ORIGIN), None);
    let malformed = Value::Record(fields.with(source::vocabulary::BODY_ORIGIN, f64::value(3.0)));
    let driver = evaluate_runtime_at(
        &RuntimeValue::from(lambda([callback], call(callback.into(), []))),
        Some(SourceOrigin::Stored(vec![gid::Step::Key(caller)])),
        &empty,
        100,
    )
    .result;
    let trace = RefCell::new(None);
    let tracer = Tracer::default();
    let capture = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
        *trace.borrow_mut() = tracer.trace();
        Ok(RuntimeValue::f64(7.0))
    };
    let ids = [sink];
    let overlay = ForeignOverlay::new(&ids, &capture);
    for value in [generated, serialized.into(), malformed.into()] {
        let result = apply(
            &driver,
            [(callback, value)],
            &Observed::new(&empty, &tracer),
            Some(&overlay),
            100,
        );
        assert!(result.completed);
        assert_eq!(result.result.as_f64(), Some(7.0));
        assert_eq!(
            trace
                .borrow()
                .as_ref()
                .unwrap()
                .origins()
                .cloned()
                .collect::<Vec<_>>(),
            [SourceOrigin::Stored(vec![
                gid::Step::Key(caller),
                gid::Step::Key(vocabulary::BODY)
            ])]
        );
    }
}

#[test]
fn inline_closures_anchor_at_the_nearest_cell_not_the_reference_occurrence() {
    let (first, second, enclosing, sink) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let creator = TestHost(move |cell| {
        if cell == first || cell == second {
            Some(Definition::Value(enclosing.into()))
        } else if cell == enclosing {
            Some(Definition::Value(isa::record(
                vocabulary::kind::VALUE_WRAPPER,
                [(vocabulary::VALUE, lambda([], call(sink.into(), [])))],
            )))
        } else {
            None
        }
    });
    let a = evaluate(&first.into(), &creator, 100).result.to_value();
    let b = evaluate(&second.into(), &creator, 100).result.to_value();
    assert_eq!(a, b);
    let fields = a.as_record().unwrap();
    assert_eq!(
        source::read(fields.get(&source::vocabulary::BODY_ORIGIN).unwrap()),
        Some(SourceOrigin::Cell {
            cell: enclosing,
            path: vec![
                gid::Step::Key(vocabulary::VALUE),
                gid::Step::Key(vocabulary::BODY)
            ],
        })
    );
}

#[test]
fn embedded_closure_origins_survive_a_generated_body_and_captured_environment() {
    let (sink, location, binding) = (new_cell_id(), new_cell_id(), new_cell_id());
    let empty = host(vec![]);
    let inner = evaluate_runtime_at(
        &RuntimeValue::from(lambda([], call(sink.into(), []))),
        Some(SourceOrigin::Stored(vec![gid::Step::Key(location)])),
        &empty,
        100,
    )
    .result;
    let mut context = context(&empty, None, 100);
    let environment = Environment::default().extended_runtime([(binding, inner.clone())]);
    let captured = context.closure_value([], call(binding.into(), []), &environment);
    let inline = context.lower_runtime_code(&runtime_call([(vocabulary::FUNCTION, inner)]));
    let embedded = context.closure([], inline, &Environment::default());
    let trace = RefCell::new(None);
    let tracer = Tracer::default();
    let capture = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
        *trace.borrow_mut() = tracer.trace();
        Ok(RuntimeValue::f64(7.0))
    };
    let ids = [sink];
    let overlay = ForeignOverlay::new(&ids, &capture);
    for closure in [captured, embedded] {
        for value in [closure.clone(), closure.to_value().into()] {
            let result = apply(
                &value,
                [],
                &Observed::new(&empty, &tracer),
                Some(&overlay),
                100,
            );
            assert!(result.completed);
            assert_eq!(result.result.as_f64(), Some(7.0));
            assert_eq!(
                trace
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .origins()
                    .cloned()
                    .collect::<Vec<_>>(),
                [SourceOrigin::Stored(vec![
                    gid::Step::Key(location),
                    gid::Step::Key(vocabulary::BODY)
                ])]
            );
        }
    }
}

#[test]
fn foreign_arguments_keep_runtime_closures_without_materializing_them() {
    let (identity, argument) = (new_cell_id(), new_cell_id());
    let receiver = host(vec![(
        identity,
        Definition::foreign(
            identity,
            Value::record([]),
            ForeignFunction::new(move |context, call, environment| {
                let expression = call.field(argument).unwrap();
                assert!(expression.0.source.1.get().is_none());
                context.eval(expression, environment)
            }),
        ),
    )]);
    let original = evaluate(&lambda([], f64::value(7.0)), &receiver, 100).result;
    let RuntimeValueKind::Closure(before) = &original.0 else {
        panic!("expected a closure")
    };
    let result = apply(
        &Value::from(identity).into(),
        [(argument, original.clone())],
        &receiver,
        None,
        100,
    );
    assert!(result.completed);
    let RuntimeValueKind::Closure(after) = &result.result.0 else {
        panic!("expected a closure")
    };
    assert_eq!(before.body, after.body);
    assert_eq!(
        apply(&result.result, [], &receiver, None, 100)
            .result
            .as_f64(),
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
                    capture,
                    Value::record([]),
                    ForeignFunction::new(move |context, call, environment| {
                        let expression = call.field(argument).unwrap();
                        Ok(context.prepare_callable(expression, environment)?.value)
                    }),
                ),
            ),
            (
                target,
                Definition::foreign(
                    target,
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
            target,
            Value::record([]),
            ForeignFunction::new(|_, _, _| Ok(RuntimeValue::f64(2.0))),
        ),
    )]);
    assert_eq!(
        apply(&callable, [], &receiver, None, 100).result.as_f64(),
        Some(2.0)
    );
    assert!(
        apply(&callable, [], &host(vec![]), None, 100)
            .result
            .absent_reason()
            .is_some()
    );
}

#[test]
fn retained_callables_use_the_receiving_scoped_capability() {
    let (capture, target, argument) = (new_cell_id(), new_cell_id(), new_cell_id());
    let host = host(vec![(
        capture,
        Definition::foreign(
            capture,
            Value::record([]),
            ForeignFunction::new(move |context, call, environment| {
                let expression = call.field(argument).unwrap();
                Ok(context.prepare_callable(expression, environment)?.value)
            }),
        ),
    )]);
    let callable = {
        let original = RuntimeValue::f64(1.0);
        let capability =
            |_, _: &mut Context<'_>, _: &Expression, _: &Environment| Ok(original.clone());
        evaluate_runtime_scoped(
            &RuntimeValue::from(call(capture.into(), [(argument, target.into())])),
            &host,
            &ForeignOverlay::new(&[target], &capability),
            100,
        )
        .result
    };
    let replacement = RuntimeValue::f64(2.0);
    let capability =
        |_, _: &mut Context<'_>, _: &Expression, _: &Environment| Ok(replacement.clone());
    let result = apply(
        &callable,
        [],
        &host,
        Some(&ForeignOverlay::new(&[target], &capability)),
        100,
    );
    assert!(result.completed);
    assert_eq!(result.result.as_f64(), Some(2.0));
    assert!(
        apply(&callable, [], &host, None, 100)
            .result
            .absent_reason()
            .is_some()
    );
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

#[test]
fn every_call_path_binds_the_callee_and_charges_the_same_steps() {
    let argument = new_cell_id();
    let receiver = host(vec![]);
    let closure = evaluate(
        &lambda(
            [vocabulary::FUNCTION, argument],
            vocabulary::FUNCTION.into(),
        ),
        &receiver,
        100,
    )
    .result;
    let seven = || [(argument, RuntimeValue::f64(7.0))];
    let expression = runtime_call([
        (vocabulary::FUNCTION, closure.clone()),
        (argument, RuntimeValue::f64(7.0)),
    ]);
    let results = [
        evaluate_runtime_at(&expression, None, &receiver, 100),
        context(&receiver, None, 100).conclude(|context| context.apply(&closure, seven())),
        apply_expression(&closure, seven(), &receiver, None, 100),
    ];
    for result in &results {
        assert!(result.completed);
        assert!(result.result.same_result(&closure));
        assert_eq!(result.remaining_fuel, results[0].remaining_fuel);
    }
}

#[test]
fn every_entry_point_binds_in_its_context_index_table() {
    let (probe, argument) = (new_cell_id(), new_cell_id());
    let shared = Rc::new(std::cell::Cell::new(true));
    let seen = shared.clone();
    let receiver = host(vec![(
        probe,
        Definition::foreign(
            probe,
            Value::record([]),
            ForeignFunction::new(move |context, _, environment| {
                seen.set(seen.get() && Rc::ptr_eq(&environment.indices, &context.indices));
                Ok(RuntimeValue::f64(1.0))
            }),
        ),
    )]);
    // An inline lambda calls the probe, which sees the environment its
    // parameter's binding made.
    let lambda_call = runtime_call([
        (
            vocabulary::FUNCTION,
            lambda([argument], call(probe.into(), [])).into(),
        ),
        (argument, RuntimeValue::f64(2.0)),
    ]);
    let none: [CellId; 0] = [];
    let unused = |_: CellId, _: &mut Context<'_>, _: &Expression, _: &Environment| {
        Ok(RuntimeValue::f64(0.0))
    };
    let overlay = ForeignOverlay::new(&none, &unused);
    let results = [
        evaluate(&call(probe.into(), []), &receiver, 100),
        evaluate_runtime_at(&lambda_call, None, &receiver, 100),
        evaluate_runtime_scoped(&lambda_call, &receiver, &overlay, 100),
        apply(&Value::from(probe).into(), [], &receiver, None, 100),
        apply_expression(&Value::from(probe).into(), [], &receiver, None, 100),
    ];
    for result in &results {
        assert_eq!(result.result.as_f64(), Some(1.0));
    }
    assert!(shared.get());
}

#[test]
fn runtime_records_order_and_repeat_labels_as_gid_records_do() {
    let mut labels = [new_cell_id(), new_cell_id(), new_cell_id()];
    labels.sort();
    let [a, b, c] = labels;
    let fields = [(c, 1.0), (a, 2.0), (c, 3.0), (b, 4.0), (a, 5.0)];
    let runtime = RuntimeValue::record(
        fields
            .iter()
            .map(|&(label, number)| (label, RuntimeValue::f64(number))),
    );
    let stored = Value::record(
        fields
            .iter()
            .map(|&(label, number)| (label, f64::value(number))),
    );
    assert_eq!(runtime.record_keys(), Some(vec![a, b, c]));
    assert_eq!(runtime.to_value(), stored);
    assert_eq!(runtime.field(a).and_then(|value| value.as_f64()), Some(5.0));
    assert_eq!(runtime.field(c).and_then(|value| value.as_f64()), Some(3.0));
}
