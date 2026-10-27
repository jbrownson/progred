use super::*;
use crate::position::spread;
use crate::{List, Position, new_cell_id};

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
    original.insert(x, blob("changed"));
    assert!(!Arc::ptr_eq(&original.0, &snapshot.0));
    assert_eq!(original.get(&x), Some(&blob("changed")));
    assert_eq!(snapshot.get(&x), Some(&blob("x")));
}

#[test]
fn entries_sort_by_key_and_a_repeated_key_keeps_its_last_value() {
    let (a, b) = (new_cell_id(), new_cell_id());
    let (low, high) = (a.min(b), a.max(b));
    let record = Value::record([(high, blob("1")), (low, blob("2")), (high, blob("3"))]);
    assert_eq!(
        record
            .as_record()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        [(low, blob("2")), (high, blob("3"))]
    );
    let [first, second] = <[Position; 2]>::try_from(spread(2)).unwrap();
    let list: List = [
        (second.clone(), blob("x")),
        (first.clone(), blob("y")),
        (second.clone(), blob("z")),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        list.iter().cloned().collect::<Vec<_>>(),
        [(first, blob("y")), (second, blob("z"))]
    );
}
