use super::*;
use crate::selection::resolve_query;
use gid::new_cell_id;

#[test]
fn clipboard_spellings_round_trip() {
    let cell = new_cell_id();
    // Atoms are text and round-trip through it; structure rides
    // the private format and round-trips through its bytes.
    let atoms = [
        crate::libraries::text::value("plain"),
        crate::libraries::text::value("\"tricky\""),
        Value::from(vec![0xde, 0xad]),
    ];
    for value in atoms {
        let (text, structural) = spelling(&value);
        assert!(!structural);
        assert_eq!(resolve_query(&text), value);
    }
    let structures = [
        Value::from(cell),
        Value::list([crate::libraries::text::value("a"), Value::from(cell)]),
        Value::record([(
            crate::test_values::label("x"),
            crate::libraries::text::value("1"),
        )]),
        Value::record([(cell, Value::from(vec![0x00_u8]))]),
    ];
    for value in structures {
        let (_, structural) = spelling(&value);
        assert!(structural);
        assert_eq!(from_structure(&structure_bytes(&value)), Some(value));
    }
    // Atoms read in other apps; alien text pastes sensibly — and
    // TEXT IS NEVER STRUCTURE: characters that happen to spell
    // Value JSON read as the string they are.
    assert_eq!(spelling(&crate::libraries::text::value("hi")).0, "\"hi\"");
    assert_eq!(spelling(&Value::from(vec![0xff_u8])).0, "0xff");
    assert_eq!(
        resolve_query("loose text"),
        crate::libraries::text::value("loose text")
    );
    let spelled = spelling(&Value::record([])).0;
    assert_eq!(
        resolve_query(&spelled),
        crate::libraries::text::value(&spelled)
    );
}

#[test]
fn structure_keeps_list_positions() {
    let (document, _) =
        crate::gid_text::parse(r#"{"root": [0x40: 0x01, 0x40ff: [0x80: 0x02]]}"#).unwrap();
    let list = document.root.unwrap();
    let pasted = from_structure(&structure_bytes(&list)).unwrap();
    let positions = |value: &Value| -> Vec<Vec<u8>> {
        value
            .as_list()
            .unwrap()
            .keys()
            .map(|position| position.as_bytes().to_vec())
            .collect()
    };
    assert_eq!(positions(&pasted), [vec![0x40], vec![0x40, 0xff]]);
    let inner = |value: &Value| value.as_list().unwrap().values().nth(1).unwrap().clone();
    assert_eq!(positions(&inner(&pasted)), [vec![0x80]]);
}
