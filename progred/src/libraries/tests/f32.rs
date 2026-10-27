use super::*;
use gid::Value;
use gid::new_cell_id;

#[test]
fn representation_is_open_library_data() {
    assert_eq!(read(&value(1.25)), Some(1.25));
    assert_eq!(read(&Value::from(1.25_f32.to_le_bytes().to_vec())), None);

    let metadata = new_cell_id();
    let enriched = Value::record(
        value(1.25)
            .as_record()
            .unwrap()
            .clone()
            .update(metadata, Value::from(vec![1])),
    );
    assert_eq!(read(&enriched), Some(1.25));
}

#[test]
fn spelling_round_trips_through_the_line_conversion() {
    assert_eq!(
        number::edit("3.5", None, value).as_ref().and_then(read),
        Some(3.5)
    );
}
