use super::*;
use crate::view::{Layout, ProjectionInput};

fn field(input: &ProjectionInput<'_>) -> Option<Layout> {
    project_runtime(super::field, input)
}
use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::{ProjectionCall, inspect, unshared};
use crate::display::{Env, Pending};
use crate::libraries::fidget::{binary, node, unary};
use crate::libraries::test_widgets::{project_runtime, projection_input};

struct Names;

impl Env for Names {
    fn apply_scoped(
        &self,
        _: &Value,
        _: &[(CellId, Value)],
        _scope: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation<gid::Value> {
        panic!("source projection must not evaluate")
    }

    fn evaluate(&self, _: &Value) -> Value {
        panic!("source projection must not evaluate")
    }

    fn name(&self, _: CellId) -> Option<&str> {
        Some("renamed operator")
    }
}

fn number(n: f32) -> Value {
    crate::libraries::f32::value(n)
}

#[test]
fn operands_keep_their_paths_and_operator_targets_the_expression() {
    let sum = binary(SUM, number(1.0), number(2.0));
    let Recorded::Alternatives(options) = field(&projection_input(&Names, &sum))
        .unwrap()
        .record()
        .content()
        .clone()
    else {
        panic!()
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!()
    };
    for (child, key) in [(&children[0], LEFT), (&children[2], RIGHT)] {
        assert!(
            matches!(&inspect(&(unshared(child))), ProjectionCall::DescendPath { steps, .. }
            if *steps == [Step::Key(SUM), Step::Key(key)])
        );
    }
    assert_eq!(
        crate::libraries::test_widgets::claim(unshared(&children[1])),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![])
        ))
    );
}

#[test]
fn grouping_preserves_the_expression_tree_without_reassociating() {
    let sum = binary(SUM, number(1.0), number(2.0));
    let product = binary(MULTIPLY, number(1.0), number(2.0));
    for (parent, key, child, grouped) in [
        (MULTIPLY, LEFT, &sum, true),
        (SUM, LEFT, &product, false),
        (SUM, LEFT, &sum, false),
        (SUM, RIGHT, &sum, true),
        (DIVIDE, RIGHT, &product, true),
    ] {
        assert_eq!(
            matches!(
                operand::<crate::Editor>(parent, key, child).record(),
                Recorded::Row { .. }
            ),
            grouped
        );
    }
}

#[test]
fn unrelated_fields_do_not_block_notation_or_change_grouping() {
    let extra = gid::new_cell_id();
    for value in [
        Value::record([
            (
                SUM,
                Value::record([(LEFT, number(1.0)), (RIGHT, number(2.0))]),
            ),
            (extra, number(3.0)),
        ]),
        node(
            SUM,
            Value::record([
                (LEFT, number(1.0)),
                (RIGHT, number(2.0)),
                (extra, number(3.0)),
            ]),
        ),
    ] {
        assert!(field(&projection_input(&Names, &value)).is_some());
        for (parent, key, grouped) in [
            (MULTIPLY, LEFT, true),
            (SUM, LEFT, false),
            (SUM, RIGHT, true),
        ] {
            assert_eq!(
                matches!(
                    operand::<crate::Editor>(parent, key, &value).record(),
                    Recorded::Row { .. }
                ),
                grouped,
            );
        }
    }
    for value in [unary(SIN, number(1.0)), node(AXIS, X.into())] {
        let enriched = Value::record(value.as_record().unwrap().update(extra, number(3.0)));
        assert!(field(&projection_input(&Names, &enriched)).is_some());
    }
}

#[test]
fn incomplete_malformed_and_conflicting_forms_still_decline() {
    for value in [
        node(SUM, Value::record([(LEFT, number(1.0))])),
        node(SUM, Value::record([(RIGHT, number(1.0))])),
        node(SIN, number(1.0)),
        node(AXIS, number(1.0)),
        Value::record([
            (AXIS, X.into()),
            (SIN, Value::record([(OPERAND, number(1.0))])),
        ]),
    ] {
        assert!(field(&projection_input(&Names, &value)).is_none());
    }
    let complete = unary(SIN, number(1.0));
    assert!(
        field(&ProjectionInput {
            default_projection: crate::display::runtime_partial(|_| None),
            pending: Some(Pending::Field),
            ..projection_input(&Names, &complete)
        })
        .is_none()
    );
}

#[test]
fn coordinates_are_shallow_and_names_remain_editable_data() {
    let axis = node(AXIS, X.into());
    assert!(
        matches!(inspect(&field(&projection_input(&Names, &axis)).unwrap()), ProjectionCall::Descend { step, projection: Some(_), .. }
        if step == Step::Key(AXIS))
    );
    let named = name::record("torus", [(SQUARE, Value::record([(OPERAND, number(1.0))]))]);
    let Recorded::Alternatives(options) = field(&projection_input(&Names, &named))
        .unwrap()
        .record()
        .content()
        .clone()
    else {
        panic!()
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!()
    };
    let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
        panic!()
    };
    assert!(
        matches!(&inspect(&(&head[0])), ProjectionCall::Descend { step, .. } if *step == Step::Key(name::vocabulary::NAME))
    );
}

#[test]
fn arithmetic_calls_read_infix_and_named_ones_stay_calls() {
    let library = crate::libraries::fidget::library();
    let reads_infix = |function: CellId| {
        let call = ::grap::call(
            Value::from(function),
            [(LEFT, number(1.0)), (RIGHT, number(2.0))],
        );
        let runtime = ::grap::RuntimeValue::from(&call);
        (library.projection.as_ref().unwrap())(
            &projection_input(&Names, &call).with_value(Some(&runtime)),
        )
        .is_some_and(|layout| {
            let Recorded::Alternatives(options) = layout.record().content().clone() else {
                return false;
            };
            let Recorded::Row { children, .. } = options[0].content() else {
                return false;
            };
            matches!(&inspect(&(unshared(&children[0]))),
                ProjectionCall::Descend { step: Step::Key(field), .. } if *field == LEFT
            )
        })
    };
    for function in [SUM, SUBTRACT, MULTIPLY, DIVIDE] {
        assert!(reads_infix(function), "{function:?} reads infix");
    }
    assert!(!reads_infix(MAX));
}

mod frames;
