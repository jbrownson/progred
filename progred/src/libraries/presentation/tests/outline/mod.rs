use super::*;

#[test]
fn outline_order_preserves_repeated_occurrences_and_unrelated_fields() {
    let a = CellId::from_u128(1);
    let b = CellId::from_u128(2);
    let value = Value::record([
        (::grap::isa::ISA, kind::OUTLINE.into()),
        (SECTIONS, Value::list([b.into(), a.into(), b.into()])),
        (a, Value::record([])),
    ]);
    assert_eq!(sections(&value), Some(vec![b, a, b]));
    assert_eq!(value.as_record().unwrap().get(&b), None);
    assert!(
        sections(&Value::record([
            (::grap::isa::ISA, kind::OUTLINE.into()),
            (SECTIONS, Value::list([Value::record([])])),
        ]))
        .is_none()
    );
    assert!(
        sections(&Value::record([(SECTIONS, Value::list([a.into()]))])).is_none(),
        "a record that isn't an outline keeps its sections field as data"
    );
}

mod frames;
