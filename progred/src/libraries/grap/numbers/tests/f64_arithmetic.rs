//! f64's arithmetic in Grap.

use crate::libraries::Library;
use crate::libraries::f64::*;
use crate::projection::Layout;
use crate::projection::test_support::ProjectionInput;
use crate::test_values::binary_call;
use ::grap::RuntimeValue;
use gid::CellId;

fn binary_display(input: &ProjectionInput<'_, RuntimeValue>) -> Option<Layout> {
    crate::libraries::representation::infix_display(
        vocabulary::F64,
        &super::super::f64_parts().infix,
        input,
    )
}

/// Binary notation's one-line form; the other wraps after the left operand.
fn infix_children(layout: &Layout) -> Option<Vec<Recorded<crate::Editor, crate::frame::Hovered>>> {
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        return None;
    };
    let Recorded::Row { children, .. } = options.first()?.content() else {
        return None;
    };
    Some(
        children
            .iter()
            .map(|child| unshared(child).clone())
            .collect(),
    )
}
use crate::display::recording::{Recordable, Recorded};
use crate::libraries::logic;
use ::grap::vocabulary::FUNCTION;

use crate::display::test_support::{NoEval, ProjectionCall, inspect, unshared};
use crate::libraries::test_widgets::projection_input;
use gid::{Step, Value, new_cell_id};

fn functions() -> ::grap::ForeignFunctions {
    crate::test_support::foreign_functions(&crate::libraries::grap::library::<crate::Editor>())
}

#[test]
fn generated_arithmetic_keeps_its_grouping_with_runtime_operands() {
    let sum = RuntimeValue::record([
        (
            FUNCTION,
            Value::from(crate::libraries::grap::numbers::f64::SUM).into(),
        ),
        (
            crate::libraries::grap::numbers::vocabulary::LEFT,
            RuntimeValue::f64(1.0),
        ),
        (
            crate::libraries::grap::numbers::vocabulary::RIGHT,
            RuntimeValue::f64(2.0),
        ),
    ]);
    let product = RuntimeValue::record([
        (
            FUNCTION,
            Value::from(crate::libraries::grap::numbers::f64::MULTIPLY).into(),
        ),
        (crate::libraries::grap::numbers::vocabulary::LEFT, sum),
        (
            crate::libraries::grap::numbers::vocabulary::RIGHT,
            RuntimeValue::f64(3.0),
        ),
    ]);
    let children =
        infix_children(&binary_display(&projection_input(&NoEval, &product)).unwrap()).unwrap();
    crate::display::test_support::delimited(&children[0]);
}

#[test]
fn extrema_rounding_and_norm_follow_the_numeric_contract() {
    let evaluate = |expression: Value| {
        crate::test_support::test_evaluate(&expression, |_| None, &functions(), 100).result
    };
    for (function, a, b, expected) in [
        (crate::libraries::grap::numbers::f64::MIN, -3.0, 2.0, -3.0),
        (crate::libraries::grap::numbers::f64::MAX, -3.0, 2.0, 2.0),
        (crate::libraries::grap::numbers::f64::HYPOT, 3.0, 4.0, 5.0),
        (
            crate::libraries::grap::numbers::f64::HYPOT,
            3e200,
            4e200,
            5e200,
        ),
    ] {
        let actual = read(&evaluate(binary_call(function, value(a), value(b)))).unwrap();
        assert!((actual / expected - 1.0).abs() < 1e-14);
        assert_eq!(
            evaluate(binary_call(function, Value::record([]), value(b))),
            ::grap::absent::with_detail(
                crate::libraries::grap::numbers::f64::LEFT_NOT_F64,
                ::grap::absent::VALUE,
                Value::record([]),
            ),
        );
    }
    let unary_call = |function: CellId, argument| {
        grap::call(
            function.into(),
            [(
                crate::libraries::grap::numbers::vocabulary::OPERAND,
                argument,
            )],
        )
    };
    for (input, expected) in [(1.1, 2.0), (-1.1, -1.0), (2.0, 2.0)] {
        assert_eq!(
            evaluate(unary_call(
                crate::libraries::grap::numbers::f64::CEIL,
                value(input)
            )),
            value(expected)
        );
    }
    for input in [0.0, f64::MAX, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(
            evaluate(unary_call(
                crate::libraries::grap::numbers::f64::IS_FINITE,
                value(input)
            )),
            logic::value(input.is_finite()),
        );
    }
    for function in [
        crate::libraries::grap::numbers::f64::CEIL,
        crate::libraries::grap::numbers::f64::IS_FINITE,
    ] {
        assert_eq!(
            evaluate(unary_call(function, Value::record([]))),
            ::grap::absent::with_detail(
                crate::libraries::grap::numbers::f64::OPERAND_NOT_F64,
                ::grap::absent::VALUE,
                Value::record([]),
            ),
        );
        assert!(::grap::absent::is_absent(&evaluate(grap::call(
            function.into(),
            []
        ))));
    }
}

#[test]
fn rust_supplies_arithmetic_to_grap() {
    let sum = binary_call(
        crate::libraries::grap::numbers::f64::SUM,
        value(2.0),
        value(3.0),
    );
    let multiply = binary_call(
        crate::libraries::grap::numbers::f64::MULTIPLY,
        sum,
        value(4.0),
    );
    assert_eq!(
        crate::test_support::test_evaluate(&multiply, |_| None, &functions(), 20).result,
        value(20.0)
    );
}

#[test]
fn rust_supplies_lerp_to_grap() {
    let expression = grap::call(
        Value::from(crate::libraries::grap::numbers::f64::LERP),
        [
            (crate::libraries::grap::numbers::f64::START, value(10.0)),
            (crate::libraries::grap::numbers::f64::END, value(20.0)),
            (crate::libraries::grap::numbers::f64::AMOUNT, value(0.25)),
        ],
    );
    assert_eq!(
        crate::test_support::test_evaluate(&expression, |_| None, &functions(), 20).result,
        value(12.5)
    );
}

#[test]
fn runtime_lowering_preserves_an_enriched_value_through_a_binding() {
    let parameter = new_cell_id();
    let metadata = new_cell_id();
    let enriched = Value::Record(
        value(2.5)
            .as_record()
            .unwrap()
            .clone()
            .update(metadata, Value::from(b"kept".to_vec())),
    );
    let expression = grap::call(
        grap::lambda([parameter], Value::from(parameter)),
        [(parameter, enriched.clone())],
    );

    assert_eq!(
        crate::test_support::test_evaluate(&expression, |_| None, &functions(), 20).result,
        enriched
    );
}

#[test]
fn binary_notation_descends_through_source_fields_and_preserves_precedence() {
    let product = binary_call(
        crate::libraries::grap::numbers::f64::MULTIPLY,
        value(2.0),
        value(3.0),
    );
    let sum = binary_call(
        crate::libraries::grap::numbers::f64::SUM,
        value(1.0),
        product,
    );
    let layout = binary_display(&projection_input(&NoEval, &(&sum).into())).unwrap();
    let children = infix_children(&layout).unwrap();
    assert!(matches!(&inspect(&(&children[0])),
        ProjectionCall::Descend {
            steps,
            ..
        } if *steps == [Step::Key(crate::libraries::grap::numbers::vocabulary::LEFT)]
    ));
    assert!(matches!(&inspect(&(&children[1])),
        ProjectionCall::Descend {
            steps,
            projection: Some(_),
            ..
        } if *steps == [Step::Key(FUNCTION)]
    ));
    assert!(matches!(&inspect(&(&children[2])),
        ProjectionCall::Descend {
            steps,
            ..
        } if *steps == [Step::Key(crate::libraries::grap::numbers::vocabulary::RIGHT)]
    ));

    let product = binary_call(
        crate::libraries::grap::numbers::f64::MULTIPLY,
        binary_call(
            crate::libraries::grap::numbers::f64::SUM,
            value(1.0),
            value(2.0),
        ),
        value(3.0),
    );
    let layout = binary_display(&projection_input(&NoEval, &(&product).into())).unwrap();
    let children = infix_children(&layout).unwrap();
    crate::display::test_support::delimited(&children[0]);
}

#[test]
fn binary_notation_wraps_before_its_operator() {
    let sum = binary_call(
        crate::libraries::grap::numbers::f64::SUM,
        value(1.0),
        value(2.0),
    );
    let layout = binary_display(&projection_input(&NoEval, &(&sum).into())).unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("binary notation has a one-line form and a wrapped one");
    };
    let Recorded::Col { children, .. } = options[1].content() else {
        panic!("the wrapped form stacks");
    };
    // The left operand, then the operator with the right one.
    assert_eq!(children.len(), 2);
}

#[test]
fn every_numeric_operator_reads_infix() {
    use crate::libraries::{f32, u64};
    let infix = |library: &Library<crate::Editor>, call: Value| {
        let layout = (library.projection.as_ref().unwrap())(&projection_input(
            &NoEval,
            &RuntimeValue::from(&call),
        ))
        .expect("a call projects");
        let Some(children) = infix_children(&layout) else {
            return false;
        };
        let descends = |index: usize, key: CellId| {
            matches!(&inspect(&(&children[index])),
                ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(key)]
            )
        };
        children.len() == 3
            && descends(0, crate::libraries::grap::numbers::vocabulary::LEFT)
            && descends(1, FUNCTION)
            && descends(2, crate::libraries::grap::numbers::vocabulary::RIGHT)
    };
    for (library, operators, one) in [
        (
            crate::libraries::grap::library(),
            [
                crate::libraries::grap::numbers::f64::SUM,
                crate::libraries::grap::numbers::f64::SUBTRACT,
                crate::libraries::grap::numbers::f64::MULTIPLY,
                crate::libraries::grap::numbers::f64::DIVIDE,
                crate::libraries::grap::numbers::f64::LESS,
                crate::libraries::grap::numbers::f64::EQUAL,
            ],
            value(1.0),
        ),
        (
            crate::libraries::grap::library(),
            [
                crate::libraries::grap::numbers::f32::SUM,
                crate::libraries::grap::numbers::f32::SUBTRACT,
                crate::libraries::grap::numbers::f32::MULTIPLY,
                crate::libraries::grap::numbers::f32::DIVIDE,
                crate::libraries::grap::numbers::f32::LESS,
                crate::libraries::grap::numbers::f32::EQUAL,
            ],
            f32::value(1.0),
        ),
        (
            crate::libraries::grap::library(),
            [
                crate::libraries::grap::numbers::u64::SUM,
                crate::libraries::grap::numbers::u64::SUBTRACT,
                crate::libraries::grap::numbers::u64::MULTIPLY,
                crate::libraries::grap::numbers::u64::DIVIDE,
                crate::libraries::grap::numbers::u64::LESS,
                crate::libraries::grap::numbers::u64::EQUAL,
            ],
            u64::value(1),
        ),
    ] {
        for operator in operators {
            assert!(
                infix(&library, binary_call(operator, one.clone(), one.clone())),
                "{operator:?} reads infix"
            );
        }
    }
    // Named operations stay calls.
    for operation in [
        crate::libraries::grap::numbers::f64::MIN,
        crate::libraries::grap::numbers::f64::MAX,
        crate::libraries::grap::numbers::f64::HYPOT,
    ] {
        assert!(!infix(
            &crate::libraries::grap::library(),
            binary_call(operation, value(1.0), value(2.0))
        ));
    }
}

#[test]
fn binary_notation_reads_extra_or_missing_fields_without_changing_grouping() {
    let extra = new_cell_id();
    let sum = grap::call(
        Value::from(crate::libraries::grap::numbers::f64::SUM),
        [
            (
                crate::libraries::grap::numbers::vocabulary::LEFT,
                value(1.0),
            ),
            (
                crate::libraries::grap::numbers::vocabulary::RIGHT,
                value(2.0),
            ),
            (extra, value(3.0)),
        ],
    );
    assert!(binary_display(&projection_input(&NoEval, &(&sum).into())).is_some());
    for (parent, field, grouped) in [
        (
            crate::libraries::grap::numbers::f64::MULTIPLY,
            crate::libraries::grap::numbers::vocabulary::LEFT,
            true,
        ),
        (
            crate::libraries::grap::numbers::f64::SUM,
            crate::libraries::grap::numbers::vocabulary::LEFT,
            false,
        ),
        (
            crate::libraries::grap::numbers::f64::SUM,
            crate::libraries::grap::numbers::vocabulary::RIGHT,
            true,
        ),
    ] {
        let (left, right, index) = if field == crate::libraries::grap::numbers::vocabulary::LEFT {
            (sum.clone(), value(4.0), 0)
        } else {
            (value(4.0), sum.clone(), 2)
        };
        let layout = binary_display(&projection_input(
            &NoEval,
            &(&binary_call(parent, left, right)).into(),
        ))
        .unwrap();
        let children = infix_children(&layout).unwrap();
        assert_eq!(
            matches!(children[index].content(), Recorded::Row { .. }),
            grouped
        );
    }
    let without = |value: &Value, key| Value::record(value.as_record().unwrap().without(&key));
    assert!(
        binary_display(&projection_input(
            &NoEval,
            &(&without(&sum, FUNCTION)).into()
        ))
        .is_none()
    );
    // From the moment the operator is chosen, a missing operand is a slot.
    let plain = without(&sum, extra);
    for (key, index) in [
        (crate::libraries::grap::numbers::vocabulary::LEFT, 0),
        (crate::libraries::grap::numbers::vocabulary::RIGHT, 2),
    ] {
        let layout =
            binary_display(&projection_input(&NoEval, &(&without(&plain, key)).into())).unwrap();
        let children = infix_children(&layout).unwrap();
        assert!(matches!(&inspect(&(&children[index])),
            ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(key)]
        ));
    }
}

#[test]
fn type_absences_are_library_values() {
    let left = binary_call(
        crate::libraries::grap::numbers::f64::SUM,
        Value::from(b"two".to_vec()),
        value(3.0),
    );
    let right = binary_call(
        crate::libraries::grap::numbers::f64::SUM,
        value(2.0),
        Value::from(b"three".to_vec()),
    );
    assert_eq!(
        crate::test_support::test_evaluate(&left, |_| None, &functions(), 10).result,
        ::grap::absent::with_detail(
            crate::libraries::grap::numbers::f64::LEFT_NOT_F64,
            ::grap::absent::VALUE,
            Value::from(b"two".to_vec()),
        )
    );
    assert_eq!(
        crate::test_support::test_evaluate(&right, |_| None, &functions(), 10).result,
        ::grap::absent::with_detail(
            crate::libraries::grap::numbers::f64::RIGHT_NOT_F64,
            ::grap::absent::VALUE,
            Value::from(b"three".to_vec()),
        )
    );
}

#[test]
fn an_operand_that_isnt_a_number_is_carried_as_the_cause() {
    use crate::libraries::grap::numbers::{f64 as op, vocabulary::OPERAND};
    use ::grap::absent;
    let evaluate = |expression: &gid::Value| {
        crate::test_support::evaluate_with(&crate::libraries::grap::library(), expression)
    };
    let not = |reason, operand| absent::with_detail(reason, absent::VALUE, operand);
    let (left, right) = (
        absent::value(gid::new_cell_id()),
        absent::value(gid::new_cell_id()),
    );
    let text = crate::libraries::text::value("one");
    for (l, r, expected) in [
        // The left operand's problem comes first, whatever the right's is.
        (
            left.clone(),
            right.clone(),
            not(op::LEFT_NOT_F64, left.clone()),
        ),
        (
            text.clone(),
            right.clone(),
            not(op::LEFT_NOT_F64, text.clone()),
        ),
        (
            value(1.0),
            right.clone(),
            not(op::RIGHT_NOT_F64, right.clone()),
        ),
    ] {
        assert_eq!(evaluate(&binary_call(op::SUM, l, r)), expected);
    }
    assert_eq!(
        evaluate(&crate::test_values::call(
            op::SIN,
            [(OPERAND, left.clone())]
        )),
        not(op::OPERAND_NOT_F64, left.clone())
    );
    // A loop that keeps using an absent stops growing its chain, keeping the
    // root cause.
    let sum = (0..2 * absent::CHAIN_LIMIT)
        .fold(left.clone(), |sum, _| binary_call(op::SUM, sum, value(1.0)));
    let result =
        crate::test_support::test_evaluate(
            &sum,
            |_| None,
            &crate::test_support::foreign_functions(&crate::libraries::grap::library::<
                crate::Editor,
            >()),
            1000,
        )
        .result;
    assert_eq!(absent::chain_length(&result), absent::CHAIN_LIMIT);
    assert_eq!(absent::root_cause(&result), &left);
}
