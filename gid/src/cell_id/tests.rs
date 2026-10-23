use super::*;

#[test]
fn cell_ids_are_opaque_128_bit_values_with_stable_spellings() {
    let id = CellId::from_u128(0x00112233445566778899aabbccddeeff);
    assert_eq!(
        id.as_bytes(),
        &[
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]
    );
    assert_eq!(id.to_string(), "00112233445566778899aabbccddeeff");
    assert_eq!("00112233445566778899AABBCCDDEEFF".parse(), Ok(id));
    for malformed in [
        "00112233-4455-6677-8899-aabbccddeeff",
        "00112233445566778899aabbccddeefg",
        "00112233445566778899aabbccddeef",
        "00112233445566778899aabbccddeeff00",
        "",
    ] {
        assert_eq!(malformed.parse::<CellId>(), Err(ParseCellIdError));
    }
    assert_eq!(
        serde_json::to_string(&id).unwrap(),
        r#""00112233445566778899aabbccddeeff""#
    );
    assert_eq!(
        serde_json::from_str::<CellId>(r#""00112233445566778899aabbccddeeff""#).unwrap(),
        id
    );
}
