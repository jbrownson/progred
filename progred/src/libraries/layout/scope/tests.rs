use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::libraries::control::vocabulary as control;
use std::rc::Rc;

fn call(function: CellId, args: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::call(function.into(), args)
}

fn sequence(expressions: impl IntoIterator<Item = Value>) -> Value {
    call(
        control::DO,
        [(control::EXPRESSIONS, Value::list(expressions))],
    )
}

fn quote(value: Value) -> Value {
    call(control::QUOTE, [(::grap::vocabulary::EXPRESSION, value)])
}

fn text(value: &str) -> Value {
    call(TEXT, [(CONTENT, crate::libraries::text::value(value))])
}

fn evaluate(
    program: &Value,
    fuel: usize,
) -> (
    Evaluation,
    Option<Layout<crate::Editor, crate::frame::Hovered>>,
) {
    let libraries = [
        crate::libraries::control::library(),
        super::super::library(),
    ];
    let host = crate::libraries::TestHost(|cell| {
        libraries
            .iter()
            .filter_map(|library| {
                library
                    .definitions
                    .get(cell)
                    .cloned()
                    .map(|definition| (gid::Resolution::Document, definition))
            })
            .collect()
    });
    run(
        || ProjectionTarget {
            select: Rc::new(|_| true),
            select_with: Rc::new(|_, _| true),
            hover: crate::libraries::test_widgets::hover(vec![]),
        },
        |scope| ::grap::evaluate_scoped(program, &host, scope, fuel),
    )
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
    assert_eq!(result.result, Value::record([]));
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
    let encoded = crate::libraries::path::value(&steps);
    let (result, layout) = evaluate(&call(DESCEND_PATH, [(STEPS, quote(encoded.clone()))]), 1000);
    assert!(result.completed);
    assert!(
        matches!(inspect(&layout.unwrap()), ProjectionCall::DescendPath { steps: actual, .. } if actual == steps)
    );
    let (result, layout) = evaluate(
        &call(
            JUMP,
            [
                (STEPS, quote(encoded.clone())),
                (
                    DOCUMENT_PATH,
                    quote(crate::libraries::path::value(&document)),
                ),
            ],
        ),
        1000,
    );
    assert!(result.completed);
    assert!(
        matches!(inspect(&layout.unwrap()), ProjectionCall::Jump { steps: actual, document: target, .. } if actual == steps && target == document)
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
        call(PAD, [(CHILD, sequence([text("two"), text("children")]))]),
        call(ALTERNATIVES, [(CHILDREN, Value::record([]))]),
    ];
    for program in programs {
        let (result, layout) = evaluate(&program, 10000);
        assert_eq!(absent::reason(&result.result), Some(INVALID_PROGRAM));
        assert!(layout.is_none());
    }
    let (result, layout) = evaluate(&sequence([text("one"), text("two")]), 1);
    assert!(!result.completed);
    assert!(layout.is_none());
    assert!(evaluate(&text("fresh evaluation"), 10000).1.is_some());
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
    assert_eq!(evaluation.result, failure);
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
    assert_eq!(evaluation.result, Value::record([]));
    assert!(
        matches!(layout.unwrap().record(), Recorded::Leaf(Leaf::Text { text, .. }) if text == "recovered")
    );

    let empty = call(ROW, [(CHILDREN, sequence([]))]);
    let (evaluation, layout) = evaluate(&empty, 1000);
    assert_eq!(
        evaluation.result,
        absent::with_reason(control::MISSING_FINAL_EXPRESSION)
    );
    assert!(layout.is_none());
}

#[test]
fn scope_does_not_reinterpret_plain_values_or_escape_into_returned_closures() {
    let value = super::super::row(2.0, [super::super::text_leaf("data", NAME_FACE)]);
    let (result, layout) = evaluate(&quote(value.clone()), 10000);
    assert_eq!(result.result, value);
    assert!(layout.is_none());
    let (result, layout) = evaluate(&::grap::lambda([], text("later")), 10000);
    assert!(layout.is_none());
    let unscoped = ::grap::apply(
        &result.result,
        [],
        &crate::libraries::TestHost(|_| vec![]),
        10000,
    );
    assert!(absent::is_absent(&unscoped.result));
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
    let function = result
        .result
        .as_record()
        .unwrap()
        .get(&LAYOUT_PROGRAM)
        .unwrap();
    assert!(
        function
            .as_record()
            .unwrap()
            .get(&::grap::vocabulary::CLOSURE)
            .is_some()
    );
    let (result, layout) = evaluate(&::grap::call(function.clone(), []), 10000);
    assert!(result.completed);
    assert!(
        matches!(layout.unwrap().record(), Recorded::Leaf(Leaf::Text { text, .. }) if text == "captured")
    );
}
