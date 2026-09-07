use super::*;
use crate::fidget::{binary, node, unary};
use progred_display::{Env, Pending, ProjectionTarget, ProjectionTargets};
use std::rc::Rc;

struct Names;

impl Env for Names {
    fn apply(&self, _: &Value, _: &[(CellId, Value)]) -> (Value, usize) {
        panic!("source projection must not evaluate")
    }

    fn evaluate(&self, _: &Value) -> (Value, usize) {
        panic!("source projection must not evaluate")
    }

    fn name(&self, _: CellId) -> Option<&str> {
        Some("renamed operator")
    }
}

fn target(path: Vec<Step>) -> ProjectionTarget<(), Vec<Step>> {
    ProjectionTarget {
        hover: path,
        select: Rc::new(|_| true),
        select_with: Rc::new(|_, _| true),
    }
}

fn input(value: &Value) -> ProjectionInput<'_, (), Vec<Step>> {
    ProjectionInput {
        default_projection: progred_display::partial(|_| None),
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
    crate::f32::value(n)
}

fn unshared(layout: &Layout<(), Vec<Step>>) -> &Layout<(), Vec<Step>> {
    match layout {
        Layout::Shared { child, .. } => unshared(child),
        _ => layout,
    }
}

#[test]
fn operands_keep_their_paths_and_operator_targets_the_expression() {
    let sum = binary(SUM, number(1.0), number(2.0));
    let Layout::Alternatives(options) = field(&input(&sum)).unwrap() else {
        panic!()
    };
    let Layout::Row { children, .. } = &options[0] else {
        panic!()
    };
    for (child, key) in [(&children[0], LEFT), (&children[2], RIGHT)] {
        assert!(matches!(unshared(child), Layout::At { steps, .. }
            if *steps == [Step::Key(SUM), Step::Key(key)]));
    }
    assert_eq!(
        crate::test_widgets::claim(unshared(&children[1])),
        Some(puri::hover::Claim::Direct(vec![]))
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
                operand::<(), ()>(parent, key, child),
                Layout::Surround { .. }
            ),
            grouped
        );
    }
}

#[test]
fn unshown_or_incomplete_fields_decline_instead_of_disappearing() {
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
        node(SUM, Value::record([(LEFT, number(1.0))])),
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
            default_projection: progred_display::partial(|_| None),
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
        matches!(field(&input(&axis)), Some(Layout::At { steps, projection: Some(_), .. })
        if steps == [Step::Key(AXIS)])
    );
    let named = name::record("torus", [(SQUARE, Value::record([(OPERAND, number(1.0))]))]);
    let Layout::Alternatives(options) = field(&input(&named)).unwrap() else {
        panic!()
    };
    let Layout::Row { children, .. } = &options[0] else {
        panic!()
    };
    let Layout::Row { children: head, .. } = unshared(&children[0]) else {
        panic!()
    };
    assert!(
        matches!(&head[0], Layout::At { steps, .. } if *steps == [Step::Key(name::vocabulary::NAME)])
    );
}
