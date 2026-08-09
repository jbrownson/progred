//! The shared Grap error classification. Particular libraries own
//! their error identities; their values classify those identities as
//! errors through the independent `isa` convention.

use progred_graph::{Cells, Label, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    pub const ERROR: CellId = CellId::from_u128(0x4a8006a54b0500e2ede24cea0d5ca4ab);
}

pub fn value() -> Value {
    progred_isa::value(vocabulary::ERROR)
}

pub fn named(name: impl Into<String>) -> Value {
    progred_name::record(
        name,
        [(
            Label::Cell(progred_isa::vocabulary::ISA),
            Value::from(vocabulary::ERROR),
        )],
    )
}

pub fn is_error(value: &Value) -> bool {
    progred_isa::is(value, vocabulary::ERROR)
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ERROR, progred_name::value("error"));
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_is_an_extensible_structural_classification() {
        let mut fields = value().as_record().unwrap().clone();
        fields.insert(Label::from("detail"), Value::from("anything"));
        assert!(is_error(&Value::Record(fields)));
        assert!(is_error(&named("specific failure")));
        assert_eq!(
            progred_name::read(&named("specific failure")),
            Some("specific failure")
        );
        assert!(!is_error(&Value::from(vocabulary::ERROR)));
    }
}
