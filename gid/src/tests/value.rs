use super::*;
use crate::position::between;
use crate::{CellId, new_cell_id};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn relation(name: &str) -> CellId {
    CellId::from_u128(match name {
        "x" => 0xebbb03b25e12960d230b25badc553723,
        "y" => 0x7824623804db8467097bdddbf3518394,
        "name" => 0xc19a573c2534703d0797bb163528547c,
        "at" => 0xd6448992e5df9057033a15ff4396f43e,
        "row" => 0x811d61c56fb3341c9247363be492d680,
        "k" => 0x2be2251626ad23562583b8af8649e746,
        "a" => 0x1ca0184130075343f0d121acc948215f,
        "b" => 0xc882fd0d8c251a0b6f0d97306bd87886,
        _ => unreachable!("fixture relation"),
    })
}

fn label(name: &str) -> CellId {
    relation(name)
}

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

fn hash_of(value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn list_equality_ignores_positions() {
    let a = Value::list([blob("x"), blob("y")]);
    // The same sequence under entirely different positions: an
    // appended-then-prepended construction.
    let first = between(None, None).unwrap();
    let second = between(Some(&first), None).unwrap();
    let b = Value::List(
        [(first, blob("x")), (second, blob("y"))]
            .into_iter()
            .collect(),
    );
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));

    assert_ne!(a, Value::list([blob("y"), blob("x")]));
    assert_ne!(a, Value::list([blob("x")]));
    assert_ne!(a, blob("x"));
    // Nested lists compare structurally too.
    assert_eq!(Value::list([a.clone()]), Value::list([b.clone()]));
    // Comparison stops at links: equal links, not equal linked
    // values.
    let cell = new_cell_id();
    assert_eq!(
        Value::list([Value::from(cell)]),
        Value::list([Value::from(cell)]),
    );
    assert_ne!(
        Value::list([Value::from(cell)]),
        Value::list([Value::from(new_cell_id())]),
    );
}

#[test]
fn records_are_content_compared_values() {
    let a = Value::record([(label("x"), blob("1")), (label("y"), blob("2"))]);
    let b = Value::record([(label("y"), blob("2")), (label("x"), blob("1"))]);
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_ne!(a, Value::record([(label("x"), blob("1"))]));
    assert_ne!(a, Value::record([]));
    assert_ne!(Value::record([]), Value::list([]));
    // Equal inline records nest equally.
    assert_eq!(Value::list([a.clone()]), Value::list([b.clone()]));
}

#[test]
fn blobs_are_their_bytes() {
    assert_eq!(Value::from(vec![0xde, 0xad]), Value::from(vec![0xde, 0xad]));
    assert_ne!(Value::from(vec![0xde, 0xad]), Value::from(vec![0xad, 0xde]));
    assert_eq!(Value::from(vec![0xde]).as_blob(), Some(&[0xde_u8][..]));
}
