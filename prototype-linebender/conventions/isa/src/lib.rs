//! An optional semantic convention for structurally classifying
//! Progred graph values. It is independent of Grap and of the graph
//! data model itself.

use progred_graph::{CellId, Cells, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    pub const ISA: CellId = CellId::from_u128(0x1414c6e28ba705fc08e89a5f74942fac);
}

pub fn value(class: CellId) -> Value {
    Value::record([(vocabulary::ISA, Value::from(class))])
}

pub fn is(value: &Value, class: CellId) -> bool {
    value
        .as_record()
        .and_then(|fields| fields.get(&vocabulary::ISA))
        .and_then(Value::as_cell)
        == Some(class)
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ISA, progred_name::value("isa"));
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    #[test]
    fn classification_is_structural_and_extensible() {
        let class = new_cell_id();
        let mut fields = value(class).as_record().unwrap().clone();
        fields.insert(new_cell_id(), Value::from(vec![1]));
        assert!(is(&Value::Record(fields), class));
        assert!(!is(&Value::from(class), class));
    }
}
