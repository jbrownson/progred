use super::*;
use crate::new_cell_id;

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

#[test]
fn records_are_canonical_copy_on_write_values() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Value>();

    let x = new_cell_id();
    let y = new_cell_id();
    let mut original: Record = [(y, blob("first y")), (x, blob("x")), (y, blob("last y"))]
        .into_iter()
        .collect();
    assert_eq!(original.keys().copied().collect::<Vec<_>>(), {
        let mut labels = vec![x, y];
        labels.sort();
        labels
    });
    assert_eq!(original.get(&y), Some(&blob("last y")));

    let snapshot = original.clone();
    assert!(Arc::ptr_eq(&original.0, &snapshot.0));
    original.set(x, blob("changed"));
    assert!(!Arc::ptr_eq(&original.0, &snapshot.0));
    assert_eq!(original.get(&x), Some(&blob("changed")));
    assert_eq!(snapshot.get(&x), Some(&blob("x")));
}

#[test]
fn extended_fields_replace_those_under_their_labels() {
    let (a, b, k) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut fields: Record = [(a, blob("a")), (b, blob("b"))].into_iter().collect();
    fields.extend([(b, blob("patched")), (k, blob("k"))]);
    assert_eq!(
        fields,
        [(a, blob("a")), (b, blob("patched")), (k, blob("k"))]
            .into_iter()
            .collect()
    );
}

#[test]
fn text_lists_labels_in_any_order_but_once() {
    let (a, b) = (new_cell_id(), new_cell_id());
    let (low, high) = (a.min(b), a.max(b));
    let unordered = serde_json::to_value([(high, blob("2")), (low, blob("1"))]).unwrap();
    assert_eq!(
        serde_json::from_value::<Record>(unordered).unwrap(),
        [(low, blob("1")), (high, blob("2"))].into_iter().collect()
    );
    let repeated = serde_json::to_value([(low, blob("1")), (low, blob("2"))]).unwrap();
    assert!(serde_json::from_value::<Record>(repeated).is_err());
}

#[test]
fn binary_lists_labels_in_increasing_order() {
    let (a, b) = (new_cell_id(), new_cell_id());
    let (low, high) = (a.min(b), a.max(b));
    let ordered = vec![(low, blob("1")), (high, blob("2"))];
    assert_eq!(
        Record::read(ordered.clone(), false),
        Ok(ordered.into_iter().collect())
    );
    assert!(Record::read(vec![(high, blob("2")), (low, blob("1"))], false).is_err());
    assert!(Record::read(vec![(low, blob("1")), (low, blob("2"))], false).is_err());
}

#[test]
fn a_field_is_set_or_dropped_and_returns_what_was_there() {
    let (a, b) = (new_cell_id(), new_cell_id());
    let mut fields: Record = [(a, blob("a"))].into_iter().collect();
    assert_eq!(fields.set(b, blob("b")), None);
    assert_eq!(fields.set(a, blob("changed")), Some(blob("a")));
    assert_eq!(fields.set(b, None), Some(blob("b")));
    assert_eq!(fields.set(b, None), None);
    assert_eq!(fields, [(a, blob("changed"))].into_iter().collect());

    // Writing back what was read changes nothing; `with` leaves the original.
    assert_eq!(fields.with(a, fields.get(&a).cloned()), fields);
    assert_eq!(fields.with(b, fields.get(&b).cloned()), fields);
    assert_eq!(fields.with(a, None), Record::new());
    assert_eq!(fields.get(&a), Some(&blob("changed")));
}
