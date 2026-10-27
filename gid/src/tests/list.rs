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
    original.insert(first.clone(), blob("changed"));
    assert!(!Arc::ptr_eq(&original.0, &snapshot.0));
    assert_eq!(original.get(&first), Some(&blob("changed")));
    assert_eq!(snapshot.get(&first), Some(&blob("first")));
}
