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

#[test]
fn values_round_trip_through_json() {
    let cell = new_cell_id();
    let cases = [
        Value::from(cell),
        blob("hello"),
        Value::from(vec![0x89, 0x50, 0x4e, 0x47]),
        Value::from(Vec::<u8>::new()),
        Value::list([]),
        Value::record([]),
        Value::record([
            (label("name"), blob("roof")),
            (cell, Value::list([blob("a")])),
            (label("at"), Value::record([(label("row"), blob("top"))])),
        ]),
    ];
    for value in cases {
        let json = serde_json::to_string(&value).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value, "{json}");
        // Save → load → save is a fixed point even though minted
        // positions differ: this form keeps only the order.
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }
    assert_eq!(
        serde_json::to_string(&Value::from(vec![0xde, 0xad])).unwrap(),
        r#"{"blob":"dead"}"#
    );
    assert_eq!(
        serde_json::to_string(&Value::from(CellId::from_u128(
            0x00112233445566778899aabbccddeeff,
        )))
        .unwrap(),
        r#"{"cell":"00112233445566778899aabbccddeeff"}"#
    );
    let record_json = serde_json::to_string(&Value::record([(label("k"), blob("v"))])).unwrap();
    assert!(record_json.contains(&label("k").to_string()));
    assert!(!record_json.contains("\"string\""));
}

#[test]
fn record_pairs_serialize_in_label_order() {
    let cell = new_cell_id();
    let value = Value::record([
        (label("b"), blob("2")),
        (label("a"), blob("1")),
        (cell, blob("0")),
    ]);
    let json = serde_json::to_value(&value).unwrap();
    let labels: Vec<String> = json["record"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| serde_json::to_string(&pair[0]).unwrap())
        .collect();
    let mut sorted = labels.clone();
    sorted.sort();
    assert_eq!(labels, sorted);
}

#[test]
fn malformed_spellings_refuse() {
    // Blob hex is strict: lowercase, even length.
    assert!(serde_json::from_str::<Value>(r#"{"blob":"DEAD"}"#).is_err());
    assert!(serde_json::from_str::<Value>(r#"{"blob":"abc"}"#).is_err());
    assert!(serde_json::from_str::<Value>(r#"{"blob":"zz"}"#).is_err());
    // The removed string representation is rejected for values,
    // and record keys must deserialize as cell ids.
    assert!(serde_json::from_str::<Value>(r#"{"string":"v"}"#).is_err());
    assert!(
        serde_json::from_str::<Value>(r#"{"record":[[{"blob":"00"},{"string":"v"}]]}"#).is_err()
    );
    assert!(serde_json::from_str::<Value>(r#"{"record":[[{"list":[]},{"string":"v"}]]}"#).is_err());
    // Numbers left the data model.
    assert!(serde_json::from_str::<Value>(r#"{"number":1.0}"#).is_err());
    assert!(serde_json::from_str::<CellId>(r#"{"number":1.0}"#).is_err());
}

#[test]
fn values_display_through_their_parts() {
    let cell = CellId::from_u128(0x00112233445566778899aabbccddeeff);
    let value = Value::record([(
        label("k"),
        Value::list([Value::from(vec![0xde, 0xad]), Value::from(cell)]),
    )]);
    assert_eq!(
        value.to_string(),
        format!("{{{}: [0xdead, {cell}]}}", label("k"))
    );
}
