use super::*;
use crate::libraries::absent;

#[test]
fn hex_spelling_round_trips_every_byte_and_requires_complete_bytes() {
    let bytes: Vec<u8> = (0..=255).collect();
    for hex in [
        gid::hex_string(&bytes),
        gid::hex_string(&bytes).to_uppercase(),
    ] {
        assert_eq!(parse(&format!("0x{hex}")), Some(bytes.clone()));
    }
    assert_eq!(parse("0x"), Some(vec![]));
    for malformed in ["ff", "0xf", "0xfg", "0xé", "0xff 00", "0x🦀"] {
        assert_eq!(parse(malformed), None);
    }
}

#[test]
fn line_updates_require_complete_hex() {
    for (input, expected) in [
        ("DEad", Some(vec![0xde, 0xad])),
        ("", Some(vec![])),
        (" 00ff ", Some(vec![0x00, 0xff])),
        ("f", None),
        ("xyz", None),
    ] {
        let evaluation = crate::libraries::test_evaluate(
            &::grap::call(
                ::grap::ffi(vocabulary::UPDATE),
                [(line_edit::vocabulary::INPUT, text::value(input))],
            ),
            |_| None,
            &functions(),
            100,
        );
        assert!(evaluation.completed);
        match expected {
            Some(bytes) => assert_eq!(evaluation.result, Value::from(bytes)),
            None => assert!(absent::is_absent(&evaluation.result)),
        }
    }
}
