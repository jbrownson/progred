use super::*;
use crate::display::recording::{Recordable, Recorded};

use crate::display::test_support::{NoEval, ProjectionCall, inspect};
use crate::libraries::name;
use crate::libraries::test_widgets::projection_input;
use gid::new_cell_id;
use std::sync::atomic::{AtomicUsize, Ordering};

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

fn binding(cell: CellId) -> Value {
    ::grap::isa::record(
        vocabulary::kind::BINDER,
        [(vocabulary::BIND, Value::from(cell))],
    )
}

fn case_arm(pattern: Value, expression: Value) -> Value {
    ::grap::isa::record(
        vocabulary::kind::CASE,
        [
            (vocabulary::PATTERN, pattern),
            (grap::vocabulary::EXPRESSION, expression),
        ],
    )
}

fn binding_clause(pattern: Value, value: Value) -> Value {
    ::grap::isa::record(
        vocabulary::kind::BINDING,
        [(vocabulary::PATTERN, pattern), (vocabulary::VALUE, value)],
    )
}

fn bind_clause(binder: CellId, value: Value) -> Value {
    ::grap::isa::record(
        vocabulary::kind::BINDING,
        [
            (vocabulary::BIND, Value::from(binder)),
            (vocabulary::VALUE, value),
        ],
    )
}

fn bindings_call(
    function: CellId,
    bindings: impl IntoIterator<Item = Value>,
    expression: Value,
) -> Value {
    grap::call(
        Value::from(function),
        [
            (vocabulary::BINDINGS, Value::list(bindings)),
            (grap::vocabulary::EXPRESSION, expression),
        ],
    )
}

fn match_call(value: Value, cases: impl IntoIterator<Item = Value>) -> Value {
    grap::call(
        Value::from(vocabulary::MATCH),
        [
            (vocabulary::VALUE, value),
            (vocabulary::CASES, Value::list(cases)),
        ],
    )
}

fn quote_call(expression: Value) -> Value {
    grap::call(
        Value::from(vocabulary::QUOTE),
        [(grap::vocabulary::EXPRESSION, expression)],
    )
}

fn do_call(expressions: impl IntoIterator<Item = Value>) -> Value {
    grap::call(
        Value::from(vocabulary::DO),
        [(vocabulary::EXPRESSIONS, Value::list(expressions))],
    )
}

fn all_call(expressions: impl IntoIterator<Item = Value>) -> Value {
    grap::call(
        Value::from(vocabulary::ALL),
        [(vocabulary::EXPRESSIONS, Value::list(expressions))],
    )
}

fn evaluate(expression: &Value) -> grap::Evaluation<gid::Value> {
    crate::test_support::test_evaluate(expression, |_| None, &functions(), 100)
}

fn evaluate_with_fuel(expression: &Value, fuel: usize) -> grap::Evaluation<gid::Value> {
    crate::test_support::test_evaluate(expression, |_| None, &functions(), fuel)
}

#[test]
fn do_evaluates_in_order_and_returns_the_last_result() {
    let first = new_cell_id();
    let second = new_cell_id();
    let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let first_calls = calls.clone();
    let second_calls = calls.clone();
    let functions = functions()
        .register(
            first,
            ForeignFunction::from_value(move |_, _, _| {
                first_calls.borrow_mut().push(first);
                Ok(blob("first"))
            }),
        )
        .register(
            second,
            ForeignFunction::from_value(move |_, _, _| {
                second_calls.borrow_mut().push(second);
                Ok(blob("second"))
            }),
        );
    let expression = do_call([
        grap::call(Value::from(first), []),
        grap::call(Value::from(second), []),
    ]);
    let result = crate::test_support::test_evaluate(&expression, |_| None, &functions, 100).result;

    assert_eq!(&*calls.borrow(), &[first, second]);
    assert_eq!(result, blob("second"));
    assert_eq!(
        evaluate(&do_call([])).result,
        ::grap::absent::value(vocabulary::MISSING_FINAL_EXPRESSION)
    );
}

#[test]
fn do_short_circuits_without_halting_or_rolling_back_effects() {
    let emit = new_cell_id();
    let log = Rc::new(std::cell::RefCell::new(Vec::new()));
    let output = log.clone();
    let functions = functions().register(
        emit,
        ForeignFunction::from_value(move |context, call, environment| {
            let value = context
                .eval_to_value(context.field(call, vocabulary::VALUE).unwrap(), environment)?;
            Ok(context.effect(|| {
                output.borrow_mut().push(value);
                Value::record([])
            }))
        }),
    );
    let effect = |text| grap::call(emit.into(), [(vocabulary::VALUE, blob(text))]);
    let run = |expression: &Value| {
        crate::test_support::test_evaluate(expression, |_| None, &functions, 100)
    };
    for reason in [new_cell_id(), grap::absent::FUEL_EXHAUSTED] {
        let failure = grap::absent::with_detail(reason, vocabulary::VALUE, blob("detail"));
        let expression = do_call([effect("before"), failure.clone(), effect("never")]);
        log.borrow_mut().clear();
        let result = run(&expression);
        assert!(result.completed);
        assert_eq!(result.result, failure);
        assert_eq!(*log.borrow(), [blob("before")]);

        let recovered = match_call(
            expression,
            [case_arm(
                ::grap::isa::record(
                    ::grap::vocabulary::kind::ABSENT,
                    [(grap::absent::REASON, reason.into())],
                ),
                effect("recovered"),
            )],
        );
        log.borrow_mut().clear();
        let result = run(&recovered);
        assert!(result.completed);
        assert_eq!(result.result, Value::record([]));
        assert_eq!(*log.borrow(), [blob("before"), blob("recovered")]);
    }
    // A pure decline is still an absent, even without any preceding effect.
    log.borrow_mut().clear();
    let result = run(&do_call([::grap::absent::decline(), effect("never")]));
    assert!(result.completed);
    assert_eq!(result.result, ::grap::absent::decline());
    assert!(log.borrow().is_empty());
}

#[test]
fn all_retains_absents_and_effects_and_continues_in_order() {
    let emit = new_cell_id();
    let log = Rc::new(std::cell::RefCell::new(Vec::new()));
    let output = log.clone();
    let functions = functions().register(
        emit,
        ForeignFunction::from_value(move |context, call, env| {
            let value =
                context.eval_to_value(context.field(call, vocabulary::VALUE).unwrap(), env)?;
            Ok(context.effect(|| {
                output.borrow_mut().push(value.clone());
                value
            }))
        }),
    );
    let before = Value::from(b"before".to_vec());
    let after = Value::from(b"after".to_vec());
    for reason in [new_cell_id(), ::grap::absent::FUEL_EXHAUSTED] {
        let failure = ::grap::absent::value(reason);
        let values = [before.clone(), failure, after.clone()];
        let expression = all_call(
            values
                .clone()
                .map(|value| ::grap::call(emit.into(), [(vocabulary::VALUE, value)])),
        );
        log.borrow_mut().clear();
        let result = crate::test_support::test_evaluate(&expression, |_| None, &functions, 100);
        assert!(result.completed);
        assert_eq!(result.result, Value::list(values.clone()));
        assert_eq!(*log.borrow(), values);
    }
}

#[test]
fn all_handles_empty_and_malformed_lists_but_does_not_swallow_halts() {
    assert_eq!(evaluate(&all_call([])).result, Value::list([]));
    let missing = evaluate(&::grap::call(vocabulary::ALL.into(), []));
    assert!(missing.completed);
    assert!(::grap::absent::reason(&missing.result).is_some());
    let invalid = ::grap::call(
        vocabulary::ALL.into(),
        [(vocabulary::EXPRESSIONS, Value::record([]))],
    );
    assert_eq!(
        evaluate(&invalid).result,
        ::grap::absent::value(vocabulary::INVALID_EXPRESSIONS)
    );
    let exhausted = evaluate_with_fuel(&all_call((0..100).map(|_| Value::record([]))), 5);
    assert!(!exhausted.completed);
    assert_eq!(
        ::grap::absent::reason(&exhausted.result),
        Some(::grap::absent::FUEL_EXHAUSTED)
    );
}

#[test]
fn ordinary_bindings_may_contain_absents_without_short_circuiting() {
    let binder = new_cell_id();
    let failure = ::grap::absent::value(new_cell_id());
    for function in [vocabulary::LET, vocabulary::WHERE] {
        let expression = bindings_call(
            function,
            [binding_clause(binding(binder), failure.clone())],
            blob("continues"),
        );
        assert_eq!(evaluate(&expression).result, blob("continues"));
    }
}

#[test]
fn do_projects_its_expression_list_despite_unrelated_fields() {
    let expression = do_call([blob("first"), blob("second")]);
    let Recorded::Row { children, .. } =
        do_display(&projection_input(&NoEval, &(&expression).into()))
            .expect("coherent do projection")
            .record()
            .content()
            .clone()
    else {
        panic!("do is its marker followed by a list")
    };
    assert_eq!(children.len(), 2);

    let decorated = expression
        .as_record()
        .unwrap()
        .with(new_cell_id(), blob("extra"));
    assert!(do_display(&projection_input(&NoEval, &Value::record(decorated).into())).is_some());
}

#[test]
fn compact_control_projections_recognize_required_fields_in_open_records() {
    use crate::display::{Pending, runtime_partial as partial};
    let binder = new_cell_id();
    for (projection, value) in [
        (partial(match_display), match_call(blob("subject"), [])),
        (partial(pattern_binder), binding(binder)),
        (
            partial(case_display),
            case_arm(blob("pattern"), blob("body")),
        ),
        (
            partial(bindings_display),
            bindings_call(vocabulary::LET, [], blob("body")),
        ),
        (
            partial(bindings_display),
            bindings_call(vocabulary::WHERE, [], blob("body")),
        ),
        (partial(binding_display), bind_clause(binder, blob("value"))),
        (
            partial(binding_display),
            binding_clause(blob("pattern"), blob("value")),
        ),
        (partial(do_display), do_call([blob("result")])),
    ] {
        let fields = value.as_record().unwrap();
        let decorated = Value::record(fields.with(new_cell_id(), blob("metadata")).with(
            name::vocabulary::NAME,
            crate::libraries::text::value("named"),
        ));
        assert!(projection(&projection_input(&NoEval, &(&decorated).into())).is_some());
        for key in fields.keys() {
            let missing = Value::record(decorated.as_record().unwrap().with(*key, None));
            assert!(projection(&projection_input(&NoEval, &(&missing).into())).is_none());
        }
        // A field being added joins the others after the form.
        let runtime = (&decorated).into();
        let mut input = projection_input(&NoEval, &runtime);
        input.pending = Some(Pending::Field);
        assert!(projection(&input).is_some());
    }
}

#[test]
fn compact_control_projections_still_decline_malformed_contents_and_conflicting_bindings() {
    use crate::display::runtime_partial as partial;
    for (projection, value) in [
        (
            partial(match_display),
            grap::call(
                vocabulary::MATCH.into(),
                [
                    (vocabulary::VALUE, blob("subject")),
                    (vocabulary::CASES, blob("not a list")),
                ],
            ),
        ),
        (
            partial(pattern_binder),
            ::grap::isa::record(
                vocabulary::kind::BINDER,
                [(vocabulary::BIND, blob("not a cell"))],
            ),
        ),
        (
            partial(binding_display),
            ::grap::isa::record(
                vocabulary::kind::BINDING,
                [
                    (vocabulary::BIND, new_cell_id().into()),
                    (vocabulary::PATTERN, blob("pattern")),
                    (vocabulary::VALUE, blob("value")),
                ],
            ),
        ),
        (
            partial(bindings_display),
            grap::call(
                vocabulary::LET.into(),
                [
                    (vocabulary::BINDINGS, blob("not a list")),
                    (grap::vocabulary::EXPRESSION, blob("body")),
                ],
            ),
        ),
        (
            partial(do_display),
            grap::call(
                vocabulary::DO.into(),
                [(vocabulary::EXPRESSIONS, blob("not a list"))],
            ),
        ),
    ] {
        assert!(projection(&projection_input(&NoEval, &(&value).into())).is_none());
    }
}

#[test]
fn quote_returns_recognized_expressions_as_data() {
    let function = new_cell_id();
    let expression = grap::call(Value::from(function), []);
    let evaluation = evaluate(&quote_call(expression.clone()));
    assert_eq!(evaluation.result, expression);
}

#[test]
fn quote_preserves_value_wrappers_and_splices_inside_them() {
    let expression = name::record(
        "named expression",
        [
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
            ),
            (grap::vocabulary::VALUE, new_cell_id().into()),
        ],
    );
    assert_eq!(evaluate(&quote_call(expression.clone())).result, expression);
    let template = name::record(
        "named expression",
        [
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
            ),
            (
                grap::vocabulary::VALUE,
                Value::record([
                    (
                        ::grap::isa::ISA,
                        gid::Value::from(vocabulary::kind::UNQUOTE).into(),
                    ),
                    (vocabulary::UNQUOTE, blob("spliced")),
                ]),
            ),
        ],
    );
    let result = evaluate(&quote_call(template)).result;
    assert_eq!(
        result,
        name::record(
            "named expression",
            [
                (
                    ::grap::isa::ISA,
                    gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into()
                ),
                (grap::vocabulary::VALUE, blob("spliced"))
            ]
        ),
    );
    assert_eq!(evaluate(&result).result, blob("spliced"));
}

#[test]
fn quote_projects_a_selectable_marker_and_its_expression() {
    let expression = blob("body");
    let quoted = quote_call(expression.clone());
    let layout = quote_display(&projection_input(&NoEval, &(&quoted).into())).unwrap();
    let Recorded::Row { children, .. } = &layout.record().content().clone() else {
        panic!("quote is an inline prefix");
    };
    let [marker, body] = children.as_slice() else {
        panic!("quote has a marker and expression");
    };
    let ProjectionCall::Descend {
        steps,
        projection: Some(_),
        ..
    } = &inspect(&(marker))
    else {
        panic!("the marker retains the function-field location");
    };
    assert_eq!(*steps, [Step::Key(grap::vocabulary::FUNCTION)]);

    let ProjectionCall::Descend { steps, .. } = &inspect(&(body)) else {
        panic!("the expression retains its field location");
    };
    assert_eq!(*steps, [Step::Key(grap::vocabulary::EXPRESSION)]);

    let marker = quote_marker(&projection_input(
        &NoEval,
        &Value::from(vocabulary::QUOTE).into(),
    ))
    .unwrap();
    let Recorded::Before { child, .. } = marker.record() else {
        panic!("the marker claims hover");
    };
    let Recorded::Before { child, .. } = *child else {
        panic!("the marker remains selectable");
    };
    let Recorded::Leaf(puri::draw::Leaf::Text {
        text, paint: face, ..
    }) = *child
    else {
        panic!("the marker is text");
    };
    assert_eq!(text, "\"");
    assert!(matches!(
        face,
        crate::display::Paint::Face(crate::display::Face::Dim)
    ));
}

#[test]
fn quote_prefixes_recognize_required_fields_despite_unrelated_metadata() {
    let extra = new_cell_id();
    let quote = grap::call(
        Value::from(vocabulary::QUOTE),
        [
            (grap::vocabulary::EXPRESSION, blob("body")),
            (extra, blob("visible")),
        ],
    );
    assert!(quote_display(&projection_input(&NoEval, &(&quote).into())).is_some());
    let unquote = name::record(
        "named unquote",
        [
            (
                ::grap::isa::ISA,
                gid::Value::from(vocabulary::kind::UNQUOTE).into(),
            ),
            (vocabulary::UNQUOTE, blob("body")),
            (extra, blob("metadata")),
        ],
    );
    assert!(unquote_display(&projection_input(&NoEval, &(&unquote).into())).is_some());
    for malformed in [
        Value::record([]),
        Value::record([
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::CALL).into(),
            ),
            (grap::vocabulary::FUNCTION, vocabulary::QUOTE.into()),
        ]),
        Value::record([(grap::vocabulary::EXPRESSION, blob("body"))]),
        grap::call(
            new_cell_id().into(),
            [(grap::vocabulary::EXPRESSION, blob("body"))],
        ),
    ] {
        assert!(quote_display(&projection_input(&NoEval, &(&malformed).into())).is_none());
        assert!(unquote_display(&projection_input(&NoEval, &(&malformed).into())).is_none());
    }
}

#[test]
fn quote_replaces_unquotes_in_its_calling_environment() {
    let parameter = new_cell_id();
    let field = new_cell_id();
    let template = Value::list([Value::record([(
        field,
        Value::record([
            (
                ::grap::isa::ISA,
                gid::Value::from(vocabulary::kind::UNQUOTE).into(),
            ),
            (vocabulary::UNQUOTE, Value::from(parameter)),
        ]),
    )])]);
    let positions = template
        .as_list()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let expression = grap::call(
        grap::lambda([parameter], quote_call(template)),
        [(parameter, blob("spliced"))],
    );
    let evaluation = evaluate(&expression);
    assert_eq!(
        evaluation.result,
        Value::list([Value::record([(field, blob("spliced"))])])
    );
    assert_eq!(
        evaluation
            .result
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        positions
    );
}

#[test]
fn unquote_is_ordinary_data_outside_quote() {
    let missing = new_cell_id();
    let expression = Value::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(vocabulary::kind::UNQUOTE).into(),
        ),
        (vocabulary::UNQUOTE, Value::from(missing)),
    ]);
    let evaluation = evaluate(&expression);
    assert_eq!(evaluation.result, expression);
}

#[test]
fn quote_does_not_revisit_an_unquoted_result() {
    let missing = new_cell_id();
    let inner = Value::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(vocabulary::kind::UNQUOTE).into(),
        ),
        (vocabulary::UNQUOTE, Value::from(missing)),
    ]);
    let expression = quote_call(Value::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(vocabulary::kind::UNQUOTE).into(),
        ),
        (vocabulary::UNQUOTE, inner.clone()),
    ]));
    let evaluation = evaluate(&expression);
    assert_eq!(evaluation.result, inner);
}

#[test]
fn match_evaluates_the_first_matching_case_with_open_record_bindings() {
    let first = new_cell_id();
    let last = new_cell_id();
    let metadata = new_cell_id();
    let name = new_cell_id();
    let never = new_cell_id();
    let expression = match_call(
        Value::record([
            (first, blob("Ada")),
            (last, blob("Lovelace")),
            (metadata, blob("extra")),
        ]),
        [
            case_arm(Value::record([(first, blob("Grace"))]), Value::from(never)),
            case_arm(Value::record([(first, binding(name))]), Value::from(name)),
        ],
    );
    let evaluation = evaluate(&expression);
    assert_eq!(evaluation.result, blob("Ada"));
}

#[test]
fn match_evaluates_its_subject_once_across_all_cases() {
    static SUBJECT_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

    let subject = new_cell_id();
    let foreign = functions().register(
        subject,
        ForeignFunction::from_value(|_, _, _| {
            SUBJECT_EVALUATIONS.fetch_add(1, Ordering::SeqCst);
            Ok(blob("subject"))
        }),
    );
    SUBJECT_EVALUATIONS.store(0, Ordering::SeqCst);

    let expression = match_call(
        grap::call(Value::from(subject), []),
        [
            case_arm(blob("first"), blob("first result")),
            case_arm(blob("second"), blob("second result")),
        ],
    );
    let evaluation = crate::test_support::test_evaluate(&expression, |_| None, &foreign, 100);
    assert_eq!(
        evaluation.result,
        ::grap::absent::from_causes([
            pattern_mismatch(&blob("first")),
            pattern_mismatch(&blob("second")),
        ]),
    );

    assert_eq!(SUBJECT_EVALUATIONS.load(Ordering::SeqCst), 1);
}

/// The cases are read, never evaluated: the call, its function, the
/// subject and the chosen arm burn.
#[test]
fn a_match_burns_its_subject_and_arm_but_not_its_cases() {
    let expression = match_call(blob("subject"), [case_arm(blob("subject"), blob("result"))]);

    let completed = evaluate_with_fuel(&expression, 5);
    assert_eq!(completed.result, blob("result"));
    assert_eq!(completed.remaining_fuel, 1);

    let exhausted = evaluate_with_fuel(&expression, 4);
    assert_eq!(
        exhausted.result,
        grap::absent::value(grap::absent::FUEL_EXHAUSTED),
    );
    assert_eq!(exhausted.remaining_fuel, 0);
}

#[test]
fn match_patterns_match_exact_lists_and_repeated_binders_must_agree() {
    let element = new_cell_id();
    let other = new_cell_id();
    let expression = match_call(
        Value::list([blob("left"), blob("right")]),
        [
            case_arm(
                Value::list([binding(element), binding(element)]),
                blob("repeated"),
            ),
            case_arm(
                Value::list([binding(element), binding(other)]),
                Value::from(other),
            ),
        ],
    );
    assert_eq!(evaluate(&expression).result, blob("right"));

    let too_short = match_call(
        Value::list([blob("only")]),
        [case_arm(
            Value::list([binding(element), binding(other)]),
            blob("matched"),
        )],
    );
    assert_eq!(
        evaluate(&too_short).result,
        pattern_mismatch(&Value::list([binding(element), binding(other),])),
    );
}

#[test]
fn first_pattern_wins_even_when_its_expression_returns_an_absent() {
    let missing = new_cell_id();
    let expression = match_call(
        Value::record([]),
        [
            case_arm(Value::record([]), Value::from(missing)),
            case_arm(Value::record([]), blob("second")),
        ],
    );
    assert_eq!(
        evaluate(&expression).result,
        grap::absent::with_detail(
            grap::absent::MISSING_CELL,
            grap::absent::CELL,
            missing.into()
        )
    );
}

#[test]
fn let_and_where_evaluate_bindings_sequentially() {
    let first = new_cell_id();
    let second = new_cell_id();
    let bindings = || {
        [
            bind_clause(first, blob("bound")),
            bind_clause(second, Value::from(first)),
        ]
    };
    for function in [vocabulary::LET, vocabulary::WHERE] {
        let expression = bindings_call(function, bindings(), Value::from(second));
        let evaluation = evaluate(&expression);
        assert_eq!(evaluation.result, blob("bound"));
    }
}

/// The bindings are read, never evaluated: the call, its function, each
/// binding's value and the body burn.
#[test]
fn a_let_burns_its_values_and_body_but_not_its_bindings() {
    let binder = new_cell_id();
    let expression = bindings_call(
        vocabulary::LET,
        [bind_clause(binder, blob("bound"))],
        Value::from(binder),
    );

    let completed = evaluate_with_fuel(&expression, 5);
    assert_eq!(completed.result, blob("bound"));
    assert_eq!(completed.remaining_fuel, 1);

    let exhausted = evaluate_with_fuel(&expression, 4);
    assert_eq!(
        exhausted.result,
        grap::absent::value(grap::absent::FUEL_EXHAUSTED),
    );
    assert_eq!(exhausted.remaining_fuel, 0);
}

#[test]
fn let_destructures_each_value_and_returns_an_absent_on_mismatch() {
    let field = new_cell_id();
    let binder = new_cell_id();
    let matched = bindings_call(
        vocabulary::LET,
        [binding_clause(
            Value::record([(field, binding(binder))]),
            Value::record([(field, blob("inside")), (new_cell_id(), blob("extra"))]),
        )],
        Value::from(binder),
    );
    assert_eq!(evaluate(&matched).result, blob("inside"));

    let unmatched = bindings_call(
        vocabulary::LET,
        [binding_clause(blob("expected"), blob("actual"))],
        blob("never"),
    );
    assert_eq!(
        evaluate(&unmatched).result,
        pattern_mismatch(&blob("expected"))
    );
}

#[test]
fn patterns_match_runtime_and_stored_subjects_alike() {
    let field = new_cell_id();
    let binder = new_cell_id();
    let subject_cell = new_cell_id();
    let subject = RuntimeValue::record([
        (
            field,
            RuntimeValue::list([RuntimeValue::f64(7.0), RuntimeValue::f64(7.0)]),
        ),
        (new_cell_id(), blob("extra").into()),
    ]);
    let pattern = Value::record([(field, Value::list([binding(binder), binding(binder)]))]);
    for subject in [subject.clone(), subject.to_value().into()] {
        let functions = functions().register(
            subject_cell,
            ForeignFunction::new(move |_, _, _| Ok(subject.clone())),
        );
        let subject = grap::call(Value::from(subject_cell), []);
        for function in [vocabulary::MATCH, vocabulary::LET, vocabulary::WHERE] {
            let clauses = Value::list([if function == vocabulary::MATCH {
                case_arm(pattern.clone(), Value::from(binder))
            } else {
                binding_clause(pattern.clone(), subject.clone())
            }]);
            let expression = grap::call(
                Value::from(function),
                if function == vocabulary::MATCH {
                    [
                        (vocabulary::VALUE, subject.clone()),
                        (vocabulary::CASES, clauses),
                    ]
                } else {
                    [
                        (vocabulary::BINDINGS, clauses),
                        (grap::vocabulary::EXPRESSION, Value::from(binder)),
                    ]
                },
            );
            let run =
                |fuel| crate::test_support::test_evaluate(&expression, |_| None, &functions, fuel);
            // Control call (2), subject call (2), body (1).
            let completed = run(6);
            assert_eq!(completed.result, crate::libraries::f64::value(7.0));
            assert_eq!(completed.remaining_fuel, 1);
            let exhausted = run(5);
            assert_eq!(
                exhausted.result,
                grap::absent::value(grap::absent::FUEL_EXHAUSTED),
            );
            assert_eq!(exhausted.remaining_fuel, 0);
        }
    }
}

#[test]
fn patterns_report_mismatches_and_binder_errors_lazily() {
    let binder = new_cell_id();
    let invalid = ::grap::isa::record(
        vocabulary::kind::BINDER,
        [(vocabulary::BIND, blob("not a cell"))],
    );
    let subject = Value::list([blob("first"), blob("second")]);
    for (pattern, reason) in [
        (Value::list([binding(binder), binding(binder)]), None),
        (Value::list([binding(binder)]), None),
        (Value::list([blob("other"), invalid.clone()]), None),
        (
            Value::list([blob("first"), invalid]),
            Some(vocabulary::INVALID_BINDER),
        ),
    ] {
        for function in [vocabulary::MATCH, vocabulary::LET, vocabulary::WHERE] {
            let expected = reason
                .map(::grap::absent::value)
                .unwrap_or_else(|| pattern_mismatch(&pattern));
            let clauses = Value::list([if function == vocabulary::MATCH {
                case_arm(pattern.clone(), blob("never"))
            } else {
                binding_clause(pattern.clone(), subject.clone())
            }]);
            let expression = grap::call(
                Value::from(function),
                if function == vocabulary::MATCH {
                    [
                        (vocabulary::VALUE, subject.clone()),
                        (vocabulary::CASES, clauses),
                    ]
                } else {
                    [
                        (vocabulary::BINDINGS, clauses),
                        (grap::vocabulary::EXPRESSION, blob("never")),
                    ]
                },
            );
            let result =
                crate::test_support::test_evaluate(&expression, |_| None, &functions(), 20);
            assert_eq!(result.result, expected);
            assert_eq!(result.remaining_fuel, 17);
        }
    }
}

#[test]
fn let_and_where_share_a_structure_but_project_in_opposite_orders() {
    let binder = new_cell_id();
    let clauses = || [bind_clause(binder, blob("value"))];
    let body = blob("body");
    let let_call = bindings_call(vocabulary::LET, clauses(), body.clone());
    let where_call = bindings_call(vocabulary::WHERE, clauses(), body.clone());

    let let_layout = bindings_display(&projection_input(&NoEval, &(&let_call).into())).unwrap();
    let Recorded::Alternatives(let_options) = let_layout.record().content().clone() else {
        panic!("let has responsive forms");
    };
    let Recorded::Row {
        children: let_children,
        ..
    } = let_options[0].content()
    else {
        panic!("flat let first");
    };
    assert_eq!(let_children.len(), 4);
    let Recorded::Shared { child, .. } = &let_children[2] else {
        panic!("let shares its in marker");
    };
    assert_eq!(
        crate::libraries::test_widgets::claim(child.as_ref()),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![Step::Key(grap::vocabulary::EXPRESSION)])
        ))
    );

    let Recorded::Shared {
        child: bindings, ..
    } = &let_children[1]
    else {
        panic!("let shares its bindings");
    };
    let ProjectionCall::Descend {
        steps,
        projection: Some(_),
        ..
    } = &inspect(&(bindings.as_ref()))
    else {
        panic!("let descends to its bindings list");
    };
    assert_eq!(*steps, [Step::Key(vocabulary::BINDINGS)]);

    let where_layout = bindings_display(&projection_input(&NoEval, &(&where_call).into())).unwrap();
    let Recorded::Alternatives(where_options) = where_layout.record().content().clone() else {
        panic!("where has responsive forms");
    };
    let Recorded::Row { children, .. } = where_options[0].content() else {
        panic!("flat where first");
    };
    let Recorded::Shared { child, .. } = children[0].content() else {
        panic!("where starts with its body");
    };
    assert!(matches!(&inspect(&(child.as_ref())),
        ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(grap::vocabulary::EXPRESSION)]
    ));
}

#[test]
fn direct_binding_is_deep_and_centered_beside_its_equals() {
    let binder = new_cell_id();
    let binding = bind_clause(binder, blob("value"));
    let layout = binding_display(&projection_input(&NoEval, &(&binding).into())).unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("a binding has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("a flat binding first");
    };
    let Recorded::Shared { child, .. } = children[0].content() else {
        panic!("a binding shares its pattern and equals");
    };
    let Recorded::Row {
        alignment: crate::display::RowAlignment::Center,
        children,
        ..
    } = child.content()
    else {
        panic!("a binding head is vertically centered");
    };
    assert!(matches!(&inspect(&(&children[0])),
        ProjectionCall::Descend {
            steps,
            projection: Some(_),
            ..
        } if *steps == [Step::Key(vocabulary::BIND)]
    ));
}

#[test]
fn match_augments_its_standard_cases_list_with_the_case_projection() {
    let binder = new_cell_id();
    let expression = match_call(
        blob("subject"),
        [
            case_arm(blob("first"), blob("one")),
            case_arm(binding(binder), Value::from(binder)),
        ],
    );
    let layout = match_display(&projection_input(&NoEval, &(&expression).into())).unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("match has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("flat match first");
    };
    let mut arms = &children[1];
    while let Recorded::Shared { child, .. } = arms {
        arms = child.as_ref();
    }
    let ProjectionCall::Descend {
        steps,
        projection: Some(_),
        ..
    } = &inspect(&(arms))
    else {
        panic!("match descends to its cases list");
    };
    assert_eq!(*steps, [Step::Key(vocabulary::CASES)]);
    let cases = expression
        .as_record()
        .unwrap()
        .get(&vocabulary::CASES)
        .unwrap();

    assert!(case_display(&projection_input(&NoEval, &cases.into())).is_none());
    let case = cases
        .as_list()
        .and_then(|cases| cases.values().next())
        .expect("the standard list contains its cases");
    let case = case_display(&projection_input(&NoEval, &case.into())).unwrap();
    let Recorded::Alternatives(options) = case.record().content().clone() else {
        panic!("a case has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("a flat case is a row");
    };
    let Recorded::Shared { child, .. } = children[0].content() else {
        panic!("a case shares its head");
    };
    let Recorded::Row { children, .. } = child.content() else {
        panic!("a case head contains its pattern and arrow");
    };
    let Recorded::Before { child, .. } = children[1].content() else {
        panic!("the arrow claims the case hover");
    };
    assert_eq!(
        crate::libraries::test_widgets::claim(&children[1]),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![Step::Key(grap::vocabulary::EXPRESSION)])
        ))
    );
    assert!(matches!(child.as_ref(), Recorded::Before { .. }));
}

#[test]
fn non_case_elements_use_the_standard_projection_inside_match() {
    let malformed = match_call(blob("subject"), [blob("not a case")]);
    assert!(match_display(&projection_input(&NoEval, &(&malformed).into())).is_some());
    let case = malformed
        .as_record()
        .and_then(|fields| fields.get(&vocabulary::CASES))
        .and_then(Value::as_list)
        .and_then(|cases| cases.values().next())
        .unwrap();
    assert!(case_display(&projection_input(&NoEval, &case.into())).is_none());
}

#[test]
fn malformed_match_data_returns_library_absents() {
    let invalid_cases = grap::call(
        Value::from(vocabulary::MATCH),
        [
            (vocabulary::VALUE, blob("subject")),
            (vocabulary::CASES, blob("not a list")),
        ],
    );
    assert_eq!(
        evaluate(&invalid_cases).result,
        ::grap::absent::value(vocabulary::INVALID_CASES)
    );

    let invalid_case = match_call(blob("subject"), [blob("not a case")]);
    assert_eq!(
        evaluate(&invalid_case).result,
        ::grap::absent::value(vocabulary::INVALID_CASE)
    );

    let invalid_binder = match_call(
        blob("subject"),
        [case_arm(
            ::grap::isa::record(
                vocabulary::kind::BINDER,
                [(vocabulary::BIND, blob("not a cell"))],
            ),
            blob("matched"),
        )],
    );
    assert_eq!(
        evaluate(&invalid_binder).result,
        ::grap::absent::value(vocabulary::INVALID_BINDER)
    );
}

#[test]
fn malformed_let_data_returns_library_absents() {
    let invalid_bindings = grap::call(
        Value::from(vocabulary::LET),
        [
            (vocabulary::BINDINGS, blob("not a list")),
            (grap::vocabulary::EXPRESSION, blob("body")),
        ],
    );
    assert_eq!(
        evaluate(&invalid_bindings).result,
        ::grap::absent::value(vocabulary::INVALID_BINDINGS)
    );

    let invalid_binding = bindings_call(vocabulary::LET, [blob("not a binding")], blob("body"));
    assert_eq!(
        evaluate(&invalid_binding).result,
        ::grap::absent::value(vocabulary::INVALID_BINDING)
    );

    let ambiguous = bindings_call(
        vocabulary::LET,
        [::grap::isa::record(
            vocabulary::kind::BINDING,
            [
                (vocabulary::BIND, Value::from(new_cell_id())),
                (vocabulary::PATTERN, blob("pattern")),
                (vocabulary::VALUE, blob("value")),
            ],
        )],
        blob("body"),
    );
    assert_eq!(
        evaluate(&ambiguous).result,
        ::grap::absent::value(vocabulary::INVALID_BINDING)
    );

    let invalid_binder = bindings_call(
        vocabulary::LET,
        [::grap::isa::record(
            vocabulary::kind::BINDING,
            [
                (vocabulary::BIND, blob("not a cell")),
                (vocabulary::VALUE, blob("value")),
            ],
        )],
        blob("body"),
    );
    assert_eq!(
        evaluate(&invalid_binder).result,
        ::grap::absent::value(vocabulary::INVALID_BINDER)
    );
}

#[test]
fn quote_and_unquote_are_offered_under_their_marks() {
    use crate::driver::Driver;
    use crate::libraries::grap::vocabulary::GRAP;
    use crate::selection::Stage;
    use ::grap::vocabulary::{BODY, EXPRESSION};
    use gid::Step;
    use ui_events::keyboard::NamedKey;

    let mut driver = Driver::of(Value::record([(GRAP, new_cell_id().into())]));
    let program = vec![Step::Key(GRAP), Step::Follow];
    driver.select(&program);
    driver.type_text("λ");
    driver.choose("new lambda");
    driver.select(&[&program[..], &[Step::Key(BODY)]].concat());
    let offered = |driver: &mut Driver| {
        driver
            .picker_entries()
            .expect("the pending shows its picker")
    };

    driver.type_text("\"hi");
    assert!(
        !offered(&mut driver).contains(&"quote".to_owned()),
        "text after the mark is a string"
    );
    driver.press(NamedKey::Backspace);
    driver.press(NamedKey::Backspace);
    assert!(
        !offered(&mut driver).contains(&"unquote".to_owned()),
        "outside a quote there is nothing to unquote"
    );
    assert_eq!(offered(&mut driver)[0], "quote");
    driver.press(NamedKey::Enter);
    let body = [&program[..], &[Step::Key(BODY)]].concat();
    assert_eq!(
        driver.value(&body),
        ::grap::call(vocabulary::QUOTE.into(), [])
    );
    let expression = [&body[..], &[Step::Key(EXPRESSION)]].concat();
    assert_eq!(driver.selection(), expression);
    assert_eq!(driver.stage(), Some(Stage::Pending));

    driver.type_text("`");
    assert_eq!(offered(&mut driver)[0], "unquote");
    driver.press(NamedKey::Enter);
    assert_eq!(
        driver.value(&expression),
        ::grap::isa::record(vocabulary::kind::UNQUOTE, []),
        "an unquote while its expression is yet to be written"
    );
    let hole = [&expression[..], &[Step::Key(vocabulary::UNQUOTE)]].concat();
    assert_eq!(driver.selection(), hole);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    driver.type_text("\"");
    assert_eq!(offered(&mut driver)[0], "quote", "the hole is code again");
}
