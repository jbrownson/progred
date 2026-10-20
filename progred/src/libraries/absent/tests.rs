use super::*;
use gid::{Value, new_cell_id};

fn or_default(value: Value, default: Value) -> Value {
    ::grap::call(
        vocabulary::OR_DEFAULT.into(),
        [(vocabulary::VALUE, value), (vocabulary::DEFAULT, default)],
    )
}

#[test]
fn default_preserves_present_values_without_type_validation() {
    let metadata = new_cell_id();
    let enriched = Value::record(
        crate::libraries::f64::value(0.4)
            .as_record()
            .unwrap()
            .clone()
            .update(metadata, Value::from(vec![7])),
    );
    for value in [
        enriched,
        crate::libraries::text::value("not a number"),
        Value::record([]),
        Value::record([(vocabulary::ABSENT, Value::from(vec![1]))]),
    ] {
        let result = crate::test_support::test_evaluate(
            &or_default(value.clone(), new_cell_id().into()),
            |_| None,
            &functions(),
            100,
        );
        assert!(result.completed);
        assert_eq!(result.result, value);
    }
}

#[test]
fn default_evaluates_each_needed_argument_once_in_the_calling_environment() {
    use std::{cell::Cell, rc::Rc};
    let first = new_cell_id();
    let fallback = new_cell_id();
    let parameter = new_cell_id();
    for missing in [false, true] {
        let reads = Rc::new(Cell::new(0));
        let defaults = Rc::new(Cell::new(0));
        let functions = functions()
            .register(
                first,
                ForeignFunction::from_value({
                    let reads = reads.clone();
                    move |context, _, _| {
                        Ok(context.effect(|| {
                            reads.set(reads.get() + 1);
                            if missing {
                                ::grap::absent::value(new_cell_id())
                            } else {
                                Value::record([])
                            }
                        }))
                    }
                }),
            )
            .register(
                fallback,
                ForeignFunction::new({
                    let defaults = defaults.clone();
                    move |context, call, environment| {
                        let value = context.field(call, vocabulary::VALUE).unwrap();
                        let result = context.eval(value, environment)?;
                        Ok(context.effect(|| {
                            defaults.set(defaults.get() + 1);
                            result
                        }))
                    }
                }),
            );
        let result = crate::test_support::test_apply(
            &::grap::lambda(
                [parameter],
                or_default(
                    ::grap::call(first.into(), []),
                    ::grap::call(fallback.into(), [(vocabulary::VALUE, parameter.into())]),
                ),
            ),
            [(parameter, crate::libraries::f64::value(0.65))],
            |_| None,
            &functions,
            100,
        );
        assert!(result.completed);
        assert_eq!(reads.get(), 1);
        assert_eq!(defaults.get(), usize::from(missing));
        assert_eq!(
            result.result,
            if missing {
                crate::libraries::f64::value(0.65)
            } else {
                Value::record([])
            }
        );
    }
}

#[test]
fn default_recovers_returned_absents_but_does_not_intercept_evaluator_halts() {
    let replacement = ::grap::absent::value(new_cell_id());
    let returned = crate::test_support::test_evaluate(
        &or_default(
            ::grap::absent::value(::grap::absent::FUEL_EXHAUSTED),
            replacement.clone(),
        ),
        |_| None,
        &functions(),
        100,
    );
    assert!(returned.completed);
    assert_eq!(returned.result, replacement);

    let recurse = new_cell_id();
    let fallback = new_cell_id();
    let functions = functions().register(
        fallback,
        ForeignFunction::from_value(|_, _, _| {
            panic!("an evaluator halt must not evaluate the default")
        }),
    );
    let result = crate::test_support::test_evaluate(
        &or_default(
            ::grap::call(recurse.into(), []),
            ::grap::call(fallback.into(), []),
        ),
        |cell| (cell == recurse).then(|| ::grap::lambda([], ::grap::call(recurse.into(), []))),
        &functions,
        30,
    );
    assert!(!result.completed);
    assert_eq!(
        ::grap::absent::reason(&result.result),
        Some(::grap::absent::FUEL_EXHAUSTED)
    );
}

#[test]
fn absence_is_an_open_tag_with_a_stable_reason() {
    let reason = new_cell_id();
    let extra = new_cell_id();
    let absent = Value::record(
        ::grap::absent::value(reason)
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(vec![1])),
    );

    assert!(::grap::absent::is_absent(&absent));
    assert_eq!(::grap::absent::reason(&absent), Some(reason));
    assert!(!::grap::absent::is_absent(&name::record(
        "specific absence",
        []
    )));
    assert!(!::grap::absent::is_absent(&Value::record([(
        vocabulary::ABSENT,
        Value::from(vec![1]),
    )])));
}

#[test]
fn multiple_causes_form_an_ordered_absence_while_one_remains_itself() {
    let first = ::grap::absent::value(new_cell_id());
    let second = ::grap::absent::value(new_cell_id());

    assert_eq!(::grap::absent::from_causes([first.clone()]), first);
    let combined = ::grap::absent::from_causes([first.clone(), second.clone()]);
    assert_eq!(
        ::grap::absent::reason(&combined),
        Some(vocabulary::NO_ALTERNATIVE)
    );
    assert_eq!(
        combined
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::CAUSES)),
        Some(&Value::list([first, second])),
    );
}

fn checked(reason: Value, arguments: Value) -> Value {
    ::grap::call(
        vocabulary::CHECKED.into(),
        [
            (vocabulary::REASON, reason),
            (vocabulary::ARGUMENTS, arguments),
        ],
    )
}

fn evaluate(expression: &Value) -> Value {
    let result = crate::test_support::test_evaluate(expression, |_| None, &functions(), 100);
    assert!(result.completed);
    result.result
}

#[test]
fn checked_returns_its_arguments_values_or_its_reason_carrying_each_absent() {
    let reason = new_cell_id();
    let [width, height, depth] = std::array::from_fn(|_| new_cell_id());
    let size = crate::libraries::f64::value(3.0);
    let (first, second) = (
        ::grap::absent::value(new_cell_id()),
        ::grap::absent::value(new_cell_id()),
    );
    // Each argument is evaluated in the calling environment.
    assert_eq!(
        evaluate(&checked(
            reason.into(),
            Value::record([
                (width, or_default(first.clone(), size.clone())),
                (height, size.clone()),
            ])
        )),
        Value::record([(width, size.clone()), (height, size.clone())])
    );
    assert_eq!(
        evaluate(&checked(reason.into(), Value::record([]))),
        Value::record([])
    );
    // Each absent is carried under its label, and only the absents.
    assert_eq!(
        evaluate(&checked(
            reason.into(),
            Value::record([
                (width, first.clone()),
                (height, size.clone()),
                (depth, second.clone()),
            ])
        )),
        Value::record([
            (vocabulary::ABSENT, reason.into()),
            (width, first),
            (depth, second),
        ])
    );
}

#[test]
fn checked_reports_each_of_its_own_arguments_it_cant_use() {
    let missing = |label: CellId| {
        ::grap::absent::with_detail(
            ::grap::absent::MISSING_ARGUMENT,
            ::grap::absent::CELL,
            label.into(),
        )
    };
    assert_eq!(
        evaluate(&::grap::call(vocabulary::CHECKED.into(), [])),
        Value::record([
            (vocabulary::ABSENT, vocabulary::INVALID_CHECK.into()),
            (vocabulary::REASON, missing(vocabulary::REASON)),
            (vocabulary::ARGUMENTS, missing(vocabulary::ARGUMENTS)),
        ])
    );
    let (not_a_cell, not_a_record) = (Value::from(vec![1]), Value::from(vec![2]));
    assert_eq!(
        evaluate(&checked(not_a_cell.clone(), not_a_record.clone())),
        Value::record([
            (vocabulary::ABSENT, vocabulary::INVALID_CHECK.into()),
            (vocabulary::REASON, not_a_cell),
            (vocabulary::ARGUMENTS, not_a_record),
        ])
    );
}
