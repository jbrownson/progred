use std::{ops::Deref, sync::Arc};

/// Bytes with value semantics: clones share storage, writes detach on demand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Blob(Arc<Vec<u8>>);

impl Blob {
    pub fn make_mut(&mut self) -> &mut Vec<u8> {
        Arc::make_mut(&mut self.0)
    }
}

impl From<Vec<u8>> for Blob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(Arc::new(bytes))
    }
}

impl Deref for Blob {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
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
    fn edits_detach_shared_storage_but_not_unique_storage() {
        let mut blob = Blob::from(vec![1, 2, 3]);
        let address = blob.as_ptr();
        blob.make_mut()[0] = 4;
        assert_eq!(blob.as_ptr(), address);
        let snapshot = blob.clone();
        blob.make_mut().push(5);
        assert_eq!(&*blob, &[4, 2, 3, 5]);
        assert_eq!(&*snapshot, &[4, 2, 3]);
        assert_ne!(blob.as_ptr(), snapshot.as_ptr());
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
}
