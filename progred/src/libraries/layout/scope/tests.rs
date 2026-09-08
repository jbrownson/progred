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
        call(ALTERNATIVES, [(CHILDREN, sequence([]))]),
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
