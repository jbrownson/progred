//! Executable checks for the Grap programs in the checked-in example
//! documents. The GID text bridge owns their notation round trips; this module owns
//! their application-level evaluation behavior.

use crate::gid_text::{Binders, parse};
use gid::{Document, Value};
use progred_libraries::{f64, geometry};

fn evaluated_expression(value: &Value) -> &Value {
    value
        .as_record()
        .and_then(|fields| fields.get(&grap::vocabulary::EVALUATE))
        .expect("evaluation projection")
}

fn evaluate(doc: &Document, expression: &Value) -> grap::Evaluation {
    grap::evaluate(
        expression,
        |cell| doc.cells.value(cell).cloned(),
        &crate::stack::load::<()>().foreign,
        grap::DEFAULT_FUEL,
    )
}

#[test]
fn the_sample_contains_a_projectable_grap_computation() {
    let (doc, binders) =
        parse(include_str!("../../examples/sample.gid")).expect("the sample parses");
    let roof = doc
        .root
        .as_ref()
        .and_then(Value::as_record)
        .and_then(|root| root.get(&crate::test_values::label("shape")))
        .and_then(Value::as_cell)
        .and_then(|roof| doc.cells.value(roof))
        .and_then(Value::as_record)
        .expect("roof record");
    let expression = evaluated_expression(
        roof.get(&crate::test_values::label("double pitch"))
            .expect("Grap expression"),
    );
    assert_eq!(evaluate(&doc, expression).result, f64::value(5.0));

    let profile = evaluated_expression(
        roof.get(&crate::test_values::label("profile"))
            .expect("profile call"),
    );
    let evaluation = evaluate(&doc, profile);
    assert_eq!(evaluation.result, geometry::value(40.0));
    assert_eq!(
        evaluation.dependencies,
        [binders["double"], binders["pitch_value"]]
            .into_iter()
            .collect()
    );
}

fn demo_fixture() -> (Document, Binders) {
    parse(include_str!("../../examples/grap-demo.gid")).expect("the Grap demo parses")
}

fn demo_entry<'a>(doc: &'a Document, binders: &Binders, label: &str) -> &'a Value {
    doc.root
        .as_ref()
        .and_then(Value::as_list)
        .and_then(|entries| {
            entries
                .values()
                .filter_map(Value::as_record)
                .find_map(|entry| entry.get(&binders[label]))
        })
        .unwrap_or_else(|| panic!("demo entry `{label}`"))
}

#[test]
fn the_grap_demo_exercises_live_functions_data_and_absents() {
    let (mut doc, binders) = demo_fixture();
    for (label, expected) in [
        ("add_result", f64::value(7.0)),
        ("nested_result", f64::value(70.0)),
        ("evaluate_result", f64::value(7.0)),
        ("graph_function_result", f64::value(34.0)),
        ("circle_result", geometry::value(34.0)),
        ("match_result", f64::value(3.0)),
        ("quoted_match_result", f64::value(3.0)),
        ("metadata_call", f64::value(7.0)),
    ] {
        assert_eq!(
            evaluate(&doc, evaluated_expression(demo_entry(&doc, &binders, label)),).result,
            expected
        );
    }

    let inert = evaluated_expression(demo_entry(&doc, &binders, "inert_data"));
    assert_eq!(evaluate(&doc, inert).result, *inert);
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "type_absent")),
        )
        .result,
        Value::from(f64::vocabulary::LEFT_NOT_F64)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "missing_argument")),
        )
        .result,
        Value::from(grap::absent::MISSING_ARGUMENT)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "not_callable")),
        )
        .result,
        Value::from(grap::absent::NOT_CALLABLE)
    );

    doc.cells.set_value(binders["a_value"], f64::value(5.0));
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "add_result")),
        )
        .result,
        f64::value(9.0)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "graph_function_result")),
        )
        .result,
        f64::value(54.0)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "circle_result")),
        )
        .result,
        geometry::value(54.0)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "quoted_match_result")),
        )
        .result,
        f64::value(5.0)
    );
}
