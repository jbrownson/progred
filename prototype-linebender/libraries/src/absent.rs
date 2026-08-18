//! The shared Grap absent classification. Particular libraries own
//! their absent identities; their values classify those identities as
//! absents through the independent `isa` convention.

use crate::{Library, isa, name};
use gid::{Cells, Value};
use isa::Isa as _;

pub mod vocabulary {
    use gid::CellId;

    pub const ABSENT: CellId = CellId::from_u128(0xd9c0a7145a38859a245640d3469cbcd4);
}

pub fn value() -> Value {
    Value::record([isa::field(vocabulary::ABSENT)])
}

pub fn named(value: impl Into<String>) -> Value {
    name::record(value, [isa::field(vocabulary::ABSENT)])
}

pub fn is_absent(value: &Value) -> bool {
    value.isa(vocabulary::ABSENT)
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ABSENT, name::record("absent", []));
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
    fn absent_is_an_extensible_structural_classification() {
        let mut fields = value().as_record().unwrap().clone();
        fields.insert(new_cell_id(), Value::from(vec![1]));
        assert!(is_absent(&Value::Record(fields)));
        assert!(is_absent(&named("specific absence")));
        assert_eq!(
            name::read(&named("specific absence")),
            Some("specific absence")
        );
        assert!(!is_absent(&Value::from(vocabulary::ABSENT)));
    }
}
