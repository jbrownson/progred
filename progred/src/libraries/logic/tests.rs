use super::*;

#[test]
fn booleans_are_open_inert_values_not_evaluated_cell_descriptions() {
    let library = library();
    for expected in [true, false] {
        let extra = gid::new_cell_id();
        let boolean = Value::record(
            value(expected)
                .as_record()
                .unwrap()
                .update(extra, Value::record([])),
        );
        assert_eq!(read(&boolean), Some(expected));
        let evaluated = crate::libraries::test_evaluate(
            &boolean,
            |id| library.value(id).cloned(),
            &functions(),
            10,
        );
        assert_eq!(evaluated.result, boolean);
        let cell = if expected {
            vocabulary::TRUE
        } else {
            vocabulary::FALSE
        };
        assert_eq!(read(&cell.into()), None);
        assert_eq!(read(library.value(cell).unwrap()), None);
    }
    assert_eq!(
        read(&Value::record([(
            vocabulary::BOOL,
            gid::new_cell_id().into()
        )])),
        None
    );
}

#[test]
fn require_accepts_true_rejects_false_and_preserves_absents() {
    let failure =
        ::grap::absent::with_detail(gid::new_cell_id(), vocabulary::CONDITION, Value::record([]));
    for (condition, expected) in [
        (value(true), Value::record([])),
        (
            value(false),
            ::grap::absent::with_detail(
                vocabulary::CONDITION_NOT_MET,
                vocabulary::CONDITION,
                value(false),
            ),
        ),
        (
            Value::record([]),
            ::grap::absent::with_detail(
                vocabulary::NOT_BOOL,
                vocabulary::CONDITION,
                Value::record([]),
            ),
        ),
        (failure.clone(), failure),
    ] {
        let expression = ::grap::call(
            vocabulary::REQUIRE.into(),
            [(vocabulary::CONDITION, condition)],
        );
        let result = crate::libraries::test_evaluate(&expression, |_| None, &functions(), 20);
        assert!(result.completed);
        assert_eq!(result.result, expected);
    }
    let missing = crate::libraries::test_evaluate(
        &::grap::call(vocabulary::REQUIRE.into(), []),
        |_| None,
        &functions(),
        20,
    );
    assert_eq!(
        absent::reason(&missing.result),
        Some(::grap::absent::MISSING_ARGUMENT)
    );
}
