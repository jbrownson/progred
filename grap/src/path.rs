//! Ordinary GID path data, shared with the editor's path library.
//! These records are a convention, not new GID atoms or evaluator forms.

use gid::{Path, Position, Resolution, Step, Value};

pub mod vocabulary {
    use gid::CellId;

    pub const KEY: CellId = CellId::from_u128(0xf12dea12c741fe36312750a264f3a235);
    pub const ELEMENT: CellId = CellId::from_u128(0x2eb44bbe78bbb0e96af4a9cbf6e949e1);
    pub const FOLLOW: CellId = CellId::from_u128(0x33c6fb363ddc6f13fd054c68f7a37a98);
    pub const DOCUMENT: CellId = CellId::from_u128(0xef62fa62f008c1eca70389571933aba0);
    pub const LIBRARY: CellId = CellId::from_u128(0x0f1b1da59ceff3c9cfb3aaefe39466ed);
    pub const INDEXABLE: CellId = CellId::from_u128(0xbd69ea5a7f839fe98dab761c01c22c13);
}

pub fn value(path: &[Step]) -> Value {
    Value::list(path.iter().map(step_value))
}

pub fn read(value: &Value) -> Option<Path> {
    value.as_list()?.values().map(read_step).collect()
}

pub fn resolution_value(source: Resolution) -> Value {
    match source {
        Resolution::Document => vocabulary::DOCUMENT.into(),
        Resolution::Library(cell) => Value::record([(vocabulary::LIBRARY, cell.into())]),
    }
}

pub fn read_resolution(value: &Value) -> Option<Resolution> {
    if value.as_cell() == Some(vocabulary::DOCUMENT) {
        Some(Resolution::Document)
    } else {
        Some(Resolution::Library(
            value.as_record()?.get(&vocabulary::LIBRARY)?.as_cell()?,
        ))
    }
}

pub fn step_value(step: &Step) -> Value {
    use vocabulary::*;
    Value::record([match step {
        Step::Key(cell) => (KEY, (*cell).into()),
        Step::Element(position) => (
            ELEMENT,
            Value::record([(INDEXABLE, Value::from(position.as_bytes().to_vec()))]),
        ),
        Step::Follow(source) => (FOLLOW, resolution_value(*source)),
    }])
}

pub fn read_step(value: &Value) -> Option<Step> {
    use vocabulary::*;
    let fields = value.as_record()?;
    match (fields.get(&KEY), fields.get(&ELEMENT), fields.get(&FOLLOW)) {
        (Some(key), None, None) => key.as_cell().map(Step::Key),
        (None, Some(element), None) => {
            Position::from_bytes(element.as_record()?.get(&INDEXABLE)?.as_blob()?.to_vec())
                .map(Step::Element)
        }
        (None, None, Some(source)) => read_resolution(source).map(Step::Follow),
        _ => None,
    }
}
