//! A UTF-8 text convention over ordinary graph data. Text is a
//! positively recognized record facet, not a graph-core atom.

use progred_graph::{Label, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    pub const UTF8: CellId = CellId::from_u128(0x82a7c1824bc441ec8bfcdc50a2d06a6a);
}

pub fn value(text: impl AsRef<str>) -> Value {
    Value::record([(
        Label::from(vocabulary::UTF8),
        Value::from(text.as_ref().as_bytes().to_vec()),
    )])
}

pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&Label::from(vocabulary::UTF8))?
        .as_blob()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::{Label, new_cell_id};

    #[test]
    fn utf8_is_an_open_convention_over_bytes() {
        assert_eq!(read(&value("hello")), Some("hello"));
        assert_eq!(read(&Value::from(vec![0xff])), None);

        let enriched = Value::record(
            value("hello")
                .as_record()
                .unwrap()
                .clone()
                .update(Label::from(new_cell_id()), Value::from(vec![1])),
        );
        assert_eq!(read(&enriched), Some("hello"));
    }
}
