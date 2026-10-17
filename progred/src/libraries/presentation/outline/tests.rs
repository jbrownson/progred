use super::*;

#[test]
fn outline_order_preserves_repeated_occurrences_and_unrelated_fields() {
    let a = CellId::from_u128(1);
    let b = CellId::from_u128(2);
    let value = Value::record([
        (OUTLINE, Value::list([b.into(), a.into(), b.into()])),
        (a, Value::record([])),
    ]);
    assert_eq!(sections(&value), Some(vec![b, a, b]));
    assert!(!value.as_record().unwrap().contains_key(&b));
    assert!(
        sections(&Value::record([(
            OUTLINE,
            Value::list([Value::record([])])
        )]))
        .is_none()
    );
}

mod frames;
