//! Paths as ordinary GID data, interpreted in a caller-supplied root
//! and resolution context. List positions retain their session lifetime.

use crate::libraries::{Library, absent, name};
#[cfg(test)]
pub use ::grap::path::step_value;
pub use ::grap::path::{read, read_step, value};
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x7f18fc9a3362e8c7e4cd7e21501a71f6);

pub mod vocabulary {
    pub use ::grap::path::vocabulary::*;
    use gid::CellId;

    pub const INVALID_PATH: CellId = CellId::from_u128(0xda56222f270f28fa7f18a02f0c277f47);
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
        crate::display::runtime_partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Resolution, Step, Value};

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
