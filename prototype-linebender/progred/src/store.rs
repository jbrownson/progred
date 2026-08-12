//! Document files in the gid notation (docs/gid.md).
//! Write-through editing means the graph is always current, so
//! saving is a plain print of the model — plus the binder table the
//! notation keeps between load and save, which never enters the
//! model.

use crate::gid::{self, Binders};
use crate::document::Document;
use std::path::Path;

pub fn load(path: &Path) -> Result<(Document, Binders), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    gid::parse(&text)
}

pub fn save(path: &Path, doc: &Document, binders: &Binders) -> Result<(), String> {
    let text = gid::print(doc, binders);
    // Write-then-rename, so a crash mid-write cannot truncate the
    // previous save.
    let tmp = path.with_extension("gid.tmp");
    std::fs::write(&tmp, text).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_round_trip_through_disk() {
        let doc = crate::document::sample_document();
        let path = std::env::temp_dir().join(format!(
            "progred-store-test-{}.gid",
            std::process::id()
        ));
        save(&path, &doc, &Binders::new()).unwrap();
        let (loaded, binders) = load(&path).unwrap();
        assert_eq!(loaded.root, doc.root);
        let again = gid::print(&loaded, &binders);
        assert_eq!(again, gid::print(&doc, &Binders::new()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn damaged_files_refuse() {
        let path = std::env::temp_dir().join(format!(
            "progred-store-bad-{}.gid",
            std::process::id()
        ));
        std::fs::write(&path, "{\"root\": ").unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}
