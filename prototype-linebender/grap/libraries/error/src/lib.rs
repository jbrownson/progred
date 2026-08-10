//! The shared Grap error classification. Particular libraries own
//! their error identities; their values classify those identities as
//! errors through the independent `isa` convention.

use progred_graph::{Cells, Value};
use progred_isa::Isa as _;

pub mod vocabulary {
    use progred_graph::CellId;

    pub const ERROR: CellId = CellId::from_u128(0x06a183f34bdb188a226cd26bf2b4471b);
}

pub fn value() -> Value {
    Value::record([progred_isa::field(vocabulary::ERROR)])
}

pub fn named(name: impl Into<String>) -> Value {
    progred_name::record(name, [progred_isa::field(vocabulary::ERROR)])
}

pub fn is_error(value: &Value) -> bool {
    value.isa(vocabulary::ERROR)
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ERROR, progred_name::record("error", []));
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    #[test]
    fn error_is_an_extensible_structural_classification() {
        let mut fields = value().as_record().unwrap().clone();
        fields.insert(new_cell_id(), Value::from(vec![1]));
        assert!(is_error(&Value::Record(fields)));
        assert!(is_error(&named("specific failure")));
        assert_eq!(
            progred_name::read(&named("specific failure")),
            Some("specific failure")
        );
        assert!(!is_error(&Value::from(vocabulary::ERROR)));
    }
}
