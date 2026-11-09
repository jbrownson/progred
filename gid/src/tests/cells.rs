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
