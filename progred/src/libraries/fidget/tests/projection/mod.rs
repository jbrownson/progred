use super::*;
use crate::projection::Layout;
use crate::projection::test_support::ProjectionInput;

fn field(input: &ProjectionInput<'_>) -> Option<Layout> {
    project_runtime(super::field, input)
}
use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::{ProjectionCall, inspect, unshared};
use crate::display::{Env, Pending};
use crate::libraries::fidget::{axis, binary, node, unary};
use crate::libraries::test_widgets::{project_runtime, projection_input};

struct Names;

impl Env for Names {
    fn apply_expression_runtime(
        &self,
        _: &::grap::RuntimeValue,
        _: &[(CellId, ::grap::RuntimeValue)],
    ) -> ::grap::RuntimeValue {
        panic!("source projection must not evaluate")
    }

    fn evaluate_runtime(
        &self,
        _: &::grap::RuntimeValue,
        _: usize,
        _: &[Step],
    ) -> ::grap::RuntimeValue {
        panic!("source projection must not evaluate")
    }

    fn evaluate_runtime_memo(
        &self,
        _: &::grap::RuntimeValue,
        _: usize,
        _: &[Step],
    ) -> ::grap::RuntimeValue {
        panic!("source projection must not evaluate")
    }

    fn apply_runtime_scoped(
        &self,
        _: &::grap::RuntimeValue,
        _: &[(CellId, ::grap::RuntimeValue)],
        _: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation {
        panic!("source projection must not evaluate")
    }

    fn apply_runtime_memo(
        &self,
        _: &Value,
        _: &[(CellId, Value)],
        _: usize,
    ) -> ::grap::RuntimeValue {
        panic!("source projection must not evaluate")
    }

    fn name(&self, _: CellId) -> Option<crate::display::Name<'_>> {
        Some(crate::display::Name::Plain("renamed operator"))
    }
}

fn number(n: f32) -> Value {
    crate::libraries::f32::value(n)
}

#[test]
fn operands_keep_their_paths_and_operator_targets_the_expression() {
    let sum = binary(kind::SUM, number(1.0), number(2.0));
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
            matches!(&inspect(&(unshared(child))), ProjectionCall::Descend { steps, .. }
            if *steps == [Step::Key(key)])
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
    let sum = binary(kind::SUM, number(1.0), number(2.0));
    let product = binary(kind::MULTIPLY, number(1.0), number(2.0));
    for (parent, key, child, expected) in [
        (kind::MULTIPLY, LEFT, &sum, true),
        (kind::SUM, LEFT, &product, false),
        (kind::SUM, LEFT, &sum, false),
        (kind::SUM, RIGHT, &sum, true),
        (kind::DIVIDE, RIGHT, &product, true),
    ] {
        assert_eq!(grouped(parent, key, child), expected);
    }
}

#[test]
fn unrelated_fields_do_not_block_notation_or_change_grouping() {
    let extra = gid::new_cell_id();
    let value = node(
        kind::SUM,
        [
            (LEFT, number(1.0)),
            (RIGHT, number(2.0)),
            (extra, number(3.0)),
        ],
    );
    assert!(field(&projection_input(&Names, &value)).is_some());
    for (parent, key, expected) in [
        (kind::MULTIPLY, LEFT, true),
        (kind::SUM, LEFT, false),
        (kind::SUM, RIGHT, true),
    ] {
        assert_eq!(grouped(parent, key, &value), expected);
    }
    for value in [unary(kind::SIN, number(1.0)), axis(X)] {
        let enriched = Value::record(value.as_record().unwrap().with(extra, number(3.0)));
        assert!(field(&projection_input(&Names, &enriched)).is_some());
    }
}

#[test]
fn incomplete_malformed_and_unkinded_forms_still_decline() {
    for value in [
        node(kind::SUM, [(LEFT, number(1.0))]),
        node(kind::SUM, [(RIGHT, number(1.0))]),
        node(kind::SIN, []),
        node(kind::AXIS, [(AXIS, number(1.0))]),
        // What would be a node, but says no kind.
        Value::record([(AXIS, X.into()), (OPERAND, number(1.0))]),
    ] {
        assert!(field(&projection_input(&Names, &value)).is_none());
    }
    let complete = unary(kind::SIN, number(1.0));
    assert!(
        field(&ProjectionInput {
            default_projection: crate::display::runtime_partial(|_| None),
            provided: Default::default(),
            pending: Some(Pending::Field),
            ..projection_input(&Names, &complete)
        })
        .is_none()
    );
}

#[test]
fn coordinates_are_shallow_and_names_remain_editable_data() {
    let axis = axis(X);
    assert!(
        matches!(inspect(&field(&projection_input(&Names, &axis)).unwrap()), ProjectionCall::Descend { steps, projection: Some(_), .. } if steps == [Step::Key(AXIS)])
    );
    let named = name::record(
        "torus",
        [
            (::grap::isa::ISA, kind::SQUARE.into()),
            (OPERAND, number(1.0)),
        ],
    );
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
        matches!(&inspect(&(&head[0])), ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(name::vocabulary::NAME)])
    );
}

mod frames;
