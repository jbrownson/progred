use super::*;
use crate::selection::resolve_query;
use gid::new_cell_id;

#[test]
fn clipboard_spellings_round_trip() {
    let cell = new_cell_id();
    // Atoms are text and round-trip through it; structure rides
    // the private format and round-trips through its bytes.
    let atoms = [
        crate::test_values::text("plain"),
        crate::test_values::text("\"tricky\""),
        Value::from(vec![0xde, 0xad]),
    ];
    for value in atoms {
        let (text, structural) = spelling(&value);
        assert!(!structural);
        assert_eq!(resolve_query(&text), value);
    }
    let structures = [
        Value::from(cell),
        Value::list([crate::test_values::text("a"), Value::from(cell)]),
        Value::record([(
            crate::test_values::label("x"),
            crate::test_values::text("1"),
        )]),
        Value::record([(cell, Value::from(vec![0x00_u8]))]),
    ];
    for value in structures {
        let (text, structural) = spelling(&value);
        assert!(structural);
        assert_eq!(from_structure(text.as_bytes()), Some(value));
    }
    // Atoms read in other apps; alien text pastes sensibly — and
    // TEXT IS NEVER STRUCTURE: characters that happen to spell
    // Value JSON read as the string they are.
    assert_eq!(spelling(&crate::test_values::text("hi")).0, "\"hi\"");
    assert_eq!(spelling(&Value::from(vec![0xff_u8])).0, "0xff");
    assert_eq!(
        resolve_query("loose text"),
        crate::test_values::text("loose text")
    );
    let spelled = spelling(&Value::record([])).0;
    assert_eq!(resolve_query(&spelled), crate::test_values::text(&spelled));
}
