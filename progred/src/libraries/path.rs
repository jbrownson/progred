//! Paths as ordinary GID data, interpreted in a caller-supplied root
//! and resolution context. List positions retain their session lifetime.

use crate::libraries::{Library, absent, name};
use gid::{CellId, Path, Position, Resolution, Step, Value};

pub const ID: CellId = CellId::from_u128(0x7f18fc9a3362e8c7e4cd7e21501a71f6);

pub mod vocabulary {
    use gid::CellId;

    pub const KEY: CellId = CellId::from_u128(0xf12dea12c741fe36312750a264f3a235);
    pub const ELEMENT: CellId = CellId::from_u128(0x2eb44bbe78bbb0e96af4a9cbf6e949e1);
    pub const FOLLOW: CellId = CellId::from_u128(0x33c6fb363ddc6f13fd054c68f7a37a98);
    pub const DOCUMENT: CellId = CellId::from_u128(0xef62fa62f008c1eca70389571933aba0);
    pub const LIBRARY: CellId = CellId::from_u128(0x0f1b1da59ceff3c9cfb3aaefe39466ed);
    pub const INDEXABLE: CellId = CellId::from_u128(0xbd69ea5a7f839fe98dab761c01c22c13);
    pub const INVALID_PATH: CellId = CellId::from_u128(0xda56222f270f28fa7f18a02f0c277f47);
}

pub fn value(path: &[Step]) -> Value {
    Value::list(path.iter().map(step_value))
}

pub fn read(value: &Value) -> Option<Path> {
    value.as_list()?.values().map(read_step).collect()
}

pub fn step_value(step: &Step) -> Value {
    use vocabulary::*;
    Value::record([match step {
        Step::Key(cell) => (KEY, (*cell).into()),
        Step::Element(position) => (
            ELEMENT,
            Value::record([(INDEXABLE, Value::from(position.as_bytes().to_vec()))]),
        ),
        Step::Follow(source) => (
            FOLLOW,
            match source {
                Resolution::Document => DOCUMENT.into(),
                Resolution::Library(cell) => Value::record([(LIBRARY, (*cell).into())]),
            },
        ),
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
        (None, None, Some(source)) => Some(Step::Follow(if source.as_cell() == Some(DOCUMENT) {
            Resolution::Document
        } else {
            Resolution::Library(source.as_record()?.get(&LIBRARY)?.as_cell()?)
        })),
        _ => None,
    }
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    use vocabulary::*;
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (KEY, "key"),
        (ELEMENT, "element"),
        (FOLLOW, "follow"),
        (DOCUMENT, "document"),
        (LIBRARY, "library"),
        (INDEXABLE, "indexable"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    cells.set_value(INVALID_PATH, absent::named_reason("invalid path"));
    Library::named(
        ID,
        "path",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip_with_definition_sources_and_list_positions() {
        let key = gid::new_cell_id();
        let library = gid::new_cell_id();
        let position = gid::position::between(None, None).unwrap();
        let path = vec![
            Step::Key(key),
            Step::Follow(Resolution::Library(library)),
            Step::Element(position),
            Step::Follow(Resolution::Document),
        ];
        assert_eq!(read(&value(&path)), Some(path));
        assert_eq!(read(&value(&[])), Some(vec![]));
    }

    #[test]
    fn decoding_rejects_ambiguous_steps_and_noncanonical_positions() {
        use vocabulary::*;
        let key = gid::new_cell_id();
        for step in [
            Value::record([(KEY, key.into()), (FOLLOW, DOCUMENT.into())]),
            Value::record([(FOLLOW, key.into())]),
            Value::record([(KEY, Value::list([]))]),
            Value::record([]),
        ] {
            assert_eq!(read(&Value::list([step])), None);
        }
        for bytes in [vec![], vec![0], vec![0x80, 0]] {
            assert_eq!(
                read(&Value::list([Value::record([(
                    ELEMENT,
                    Value::record([(INDEXABLE, bytes.into())]),
                )])])),
                None,
            );
        }
        let extended = Value::record([
            (KEY, key.into()),
            (
                crate::libraries::name::vocabulary::NAME,
                crate::libraries::text::value("metadata"),
            ),
        ]);
        assert_eq!(read(&Value::list([extended])), Some(vec![Step::Key(key)]));
    }
}
