//! A simple name on a record. The field is ordinary GID data. This
//! editor assumes the library; other hosts need not.

use gid::{CellId, Cells, Value};

pub mod vocabulary {
    use gid::CellId;

    pub const NAME: CellId = CellId::from_u128(0x02e562654d6d0828d3a7559e6f75fffe);
}

pub fn field(name: impl Into<String>) -> (CellId, Value) {
    (vocabulary::NAME, progred_text::value(name))
}

pub fn record(name: impl Into<String>, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(std::iter::once(field(name)).chain(fields))
}

pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&vocabulary::NAME)
        .and_then(progred_text::read)
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::NAME, record("name", []));
    cells.set_value(progred_text::vocabulary::UTF8, record("utf8", []));
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn names_are_extensible_ordinary_record_data() {
        let mut fields = record("roof", []).as_record().unwrap().clone();
        fields.insert(new_cell_id(), progred_text::value("anything"));
        assert_eq!(read(&Value::Record(fields)), Some("roof"));
        assert_eq!(read(&progred_text::value("roof")), None);

        let empty = record("", []);
        assert_eq!(read(&empty), Some(""));
        assert_eq!(
            empty
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
