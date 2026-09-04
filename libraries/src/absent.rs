//! Tagged Grap absence values. The tag identifies the result as absent;
//! its cell payload is the stable, language-independent reason identity.

use crate::{Library, name};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0x873c68ac371dbbb98a4f198546d60241);

pub mod vocabulary {
    use gid::CellId;

    pub const ABSENT: CellId = grap_runtime::absent::ABSENT;
    pub const CELL: CellId = grap_runtime::absent::CELL;
    pub const VALUE: CellId = grap_runtime::absent::VALUE;
    pub const CYCLE: CellId = grap_runtime::absent::CYCLE;
    pub const CAUSES: CellId = grap_runtime::absent::CAUSES;
    pub const NO_ALTERNATIVE: CellId = grap_runtime::absent::NO_ALTERNATIVE;
    pub const UNSPECIFIED: CellId = CellId::from_u128(0x017c4e09bedca389122e5da48156b229);
}

pub fn value() -> Value {
    with_reason(vocabulary::UNSPECIFIED)
}

pub fn with_reason(reason: CellId) -> Value {
    grap_runtime::absent::value(reason)
}

pub fn reason(value: &Value) -> Option<CellId> {
    grap_runtime::absent::reason(value)
}

pub fn is_absent(value: &Value) -> bool {
    grap_runtime::absent::is_absent(value)
}

pub fn from_causes(causes: impl IntoIterator<Item = Value>) -> Value {
    grap_runtime::absent::from_causes(causes)
}

pub fn named_reason(value: impl Into<String>) -> Value {
    name::record(value, [])
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ABSENT, name::record("absent", []));
    cells.set_value(vocabulary::CELL, name::record("cell", []));
    cells.set_value(vocabulary::VALUE, name::record("value", []));
    cells.set_value(vocabulary::CYCLE, name::record("cycle", []));
    cells.set_value(vocabulary::CAUSES, name::record("causes", []));
    cells.set_value(
        vocabulary::NO_ALTERNATIVE,
        named_reason("no applicable alternative"),
    );
    cells.set_value(vocabulary::UNSPECIFIED, named_reason("unspecified absence"));
    Library::named(
        "absent",
        crate::Definitions::from_parts(cells, Default::default()),
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn absence_is_an_open_tag_with_a_stable_reason() {
        let reason = new_cell_id();
        let extra = new_cell_id();
        let absent = Value::record(
            with_reason(reason)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(vec![1])),
        );

        assert!(is_absent(&absent));
        assert_eq!(super::reason(&absent), Some(reason));
        assert_eq!(super::reason(&value()), Some(vocabulary::UNSPECIFIED));
        assert!(!is_absent(&named_reason("specific absence")));
        assert!(!is_absent(&Value::record([(
            vocabulary::ABSENT,
            Value::from(vec![1]),
        )])));
    }

    #[test]
    fn multiple_causes_form_an_ordered_absence_while_one_remains_itself() {
        let first = with_reason(new_cell_id());
        let second = with_reason(new_cell_id());

        assert_eq!(from_causes([first.clone()]), first);
        let combined = from_causes([first.clone(), second.clone()]);
        assert_eq!(reason(&combined), Some(vocabulary::NO_ALTERNATIVE));
        assert_eq!(
            combined
                .as_record()
                .and_then(|fields| fields.get(&vocabulary::CAUSES)),
            Some(&Value::list([first, second])),
        );
    }
}
