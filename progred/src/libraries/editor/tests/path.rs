use super::*;
use gid::{Step, Value};

#[test]
fn paths_round_trip_with_follows_and_list_positions() {
    let key = gid::new_cell_id();
    let position = gid::position::between(None, None).unwrap();
    let path = vec![
        Step::Key(key),
        Step::Follow,
        Step::Element(position),
        Step::Follow,
    ];
    assert_eq!(read(&value(&path)), Some(path));
    assert_eq!(read(&value(&[])), Some(vec![]));
}

#[test]
fn a_follow_saved_with_its_source_reads_as_a_follow() {
    use vocabulary::*;
    let library = Value::record([(gid::new_cell_id(), gid::new_cell_id().into())]);
    for source in [gid::new_cell_id().into(), library] {
        assert_eq!(
            read(&Value::list([Value::record([(FOLLOW, source)])])),
            Some(vec![Step::Follow])
        );
    }
}

#[test]
fn decoding_rejects_ambiguous_steps_and_noncanonical_positions() {
    use vocabulary::*;
    let key = gid::new_cell_id();
    for step in [
        Value::record([(KEY, key.into()), (FOLLOW, Value::record([]))]),
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
