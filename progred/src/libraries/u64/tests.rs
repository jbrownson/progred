use super::*;
use gid::new_cell_id;

#[test]
fn representation_is_open_library_data() {
    let extra = new_cell_id();
    let current = Value::record(
        value(u64::MAX)
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(b"metadata".to_vec())),
    );
    let update = |input: &str| number::edit(input, Some(&current), value);

    assert_eq!(read(&current), Some(u64::MAX));
    assert_eq!(
        update("42"),
        Some(Value::record(
            value(42)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"metadata".to_vec())),
        ))
    );
    assert_eq!(update("-1"), None);
}

#[test]
fn scrubbing_is_exact_at_the_unsigned_bounds() {
    assert_eq!(<u64 as number::Scrubbable>::from_offset(0, -100.0, 1.0), 0,);
    assert_eq!(
        <u64 as number::Scrubbable>::from_offset(u64::MAX, 100.0, 1.0),
        u64::MAX,
    );
    assert_eq!(
        <u64 as number::Scrubbable>::from_offset(u64::MAX, -1.0, 1.0),
        u64::MAX - 1,
    );
}
