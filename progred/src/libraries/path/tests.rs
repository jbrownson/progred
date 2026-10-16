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
