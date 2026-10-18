use super::*;
use crate::libraries::absent;

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
