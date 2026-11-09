use super::*;

#[test]
fn typed_hex_reads_every_byte_in_either_case_and_only_whole_bytes() {
    let bytes: Vec<u8> = (0..=255).collect();
    for hex in [hex_string(&bytes), hex_string(&bytes).to_uppercase()] {
        assert_eq!(read_hex(&hex), Some(bytes.clone()));
    }
    assert_eq!(read_hex(""), Some(vec![]));
    for malformed in ["f", "fg", "é", "ff 00", "🦀"] {
        assert_eq!(read_hex(malformed), None);
    }
}
