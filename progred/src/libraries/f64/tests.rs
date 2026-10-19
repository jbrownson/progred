use super::*;
use crate::projection::{Layout, ProjectionInput};

fn binary_display(input: &ProjectionInput<'_, RuntimeValue>) -> Option<Layout> {
    crate::libraries::representation::infix_display(vocabulary::F64, &parts().infix, input)
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
use crate::libraries::{line_edit, logic};

use crate::display::test_support::{NoEval, ProjectionCall, inspect, unshared};
use crate::libraries::test_widgets::projection_input;
use gid::new_cell_id;

#[test]
fn numeric_projections_read_runtime_facets_and_ignore_unrelated_children() {
    type Project = for<'a, 'b> fn(&'b ProjectionInput<'a, RuntimeValue>) -> Option<Layout>;
    let extra = new_cell_id();
    let functions = functions();
    let host = crate::libraries::test_host(|_| None, &functions);
    let callback = ::grap::evaluate(&::grap::lambda([], value(1.0)), &host, 100).result;
    for (field, bytes, project) in [
        (
            vocabulary::F64,
            3.5_f64.to_le_bytes().to_vec(),
            (|input| convention().display(input)) as Project,
        ),
        (
            crate::libraries::f32::vocabulary::F32,
            3.5_f32.to_le_bytes().to_vec(),
            |input| crate::libraries::f32::convention().display(input),
        ),
        (
            crate::libraries::u64::vocabulary::U64,
            42_u64.to_le_bytes().to_vec(),
            |input| crate::libraries::u64::convention().display(input),
        ),
    ] {
        let number = RuntimeValue::record([
            (field, Value::from(bytes).into()),
            (extra, callback.clone()),
        ]);
        assert!(project(&projection_input(&NoEval, &number)).is_some());
        assert!(number.field(extra).unwrap().same_result(&callback));
        let invalid = RuntimeValue::record([(field, Value::from(vec![0]).into())]);
        assert!(project(&projection_input(&NoEval, &invalid)).is_none());
    }
    assert!(
        convention()
            .display(&projection_input(&NoEval, &RuntimeValue::f64(3.5)))
            .is_some()
    );
}

#[test]
fn generated_arithmetic_keeps_its_grouping_with_runtime_operands() {
    let sum = RuntimeValue::record([
        (FUNCTION, Value::from(vocabulary::SUM).into()),
        (vocabulary::LEFT, RuntimeValue::f64(1.0)),
        (vocabulary::RIGHT, RuntimeValue::f64(2.0)),
    ]);
    let product = RuntimeValue::record([
        (FUNCTION, Value::from(vocabulary::MULTIPLY).into()),
        (vocabulary::LEFT, sum),
        (vocabulary::RIGHT, RuntimeValue::f64(3.0)),
    ]);
    let children =
        infix_children(&binary_display(&projection_input(&NoEval, &product)).unwrap()).unwrap();
    crate::display::test_support::delimited(&children[0]);
}

fn call(function: CellId, left: Value, right: Value) -> Value {
    grap::call(
        Value::from(function),
        [(vocabulary::LEFT, left), (vocabulary::RIGHT, right)],
    )
}

#[test]
fn extrema_rounding_and_norm_follow_the_numeric_contract() {
    let evaluate = |expression: Value| {
        crate::libraries::test_evaluate(&expression, |_| None, &functions(), 100).result
    };
    for (function, a, b, expected) in [
        (vocabulary::MIN, -3.0, 2.0, -3.0),
        (vocabulary::MAX, -3.0, 2.0, 2.0),
        (vocabulary::HYPOT, 3.0, 4.0, 5.0),
        (vocabulary::HYPOT, 3e200, 4e200, 5e200),
    ] {
        let actual = read(&evaluate(call(function, value(a), value(b)))).unwrap();
        assert!((actual / expected - 1.0).abs() < 1e-14);
        assert_eq!(
            evaluate(call(function, Value::record([]), value(b))),
            absent::with_reason(vocabulary::LEFT_NOT_F64),
        );
    }
    let unary_call =
        |function: CellId, argument| grap::call(function.into(), [(vocabulary::OPERAND, argument)]);
    for (input, expected) in [(1.1, 2.0), (-1.1, -1.0), (2.0, 2.0)] {
        assert_eq!(
            evaluate(unary_call(vocabulary::CEIL, value(input))),
            value(expected)
        );
    }
    for input in [0.0, f64::MAX, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(
            evaluate(unary_call(vocabulary::IS_FINITE, value(input))),
            logic::value(input.is_finite()),
        );
    }
    for function in [vocabulary::CEIL, vocabulary::IS_FINITE] {
        assert_eq!(
            evaluate(unary_call(function, Value::record([]))),
            absent::with_reason(vocabulary::OPERAND_NOT_F64),
        );
        assert!(absent::is_absent(&evaluate(grap::call(
            function.into(),
            []
        ))));
    }
}

#[test]
fn representation_is_library_data() {
    assert_eq!(read(&value(2.5)), Some(2.5));
    assert_eq!(read(&Value::from(b"2.5".to_vec())), None);

    let extra = new_cell_id();
    let with_extra = Value::record(
        value(2.5)
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(b"degrees".to_vec())),
    );
    assert_eq!(read(&with_extra), Some(2.5));
    let update = |input: &str| {
        crate::libraries::test_evaluate(
            &grap::call(
                grap::ffi(vocabulary::UPDATE),
                [
                    (line_edit::vocabulary::CURRENT, with_extra.clone()),
                    (
                        line_edit::vocabulary::INPUT,
                        crate::libraries::text::value(input),
                    ),
                ],
            ),
            |_| None,
            &functions(),
            100,
        )
        .result
    };
    // Unparseable input returns an absent — the line editor drops
    // the write whole.
    assert!(crate::libraries::absent::is_absent(&update("junk")));
    assert_eq!(
        update("3"),
        Value::record(
            value(3.0)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"degrees".to_vec())),
        )
    );
}

#[test]
fn rust_supplies_arithmetic_to_grap() {
    let sum = call(vocabulary::SUM, value(2.0), value(3.0));
    let multiply = call(vocabulary::MULTIPLY, sum, value(4.0));
    assert_eq!(
        crate::libraries::test_evaluate(&multiply, |_| None, &functions(), 20).result,
        value(20.0)
    );
}

#[test]
fn rust_supplies_lerp_to_grap() {
    let expression = grap::call(
        Value::from(vocabulary::LERP),
        [
            (vocabulary::START, value(10.0)),
            (vocabulary::END, value(20.0)),
            (vocabulary::AMOUNT, value(0.25)),
        ],
    );
    assert_eq!(
        crate::libraries::test_evaluate(&expression, |_| None, &functions(), 20).result,
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
        crate::libraries::test_evaluate(&expression, |_| None, &functions(), 20).result,
        enriched
    );
}

#[test]
fn binary_notation_descends_through_source_fields_and_preserves_precedence() {
    let product = call(vocabulary::MULTIPLY, value(2.0), value(3.0));
    let sum = call(vocabulary::SUM, value(1.0), product);
    let layout = binary_display(&projection_input(&NoEval, &(&sum).into())).unwrap();
    let children = infix_children(&layout).unwrap();
    assert!(matches!(&inspect(&(&children[0])),
        ProjectionCall::Descend {
            step: Step::Key(field),
            ..
        } if *field == vocabulary::LEFT
    ));
    assert!(matches!(&inspect(&(&children[1])),
        ProjectionCall::Descend {
            step: Step::Key(field),
            projection: Some(_),
            ..
        } if *field == FUNCTION
    ));
    assert!(matches!(&inspect(&(&children[2])),
        ProjectionCall::Descend {
            step: Step::Key(field),
            ..
        } if *field == vocabulary::RIGHT
    ));

    let product = call(
        vocabulary::MULTIPLY,
        call(vocabulary::SUM, value(1.0), value(2.0)),
        value(3.0),
    );
    let layout = binary_display(&projection_input(&NoEval, &(&product).into())).unwrap();
    let children = infix_children(&layout).unwrap();
    crate::display::test_support::delimited(&children[0]);
}

#[test]
fn binary_notation_wraps_before_its_operator() {
    let sum = call(vocabulary::SUM, value(1.0), value(2.0));
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
                ProjectionCall::Descend { step: Step::Key(field), .. } if *field == key
            )
        };
        children.len() == 3
            && descends(0, vocabulary::LEFT)
            && descends(1, FUNCTION)
            && descends(2, vocabulary::RIGHT)
    };
    for (library, operators, one) in [
        (
            library(),
            [
                vocabulary::SUM,
                vocabulary::SUBTRACT,
                vocabulary::MULTIPLY,
                vocabulary::DIVIDE,
                vocabulary::LESS,
                vocabulary::EQUAL,
            ],
            value(1.0),
        ),
        (
            f32::library(),
            [
                f32::vocabulary::SUM,
                f32::vocabulary::SUBTRACT,
                f32::vocabulary::MULTIPLY,
                f32::vocabulary::DIVIDE,
                f32::vocabulary::LESS,
                f32::vocabulary::EQUAL,
            ],
            f32::value(1.0),
        ),
        (
            u64::library(),
            [
                u64::vocabulary::SUM,
                u64::vocabulary::SUBTRACT,
                u64::vocabulary::MULTIPLY,
                u64::vocabulary::DIVIDE,
                u64::vocabulary::LESS,
                u64::vocabulary::EQUAL,
            ],
            u64::value(1),
        ),
    ] {
        for operator in operators {
            assert!(
                infix(&library, call(operator, one.clone(), one.clone())),
                "{operator:?} reads infix"
            );
        }
    }
    // Named operations stay calls.
    for operation in [vocabulary::MIN, vocabulary::MAX, vocabulary::HYPOT] {
        assert!(!infix(&library(), call(operation, value(1.0), value(2.0))));
    }
}

#[test]
fn binary_notation_reads_extra_or_missing_fields_without_changing_grouping() {
    let extra = new_cell_id();
    let sum = grap::call(
        Value::from(vocabulary::SUM),
        [
            (vocabulary::LEFT, value(1.0)),
            (vocabulary::RIGHT, value(2.0)),
            (extra, value(3.0)),
        ],
    );
    assert!(binary_display(&projection_input(&NoEval, &(&sum).into())).is_some());
    for (parent, field, grouped) in [
        (vocabulary::MULTIPLY, vocabulary::LEFT, true),
        (vocabulary::SUM, vocabulary::LEFT, false),
        (vocabulary::SUM, vocabulary::RIGHT, true),
    ] {
        let (left, right, index) = if field == vocabulary::LEFT {
            (sum.clone(), value(4.0), 0)
        } else {
            (value(4.0), sum.clone(), 2)
        };
        let layout = binary_display(&projection_input(
            &NoEval,
            &(&call(parent, left, right)).into(),
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
    for (key, index) in [(vocabulary::LEFT, 0), (vocabulary::RIGHT, 2)] {
        let layout =
            binary_display(&projection_input(&NoEval, &(&without(&plain, key)).into())).unwrap();
        let children = infix_children(&layout).unwrap();
        assert!(matches!(&inspect(&(&children[index])),
            ProjectionCall::Descend { step: Step::Key(field), .. } if *field == key
        ));
    }
}

#[test]
fn type_absences_are_library_values() {
    let left = call(vocabulary::SUM, Value::from(b"two".to_vec()), value(3.0));
    let right = call(vocabulary::SUM, value(2.0), Value::from(b"three".to_vec()));
    assert_eq!(
        crate::libraries::test_evaluate(&left, |_| None, &functions(), 10).result,
        absent::with_reason(vocabulary::LEFT_NOT_F64)
    );
    assert_eq!(
        crate::libraries::test_evaluate(&right, |_| None, &functions(), 10).result,
        absent::with_reason(vocabulary::RIGHT_NOT_F64)
    );
}

#[test]
fn library_names_are_ordinary_facts_for_random_identities() {
    let library = library::<crate::Editor>();
    assert_eq!(
        library
            .definitions
            .value(vocabulary::F64)
            .and_then(name::read),
        Some("f64")
    );
    assert_eq!(
        library
            .definitions
            .value(vocabulary::SUM)
            .and_then(name::read),
        Some("+")
    );
    assert_eq!(
        library
            .definitions
            .value(vocabulary::PI)
            .and_then(name::read),
        Some("π")
    );
    assert_eq!(
        library.definitions.value(vocabulary::PI).and_then(read),
        Some(std::f64::consts::PI)
    );
    assert_eq!(
        library
            .definitions
            .value(vocabulary::LEFT_NOT_F64)
            .and_then(name::read),
        Some("left is not f64")
    );
    assert_eq!(
        library
            .definitions
            .value(vocabulary::RIGHT_NOT_F64)
            .and_then(name::read),
        Some("right is not f64")
    );
    assert_eq!(
        absent::reason(&absent::with_reason(vocabulary::LEFT_NOT_F64)),
        Some(vocabulary::LEFT_NOT_F64)
    );
    assert_eq!(
        absent::reason(&absent::with_reason(vocabulary::RIGHT_NOT_F64)),
        Some(vocabulary::RIGHT_NOT_F64)
    );
    assert!(library.definitions.value(vocabulary::SUM).is_some());
}
