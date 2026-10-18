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
