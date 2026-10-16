use super::*;
use crate::new_cell_id;

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

#[test]
fn values_are_the_whole_cell_statement() {
    let mut cells = Cells::new();
    let cell = new_cell_id();
    let x = new_cell_id();
    assert!(cells.value(cell).is_none());

    cells.set_value(cell, Value::record([(x, blob("1"))]));
    assert!(matches!(cells.value(cell), Some(Value::Record(_))));
    cells.set_value(cell, Value::list([blob("a")]));
    assert_eq!(cells.value(cell), Some(&Value::list([blob("a")])));
    cells.clear_value(cell);
    assert!(cells.value(cell).is_none());
}

#[test]
fn noop_removal_keeps_ptr_eq_honest() {
    let mut cells = Cells::new();
    cells.set_value(new_cell_id(), blob("x"));
    let before = cells.clone();
    cells.remove(new_cell_id());
    assert!(cells.ptr_eq(&before));
}

#[test]
fn merge_is_left_biased_per_cell() {
    let shared = new_cell_id();
    let fresh = new_cell_id();
    let mut mine = Cells::new();
    mine.set_value(shared, blob("mine"));
    let mut other = Cells::new();
    other.set_value(shared, blob("theirs"));
    other.set_value(fresh, blob("new"));

    mine.merge(other);
    assert_eq!(mine.value(shared), Some(&blob("mine")));
    assert_eq!(mine.value(fresh), Some(&blob("new")));
}

#[test]
fn the_table_serializes_as_cell_to_value() {
    let mut cells = Cells::new();
    let first = new_cell_id();
    let second = new_cell_id();
    let key = new_cell_id();
    cells.set_value(first, Value::from(vec![0x66, 0x33, 0x99]));
    cells.set_value(second, Value::record([(key, Value::from(first))]));

    let json = serde_json::to_string(&cells).unwrap();
    let loaded: Cells = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, cells);
    assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
    assert!(!json.contains("\"value\""));
}
