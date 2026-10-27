use super::*;

#[test]
fn adopts_bytes_and_clones_share_them() {
    let bytes = vec![7; 1024 * 1024];
    let address = bytes.as_ptr();
    let blob = Blob::from(bytes);
    let copy = blob.clone();
    assert_eq!(blob.as_ptr(), address);
    assert_eq!(copy.as_ptr(), address);
    assert_eq!(copy, blob);
}

#[test]
fn equality_and_hashing_compare_bytes_not_storage() {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let hash = |blob: &Blob| {
        let mut hasher = DefaultHasher::new();
        blob.hash(&mut hasher);
        hasher.finish()
    };
    for bytes in [vec![], vec![1], vec![0; 4096]] {
        let a = Blob::from(bytes.clone());
        let b = Blob::from(bytes);
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
    }
}

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
