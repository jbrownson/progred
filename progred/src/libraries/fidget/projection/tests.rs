use super::*;

fn field(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let value = input.value.map(::grap::RuntimeValue::from);
    super::field(&input.with_value(value.as_ref()))
}
use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::{ProjectionCall, inspect};
use crate::display::{Env, Pending, ProjectionTarget, ProjectionTargets};
use crate::libraries::fidget::{binary, node, unary};
use std::rc::Rc;

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

fn target(path: Vec<Step>) -> ProjectionTarget<crate::Editor, crate::frame::Hovered> {
    ProjectionTarget {
        hover: crate::libraries::test_widgets::hover(path),
        select: Rc::new(|_| true),
        select_with: Rc::new(|_, _| true),
    }
}

fn input(value: &Value) -> ProjectionInput<'_, crate::Editor, crate::frame::Hovered> {
    ProjectionInput {
        default_projection: crate::display::runtime_partial(|_| None),
        env: &Names,
        value: Some(value),
        scale_factor: 1.0,
        writable: true,
        selection: None,
        pending: None,
        state: None,
        targets: ProjectionTargets::new(&target),
    }
}

fn number(n: f32) -> Value {
    crate::libraries::f32::value(n)
}

fn unshared(
    layout: &Recorded<crate::Editor, crate::frame::Hovered>,
) -> &Recorded<crate::Editor, crate::frame::Hovered> {
    match layout {
        Recorded::Shared { child, .. } => unshared(child),
        _ => layout,
    }
}

#[test]
fn operands_keep_their_paths_and_operator_targets_the_expression() {
    let sum = binary(SUM, number(1.0), number(2.0));
    let Recorded::Alternatives(options) = field(&input(&sum)).unwrap().record() else {
        panic!()
    };
    let Recorded::Row { children, .. } = &options[0] else {
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
            matches!(operand(parent, key, child).record(), Recorded::Row { .. }),
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
        assert!(field(&input(&value)).is_some());
        for (parent, key, grouped) in [
            (MULTIPLY, LEFT, true),
            (SUM, LEFT, false),
            (SUM, RIGHT, true),
        ] {
            assert_eq!(
                matches!(operand(parent, key, &value).record(), Recorded::Row { .. }),
                grouped,
            );
        }
    }
    for value in [unary(SIN, number(1.0)), node(AXIS, X.into())] {
        let enriched = Value::record(value.as_record().unwrap().update(extra, number(3.0)));
        assert!(field(&input(&enriched)).is_some());
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
        assert!(field(&input(&value)).is_none());
    }
    let complete = unary(SIN, number(1.0));
    assert!(
        field(&ProjectionInput {
            default_projection: crate::display::runtime_partial(|_| None),
            pending: Some(Pending::Field),
            ..input(&complete)
        })
        .is_none()
    );
}

#[test]
fn coordinates_are_shallow_and_names_remain_editable_data() {
    let axis = node(AXIS, X.into());
    assert!(
        matches!((field(&input(&axis))).map(|layout| inspect(&layout)), Some(ProjectionCall::Descend { step, projection: Some(_), .. })
        if step == Step::Key(AXIS))
    );
    let named = name::record("torus", [(SQUARE, Value::record([(OPERAND, number(1.0))]))]);
    let Recorded::Alternatives(options) = field(&input(&named)).unwrap().record() else {
        panic!()
    };
    let Recorded::Row { children, .. } = &options[0] else {
        panic!()
    };
    let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
        panic!()
    };
    assert!(
        matches!(&inspect(&(&head[0])), ProjectionCall::Descend { step, .. } if *step == Step::Key(name::vocabulary::NAME))
    );
}
