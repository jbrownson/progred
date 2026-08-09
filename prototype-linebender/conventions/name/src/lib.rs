//! An optional convention for attaching one nonempty, simple
//! human-readable name to a record. The field remains ordinary graph
//! data: languages and domain projections may use other naming
//! structures or compute displays.

use progred_graph::{CellId, Cells, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    /// The original randomly minted `name` identity, now restored as
    /// an ordinary relation rather than a graph-core feature.
    pub const NAME: CellId = CellId::from_u128(0xf8acc21e36354e5a97021ee48d29fed8);
}

pub fn field(name: impl Into<String>) -> (CellId, Value) {
    (vocabulary::NAME, progred_text::value(name))
}

pub fn value(name: impl Into<String>) -> Value {
    Value::record([field(name)])
}

pub fn record(name: impl Into<String>, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(std::iter::once(field(name)).chain(fields))
}

pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&vocabulary::NAME)
        .and_then(progred_text::read)
        .filter(|name| !name.is_empty())
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::NAME, value("name"));
    cells.set_value(progred_text::vocabulary::UTF8, value("utf8"));
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    #[test]
    fn names_are_extensible_ordinary_record_data() {
        let mut fields = value("roof").as_record().unwrap().clone();
        fields.insert(new_cell_id(), progred_text::value("anything"));
        assert_eq!(read(&Value::Record(fields)), Some("roof"));
        assert_eq!(read(&progred_text::value("roof")), None);

        let unnamed = value("");
        assert_eq!(read(&unnamed), None);
        assert_eq!(
            unnamed
                .as_record()
                .unwrap()
                .get(&vocabulary::NAME)
                .and_then(progred_text::read),
            Some("")
        );
    }

    #[test]
    fn the_name_relation_describes_itself_without_core_support() {
        let library = library();
        assert_eq!(library.value(vocabulary::NAME).and_then(read), Some("name"));
        assert_eq!(
            library.value(progred_text::vocabulary::UTF8).and_then(read),
            Some("utf8")
        );
    }
}
