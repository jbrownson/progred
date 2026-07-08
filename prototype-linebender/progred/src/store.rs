//! Document files: the graph as pretty JSON on disk. Write-through
//! editing means the graph is always current, so saving is a plain
//! serialization of the model.

use crate::raw::Document;
use std::path::Path;

pub fn load(path: &Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub fn save(path: &Path, doc: &Document) -> Result<(), String> {
    let json = serde_json::to_string_pretty(doc).map_err(|error| error.to_string())?;
    // Write-then-rename, so a crash mid-write cannot truncate the
    // previous save.
    let tmp = path.with_extension("progred.tmp");
    std::fs::write(&tmp, json).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_round_trip_through_disk_json() {
        let doc = crate::raw::sample_document();
        let path = std::env::temp_dir().join(format!(
            "progred-store-test-{}.progred",
            std::process::id()
        ));
        save(&path, &doc).unwrap();
        let loaded = load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(loaded.root, doc.root);
        assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            serde_json::to_string(&doc).unwrap()
        );
    }
}
