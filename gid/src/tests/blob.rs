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
fn blobs_read_back_the_spelling_they_display() {
    let blob = Blob::from(vec![0xde, 0xad]);
    assert_eq!(blob.to_string(), "0xdead");
    assert_eq!(Blob::read("0xdead"), Some(blob.clone()));
    assert_eq!(Blob::read("0xDEAD"), Some(blob));
    assert_eq!(Blob::read("0x"), Some(Blob::from(Vec::new())));
    for malformed in ["dead", "0xdea", "0xdeag", "0Xdead", ""] {
        assert_eq!(Blob::read(malformed), None);
    }
}
