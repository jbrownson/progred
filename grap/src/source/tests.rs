use super::*;
use gid::{Resolution, Step, new_cell_id};

#[test]
fn origins_round_trip_with_open_metadata_and_stable_paths() {
    let path = vec![
        Step::Key(new_cell_id()),
        Step::Element(gid::position::between(None, None).unwrap()),
    ];
    for origin in [
        SourceOrigin::Input(path.clone()),
        SourceOrigin::Stored(path.clone()),
        SourceOrigin::Cell {
            cell: new_cell_id(),
            source: Resolution::Document,
            path: path.clone(),
        },
        SourceOrigin::Cell {
            cell: new_cell_id(),
            source: Resolution::Library(new_cell_id()),
            path,
        },
    ] {
        let encoded = value(&origin);
        assert_eq!(read(&encoded), Some(origin.clone()));
        let enriched = Value::Record(
            encoded
                .as_record()
                .unwrap()
                .update(new_cell_id(), Value::record([])),
        );
        assert_eq!(read(&enriched), Some(origin));
    }
    assert_eq!(read(&Value::record([])), None);
    assert_eq!(
        read(&Value::record([
            (vocabulary::INPUT, Value::list([])),
            (vocabulary::STORED, Value::list([]))
        ])),
        None
    );
}
