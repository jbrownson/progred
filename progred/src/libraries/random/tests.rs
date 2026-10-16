use super::*;
use ::grap;
use gid::new_cell_id;

fn sample(seed: Option<Value>) -> Value {
    let arguments = [
        Some((
            grap::vocabulary::EXPRESSION,
            grap::call(
                Value::from(vocabulary::BETWEEN),
                [
                    (vocabulary::MIN, f64::value(-2.0)),
                    (vocabulary::MAX, f64::value(3.0)),
                ],
            ),
        )),
        seed.map(|seed| (vocabulary::SEED, seed)),
    ];
    crate::libraries::test_evaluate(
        &grap::call(
            Value::from(vocabulary::WITH_RANDOM),
            arguments.into_iter().flatten(),
        ),
        |_| None,
        &functions(),
        30,
    )
    .result
}

#[test]
fn a_seeded_scope_repeats_and_zero_is_the_default() {
    assert_eq!(sample(None), sample(Some(u64::value(0))));
    assert_eq!(sample(Some(u64::value(42))), sample(Some(u64::value(42))));
    assert_ne!(sample(Some(u64::value(0))), sample(Some(u64::value(42))));
}

#[test]
fn declining_after_sampling_halts_the_evaluation() {
    let function = new_cell_id();
    let ignored = new_cell_id();
    let next = grap::call(
        vocabulary::BETWEEN.into(),
        [
            (vocabulary::MIN, f64::value(-2.0)),
            (vocabulary::MAX, f64::value(3.0)),
        ],
    );
    let functions = functions();
    let evaluation = grap::evaluate_value(
        &grap::call(
            vocabulary::WITH_RANDOM.into(),
            [(
                grap::vocabulary::EXPRESSION,
                grap::call(function.into(), []),
            )],
        ),
        &crate::libraries::TestHost(|cell| {
            if cell == function {
                vec![
                    (
                        gid::Resolution::Document,
                        grap::Definition::Value(grap::lambda(
                            [],
                            grap::call(
                                grap::lambda([ignored], absent::decline()),
                                [(ignored, next.clone())],
                            ),
                        )),
                    ),
                    (
                        gid::Resolution::Library(ID),
                        grap::Definition::Value(grap::lambda([], next.clone())),
                    ),
                ]
            } else {
                functions
                    .get(cell)
                    .map(|function| {
                        (
                            gid::Resolution::Library(ID),
                            grap::Definition::foreign(gid::Value::record([]), function.clone()),
                        )
                    })
                    .into_iter()
                    .collect()
            }
        }),
        100,
    );
    assert!(!evaluation.completed);
    assert_eq!(
        absent::reason(&evaluation.result),
        Some(grap::absent::EFFECTFUL_DECLINE),
    );
}

#[test]
fn successive_samples_advance_the_scoped_stream() {
    let pair = new_cell_id();
    let first_field = new_cell_id();
    let second_field = new_cell_id();
    let functions = functions().register(
        pair,
        ForeignFunction::from_value(move |context, call, environment| {
            let first = context
                .field(call, first_field)
                .map(|expression| context.eval_to_value(expression, environment))
                .transpose()?
                .unwrap_or_else(|| context.missing_argument(first_field));
            let second = context
                .field(call, second_field)
                .map(|expression| context.eval_to_value(expression, environment))
                .transpose()?
                .unwrap_or_else(|| context.missing_argument(second_field));
            Ok(Value::list([first, second]))
        }),
    );
    let between = || {
        grap::call(
            Value::from(vocabulary::BETWEEN),
            [
                (vocabulary::MIN, f64::value(0.0)),
                (vocabulary::MAX, f64::value(1.0)),
            ],
        )
    };
    let expression = grap::call(
        Value::from(vocabulary::WITH_RANDOM),
        [
            (vocabulary::SEED, u64::value(42)),
            (
                grap::vocabulary::EXPRESSION,
                grap::call(
                    Value::from(pair),
                    [(first_field, between()), (second_field, between())],
                ),
            ),
        ],
    );
    let evaluate = || crate::libraries::test_evaluate(&expression, |_| None, &functions, 60).result;
    let result = evaluate();

    assert!(result.as_list().is_some_and(|values| {
        let mut values = values.values();
        matches!(
            (values.next(), values.next(), values.next()),
            (Some(first), Some(second), None) if first != second
        )
    }));
    assert_eq!(result, evaluate());
}

#[test]
fn sampling_outside_a_scope_returns_a_stable_absent() {
    let result = crate::libraries::test_evaluate(
        &grap::call(
            Value::from(vocabulary::BETWEEN),
            [
                (vocabulary::MIN, f64::value(0.0)),
                (vocabulary::MAX, f64::value(1.0)),
            ],
        ),
        |_| None,
        &functions(),
        20,
    )
    .result;

    assert_eq!(result, absent::with_reason(vocabulary::OUTSIDE_SCOPE));
}
