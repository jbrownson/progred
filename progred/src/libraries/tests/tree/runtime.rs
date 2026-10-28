use super::*;

fn retained(body: Value, source: CellId, host: &dyn ::grap::Host) -> RuntimeValue {
    let result = ::grap::Evaluator::new(host, 1000).evaluate_runtime(
        &::grap::RuntimeValue::from(&body),
        Some(SourceOrigin::Root(vec![gid::Step::Key(source)])),
    );
    assert!(result.result.absent_reason().is_none());
    result.result
}

#[test]
fn collection_and_mapping_retain_callbacks_and_positions() {
    let host = crate::stack::load().libraries;
    let callback = retained(::grap::lambda([], f64::value(7.0)), A, &host);
    let maker = retained(
        ::grap::lambda(
            [VALUE],
            ::grap::lambda(
                [],
                group(Value::list([
                    leaf(VALUE.into()),
                    call(
                        MAP,
                        [
                            (MAPPING, ::grap::lambda([VALUE], VALUE.into())),
                            (CHILDREN, group(Value::list([leaf(VALUE.into())]))),
                        ],
                    ),
                ])),
            ),
        ),
        B,
        &host,
    );
    let program = ::grap::Evaluator::new(&host, 1000)
        .apply(&maker, [(VALUE, callback.clone())])
        .result;
    let built = build(&program, &host, 1000).unwrap();
    let positions = built.items.list_positions().unwrap();
    assert!(
        built
            .items
            .list_element(&positions[0])
            .unwrap()
            .same_result(&callback)
    );
    let nested = built.items.list_element(&positions[1]).unwrap();
    let child = nested.list_positions().unwrap().remove(0);
    assert!(nested.list_element(&child).unwrap().same_result(&callback));
    assert!(
        built
            .root
            .at(&[positions[0].clone()])
            .unwrap()
            .children
            .is_none()
    );
    assert!(
        built
            .root
            .at(&[positions[1].clone(), child])
            .unwrap()
            .children
            .is_none()
    );
    assert_eq!(
        positions,
        built
            .items
            .to_value()
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    );

    let collected = ::grap::Evaluator::new(&host, 1000)
        .apply(&Value::from(COLLECT).into(), [(PROGRAM, program)]);
    assert!(collected.halt.is_none() && collected.result.same_result(&built.items));
}

#[test]
fn memo_keeps_runtime_input_output_and_absence_origins_distinct() {
    let host = crate::stack::load().libraries;
    let doc = gid::Document::default();
    let computations = crate::computations::Computations::from_sources(crate::sources::Sources {
        doc: &doc,
        libraries: &host,
    });
    let callbacks =
        [A, B].map(|source| retained(::grap::lambda([], f64::value(7.0)), source, &host));
    assert_ne!(callbacks[0].to_value(), callbacks[1].to_value());
    assert!(!callbacks[0].same_result(&callbacks[1]));
    for fail in [false, true] {
        let maker = retained(
            ::grap::lambda(
                [VALUE],
                ::grap::lambda(
                    [],
                    if fail {
                        VALUE.into()
                    } else {
                        group(Value::list([leaf(VALUE.into())]))
                    },
                ),
            ),
            C,
            &host,
        );
        let payloads = callbacks.clone().map(|callback| {
            if fail {
                ::grap::absent::runtime(A, [(VALUE, callback)])
            } else {
                callback
            }
        });
        let programs = payloads.clone().map(|value| {
            ::grap::Evaluator::new(&host, 1000)
                .apply(&maker, [(VALUE, value)])
                .result
        });
        assert_ne!(programs[0].to_value(), programs[1].to_value());
        let first = prepared(&computations, &site(), programs[0].clone(), 1000);
        assert!(Rc::ptr_eq(
            &first,
            &prepared(&computations, &site(), programs[0].clone(), 1000)
        ));
        let second = prepared(&computations, &site(), programs[1].clone(), 1000);
        assert!(!Rc::ptr_eq(&first, &second));
        assert!(!same_output(&first, &second));
        for (result, expected) in [first, second].iter().zip(&payloads) {
            match result.as_ref() {
                Ok(built) => {
                    assert!(!fail && built.items.list_get(0).unwrap().same_result(expected))
                }
                Err(error) => assert!(fail && error.same_result(expected)),
            }
        }
    }
}
