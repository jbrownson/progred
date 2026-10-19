use super::*;

#[test]
fn line_updates_require_complete_hex() {
    for (input, expected) in [
        ("DEad", Some(vec![0xde, 0xad])),
        ("", Some(vec![])),
        (" 00ff ", Some(vec![0x00, 0xff])),
        ("f", None),
        ("xyz", None),
    ] {
        assert_eq!(edit(input, None), expected.map(Value::from));
    }
}
