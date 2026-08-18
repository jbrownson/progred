//! Structural classification of GID values (`isa`). Independent of
//! Grap and of the GID core. This editor assumes the library.

use crate::{Library, name};
use gid::{CellId, Cells, Value};

pub mod vocabulary {
    use gid::CellId;

    pub const ISA: CellId = CellId::from_u128(0xdcedb3a466c3a3e2c6b2826153e82e78);
}

pub fn field(class: CellId) -> (CellId, Value) {
    (vocabulary::ISA, Value::from(class))
}

pub fn read(value: &Value) -> Option<CellId> {
    value
        .as_record()
        .and_then(|fields| fields.get(&vocabulary::ISA))
        .and_then(Value::as_cell)
}

pub trait Isa {
    fn isa(&self, class: CellId) -> bool;
}

impl Isa for Value {
    fn isa(&self, class: CellId) -> bool {
        read(self) == Some(class)
    }
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ISA, name::record("isa", []));
    Library {
        cells,
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn classification_is_structural_and_extensible() {
        let class = new_cell_id();
        let mut fields = Value::record([field(class)]).as_record().unwrap().clone();
        fields.insert(new_cell_id(), Value::from(vec![1]));
        let classified = Value::Record(fields);
        assert_eq!(read(&classified), Some(class));
        assert!(classified.isa(class));
        assert!(!Value::from(class).isa(class));
    }
}
