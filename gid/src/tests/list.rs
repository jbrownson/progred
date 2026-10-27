use super::*;
use crate::position::spread;

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

#[test]
fn lists_are_canonical_copy_on_write_sequences() {
    let [first, second, third] = spread(3).try_into().unwrap();
    let mut original: List = [
        (third.clone(), blob("third")),
        (first.clone(), blob("first")),
        (second.clone(), blob("old second")),
        (second.clone(), blob("second")),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        original.keys().cloned().collect::<Vec<_>>(),
        [first.clone(), second.clone(), third]
    );
    assert_eq!(original.get(&second), Some(&blob("second")));

    let snapshot = original.clone();
    assert!(Arc::ptr_eq(&original.0, &snapshot.0));
    original.set(&first, blob("changed"));
    assert!(!Arc::ptr_eq(&original.0, &snapshot.0));
    assert_eq!(original.get(&first), Some(&blob("changed")));
    assert_eq!(snapshot.get(&first), Some(&blob("first")));
}

#[test]
fn collected_values_get_spread_positions() {
    let elements: List = (0..100).map(|i| blob(&i.to_string())).collect();
    assert_eq!(elements.len(), 100);
    let positions: Vec<_> = elements.keys().cloned().collect();
    assert_eq!(positions, spread(100));
    let values: Vec<_> = elements.values().cloned().collect();
    assert_eq!(values[3], blob("3"));
}
