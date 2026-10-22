use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::libraries::absent;
use crate::libraries::grap::control::vocabulary as control;
use crate::projection::Layout;
use crate::test_values::call;

fn sequence(expressions: impl IntoIterator<Item = Value>) -> Value {
    call(
        control::DO,
        [(control::EXPRESSIONS, Value::list(expressions))],
    )
}

fn quote(value: Value) -> Value {
    call(control::QUOTE, [(::grap::vocabulary::EXPRESSION, value)])
}

fn site() -> crate::editing::Site {
    crate::libraries::test_widgets::test_site()
}

#[test]
fn layout_results_and_nested_absent_details_keep_native_callbacks() {
    let grap = crate::libraries::grap::library::<crate::Editor>();
    let host = crate::test_support::TestHost(|cell| {
        grap.definitions
            .get(cell)
            .cloned()
            .map(|definition| (gid::Resolution::Document, definition))
    });
    let callback = ::grap::evaluate_runtime_at(
        &::grap::RuntimeValue::from(::grap::lambda([], Value::record([]))),
        Some(::grap::SourceOrigin::Stored(vec![Step::Key(
            gid::new_cell_id(),
        )])),
        &host,
        100,
    )
    .result;
    let returned = RuntimeValue::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
        ),
        (::grap::vocabulary::VALUE, callback.clone()),
    ]);
    let (evaluation, layout) = run::<crate::Editor>(site, |scope| {
        ::grap::evaluate_runtime_scoped(&returned, &host, scope, 100)
    });
    assert!(evaluation.completed);
    assert!(evaluation.result.same_result(&callback));
    assert!(layout.is_none());

    let failure = RuntimeValue::record([
        (::grap::absent::ABSENT, Value::from(INVALID_PROGRAM).into()),
        (VALUE, callback.clone()),
    ]);
    let program = RuntimeValue::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(::grap::vocabulary::kind::CALL).into(),
        ),
        (::grap::vocabulary::FUNCTION, Value::from(ROW).into()),
        (
            CHILDREN,
            RuntimeValue::record([
                (
                    ::grap::isa::ISA,
                    gid::Value::from(::grap::vocabulary::kind::CALL).into(),
                ),
                (
                    ::grap::vocabulary::FUNCTION,
                    Value::from(control::DO).into(),
                ),
                (
                    control::EXPRESSIONS,
                    RuntimeValue::list([call(SLOT, []).into(), failure.clone()]),
                ),
            ]),
        ),
    ]);
    let (evaluation, layout) = run::<crate::Editor>(site, |scope| {
        ::grap::evaluate_runtime_scoped(&program, &host, scope, 1000)
    });
    assert!(evaluation.completed);
    assert!(evaluation.result.same_result(&failure));
    assert!(
        evaluation
            .result
            .field(VALUE)
            .unwrap()
            .same_result(&callback)
    );
    assert!(
        layout.is_none(),
        "failed child discards the partially emitted row"
    );
}

fn text(value: &str) -> Value {
    call(TEXT, [(CONTENT, crate::libraries::text::value(value))])
}

fn evaluate(program: &Value, fuel: usize) -> (Evaluation, Option<Layout>) {
    let libraries = [
        crate::libraries::grap::library::<crate::Editor>(),
        super::super::library::<crate::Editor>(),
    ];
    let host = crate::test_support::TestHost(|cell| {
        libraries.iter().find_map(|library| {
            library
                .definitions
                .get(cell)
                .cloned()
                .map(|definition| (gid::Resolution::Document, definition))
        })
    });
    run::<crate::Editor>(site, |scope| {
        ::grap::evaluate_runtime_scoped(&::grap::RuntimeValue::from(program), &host, scope, fuel)
    })
}

#[test]
fn scoped_layout_calls_compose_through_grap_functions_and_nested_bodies() {
    let parameter = gid::new_cell_id();
    let child = ::grap::lambda([parameter], call(TEXT, [(CONTENT, parameter.into())]));
    let program = call(
        ROW,
        [
            (GAP, f64_convention::value(3.0)),
            (
                CHILDREN,
                sequence([
                    text("before"),
                    call(
                        COL,
                        [(
                            CHILDREN,
                            sequence([
                                ::grap::call(
                                    child,
                                    [(parameter, crate::libraries::text::value("inside"))],
                                ),
                                text("below"),
                            ]),
                        )],
                    ),
                    text("after"),
                ]),
            ),
        ],
    );
    let (result, layout) = evaluate(&program, 10000);
    assert!(result.completed);
    assert_eq!(result.result.to_value(), Value::record([]));
    let Recorded::Row { gap, children, .. } = layout.unwrap().record() else {
        panic!("row")
    };
    assert_eq!(gap, 3.0);
    assert_eq!(children.len(), 3);
    assert!(matches!(&children[1], Recorded::Col { children, .. } if children.len() == 2));
    assert!(matches!(&children[2], Recorded::Leaf(Leaf::Text { text, .. }) if text == "after"));
}

#[test]
fn grap_recursion_calls_emit_the_same_native_location_operations() {
    use crate::display::test_support::{ProjectionCall, inspect};
    let steps = vec![
        gid::Step::Key(gid::new_cell_id()),
        gid::Step::Key(gid::new_cell_id()),
    ];
    let document = vec![gid::Step::Key(gid::new_cell_id())];
    let encoded = crate::libraries::editor::path::value(&steps);
    let (result, layout) = evaluate(&call(DESCEND_PATH, [(STEPS, quote(encoded.clone()))]), 1000);
    assert!(result.completed);
    assert!(
        matches!(inspect(&layout.unwrap()), ProjectionCall::Descend { steps: actual, .. } if actual == steps)
    );
    let (result, layout) = evaluate(
        &call(
            JUMP,
            [
                (STEPS, quote(encoded.clone())),
                (
                    DOCUMENT_PATH,
                    quote(crate::libraries::editor::path::value(&document)),
                ),
            ],
        ),
        1000,
    );
    assert!(result.completed);
    assert!(
        matches!(inspect(&layout.unwrap()), ProjectionCall::Jump { steps: actual, document: target, .. } if actual == steps && target == crate::location::Location::root(document.as_slice()))
    );
    let supplied = crate::libraries::text::value("computed");
    let (result, layout) = evaluate(
        &call(
            AT,
            [(STEPS, quote(encoded)), (VALUE, quote(supplied.clone()))],
        ),
        1000,
    );
    assert!(result.completed);
    assert!(
        matches!(inspect(&layout.unwrap()), ProjectionCall::At { steps: actual, value, .. } if actual == steps && value == supplied)
    );
}

#[test]
fn a_row_given_a_baseline_aligns_tops_as_a_column_does() {
    use crate::display::RowAlignment;
    for (baseline, expected) in [
        (None, RowAlignment::Baseline),
        (Some(1.0), RowAlignment::Top { baseline: 1 }),
    ] {
        let program = call(
            ROW,
            baseline
                .map(|baseline| (BASELINE, f64_convention::value(baseline)))
                .into_iter()
                .chain([(CHILDREN, sequence([text("left"), text("right")]))]),
        );
        let (result, layout) = evaluate(&program, 10000);
        assert!(result.completed);
        let Recorded::Row { alignment, .. } = layout.unwrap().record() else {
            panic!("row")
        };
        assert!(alignment == expected);
    }
}

#[test]
fn invalid_calls_and_fuel_exhaustion_discard_every_emission() {
    let programs = [
        sequence([text("valid"), call(TEXT, [])]),
        call(ROW, [(CHILDREN, sequence([call(TEXT, []), text("valid")]))]),
        sequence([text("two"), text("roots")]),
        call(
            COL,
            [
                (BASELINE, f64_convention::value(10.0)),
                (CHILDREN, text("one")),
            ],
        ),
        call(
            ROW,
            [
                (BASELINE, f64_convention::value(2.0)),
                (CHILDREN, sequence([text("two"), text("children")])),
            ],
        ),
        call(PAD, [(CHILD, sequence([text("two"), text("children")]))]),
        call(ALTERNATIVES, [(CHILDREN, Value::record([]))]),
    ];
    for program in programs {
        let (result, layout) = evaluate(&program, 10000);
        assert_eq!(
            ::grap::absent::reason(&result.result.to_value()),
            Some(INVALID_PROGRAM)
        );
        assert!(layout.is_none());
    }
    let (result, layout) = evaluate(&sequence([text("one"), text("two")]), 1);
    assert!(!result.completed);
    assert!(layout.is_none());
    assert!(evaluate(&text("fresh evaluation"), 10000).1.is_some());
}

#[test]
fn an_invalid_call_names_its_function() {
    for (program, function) in [
        (call(TEXT, []), TEXT),
        (
            call(ALTERNATIVES, [(CHILDREN, Value::record([]))]),
            ALTERNATIVES,
        ),
        (
            call(PAD, [(CHILD, sequence([text("two"), text("children")]))]),
            PAD,
        ),
    ] {
        let (result, layout) = evaluate(&program, 10000);
        assert_eq!(
            result.result.to_value(),
            ::grap::absent::with_detail(
                INVALID_PROGRAM,
                ::grap::vocabulary::FUNCTION,
                function.into()
            )
        );
        assert!(layout.is_none());
    }
}

#[test]
fn nested_failures_propagate_and_recovery_is_not_overridden_by_the_scope() {
    let reason = gid::new_cell_id();
    let failure =
        ::grap::absent::with_detail(reason, VALUE, crate::libraries::text::value("detail"));
    let failed = call(
        ROW,
        [(
            CHILDREN,
            sequence([text("discarded child"), failure.clone(), text("never")]),
        )],
    );
    let (evaluation, layout) = evaluate(&failed, 1000);
    assert!(evaluation.completed);
    assert_eq!(evaluation.result.to_value(), failure);
    assert!(layout.is_none());
    let recovered = call(
        control::MATCH,
        [
            (control::VALUE, failed),
            (
                control::CASES,
                Value::list([Value::record([
                    (
                        control::PATTERN,
                        Value::record([(absent::vocabulary::ABSENT, reason.into())]),
                    ),
                    (::grap::vocabulary::EXPRESSION, text("recovered")),
                ])]),
            ),
        ],
    );
    let (evaluation, layout) = evaluate(&recovered, 1000);
    assert!(evaluation.completed);
    assert_eq!(evaluation.result.to_value(), Value::record([]));
    assert!(
        matches!(layout.unwrap().record(), Recorded::Leaf(Leaf::Text { text, .. }) if text == "recovered")
    );

    let empty = call(ROW, [(CHILDREN, sequence([]))]);
    let (evaluation, layout) = evaluate(&empty, 1000);
    assert_eq!(
        evaluation.result.to_value(),
        ::grap::absent::value(control::MISSING_FINAL_EXPRESSION)
    );
    assert!(layout.is_none());
}

#[test]
fn scope_does_not_reinterpret_plain_values_or_escape_into_returned_closures() {
    let value = super::super::test_support::row(
        2.0,
        [super::super::test_support::text_leaf("data", NAME_FACE)],
    );
    let (result, layout) = evaluate(&quote(value.clone()), 10000);
    assert_eq!(result.result.to_value(), value);
    assert!(layout.is_none());
    let (result, layout) = evaluate(&::grap::lambda([], text("later")), 10000);
    assert!(layout.is_none());
    let unscoped = ::grap::apply(
        &result.result,
        [],
        &crate::test_support::TestHost(|_| None),
        None,
        10000,
    );
    assert!(unscoped.result.absent_reason().is_some());
}

#[test]
fn layout_program_is_an_ordinary_closure_with_its_lexical_environment() {
    let parameter = gid::new_cell_id();
    let factory = ::grap::lambda(
        [parameter],
        call(
            LAYOUT_PROGRAM,
            [(
                ::grap::vocabulary::EXPRESSION,
                call(TEXT, [(CONTENT, parameter.into())]),
            )],
        ),
    );
    let (result, layout) = evaluate(
        &::grap::call(
            factory,
            [(parameter, crate::libraries::text::value("captured"))],
        ),
        10000,
    );
    assert!(result.completed);
    assert!(layout.is_none());
    let function = result.result.field(LAYOUT_PROGRAM).unwrap();
    assert!(function.contains_field(::grap::vocabulary::LAMBDA));
    let (result, layout) = run::<crate::Editor>(site, |scope| {
        ::grap::apply(
            &function,
            [],
            &crate::test_support::TestHost(|_| None),
            Some(scope),
            10000,
        )
    });
    assert!(result.completed);
    assert!(
        matches!(layout.unwrap().record(), Recorded::Leaf(Leaf::Text { text, .. }) if text == "captured")
    );
}

#[test]
fn border_and_drawing_emit_in_a_program_as_their_stored_nodes_decode() {
    let program = call(
        COL,
        [(
            CHILDREN,
            sequence([
                call(BORDER, [(CHILD, text("framed"))]),
                call(
                    DRAWING,
                    [
                        (WIDTH, f64_convention::value(4.0)),
                        (DESCENT, f64_convention::value(2.0)),
                        (PROGRAM, quote(Value::record([]))),
                    ],
                ),
            ]),
        )],
    );
    let (result, layout) = evaluate(&program, 10000);
    assert!(result.completed);
    let Recorded::Col { children, .. } = layout.unwrap().record() else {
        panic!("col")
    };
    assert_eq!(children.len(), 2);
}
