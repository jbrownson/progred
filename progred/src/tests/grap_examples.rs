//! Executable checks for the Grap programs in the checked-in example
//! documents. The GID text bridge owns their notation round trips; this module owns
//! their application-level evaluation behavior.

use crate::gid_text::{Binders, parse};
use crate::libraries::f64;
use gid::{Document, Value};

fn evaluated_expression(value: &Value) -> &Value {
    value
        .as_record()
        .filter(|fields| {
            grap::isa::of(fields) == Some(crate::libraries::grap::vocabulary::kind::EVALUATION)
        })
        .and_then(|fields| fields.get(&grap::vocabulary::EXPRESSION))
        .expect("an evaluation")
}

fn evaluate(doc: &Document, expression: &Value) -> grap::Evaluation {
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc,
        libraries: &stack.libraries,
    };
    grap::Evaluator::new(&sources, crate::sources::DEFAULT_FUEL).evaluate(expression)
}

#[test]
fn the_sample_contains_a_projectable_grap_computation() {
    let doc = crate::sample::sample_document();
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
    assert_eq!(
        evaluate(&doc, expression).result.to_value(),
        f64::value(5.0)
    );

    let profile = evaluated_expression(
        roof.get(&crate::test_values::label("profile"))
            .expect("profile call"),
    );
    assert_eq!(evaluate(&doc, profile).result.to_value(), f64::value(40.0));
}

/// The case list the demo's quote generator makes for `value`: data, not
/// a match.
fn generated_cases(binders: &Binders, value: ::core::primitive::f64) -> Value {
    Value::list([Value::record([
        (binders["isa"], Value::from(binders["case"])),
        (binders["pattern"], crate::libraries::text::value("subject")),
        (binders["expression"], f64::value(value)),
    ])])
}

/// The demo's circle of `radius`, as its `circle` function makes it.
fn circle(binders: &Binders, radius: ::core::primitive::f64) -> Value {
    Value::record([(
        binders["circle"],
        Value::record([(binders["radius"], f64::value(radius))]),
    )])
}

fn demo_fixture() -> (Document, Binders) {
    parse(include_str!("../../../examples/grap-demo.gid")).expect("the Grap demo parses")
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
        ("circle_result", circle(&binders, 34.0)),
        ("match_result", f64::value(3.0)),
        ("quoted_cases", generated_cases(&binders, 3.0)),
        ("metadata_call", f64::value(7.0)),
    ] {
        assert_eq!(
            evaluate(
                &doc,
                evaluated_expression(demo_entry(&doc, &binders, label)),
            )
            .result
            .to_value(),
            expected
        );
    }

    let inert = evaluated_expression(demo_entry(&doc, &binders, "inert_data"));
    assert_eq!(evaluate(&doc, inert).result.to_value(), *inert);
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "type_absent")),
        )
        .result
        .to_value(),
        crate::test_values::invalid_arguments([(
            crate::libraries::grap::numbers::vocabulary::LEFT,
            crate::test_values::not(
                crate::libraries::grap::numbers::f64::NOT_F64,
                crate::libraries::text::value("three"),
            ),
        )])
    );
    // A missing argument is what the + couldn't use.
    for (label, reason) in [
        ("missing_argument", grap::absent::MISSING_ARGUMENT),
        ("not_callable", grap::absent::NOT_CALLABLE),
    ] {
        let result = evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, label)),
        )
        .result;
        assert_eq!(
            ::grap::absent::root_causes(result.as_value())
                .into_iter()
                .map(::grap::absent::reason)
                .collect::<Vec<_>>(),
            [Some(reason)]
        );
    }

    doc.cells.set_value(binders["a_value"], f64::value(5.0));
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "add_result")),
        )
        .result
        .to_value(),
        f64::value(9.0)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "graph_function_result")),
        )
        .result
        .to_value(),
        f64::value(54.0)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "circle_result")),
        )
        .result
        .to_value(),
        circle(&binders, 54.0)
    );
    assert_eq!(
        evaluate(
            &doc,
            evaluated_expression(demo_entry(&doc, &binders, "quoted_cases")),
        )
        .result
        .to_value(),
        generated_cases(&binders, 5.0)
    );
}
