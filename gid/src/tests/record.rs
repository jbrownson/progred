use super::*;
use crate::{Value, new_cell_id};
use std::sync::Arc;

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
