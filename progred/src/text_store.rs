//! Temporary GID-text document storage. Write-through editing keeps the
//! model current; saving prints it with the bridge's external binder table.

use crate::gid_text::{self, Binders};
use gid::Document;
use std::path::Path;

pub fn load(path: &Path) -> Result<(Document, Binders), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    gid_text::parse(&text)
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn save(path: &Path, doc: &Document, binders: &Binders) -> Result<(), String> {
    let text = gid_text::print(doc, binders);
    save_text(path, text)
}

#[cfg(all(any(not(target_arch = "wasm32"), test), target_os = "macos"))]
fn save_text(path: &Path, text: String) -> Result<(), String> {
    use objc2_foundation::{NSData, NSDataWritingOptions, NSString, NSURL};

    let path = path
        .to_str()
        .ok_or_else(|| "document path is not valid Unicode".to_string())?;
    NSData::from_vec(text.into_bytes())
        .writeToURL_options_error(
            &NSURL::fileURLWithPath(&NSString::from_str(path)),
            NSDataWritingOptions::Atomic,
        )
        .map_err(|error| error.to_string())
}

#[cfg(all(any(not(target_arch = "wasm32"), test), not(target_os = "macos")))]
fn save_text(path: &Path, text: String) -> Result<(), String> {
    // Write-then-rename, so a crash mid-write cannot truncate the
    // previous save.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_round_trip_through_disk() {
        let doc = crate::sample::sample_document();
        let path =
            std::env::temp_dir().join(format!("progred-store-test-{}.gid", std::process::id()));
        save(&path, &doc, &Binders::new()).unwrap();
        let (loaded, binders) = load(&path).unwrap();
        assert_eq!(loaded.root, doc.root);
        let again = gid_text::print(&loaded, &binders);
        assert_eq!(again, gid_text::print(&doc, &Binders::new()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn damaged_files_refuse() {
        let path =
            std::env::temp_dir().join(format!("progred-store-bad-{}.gid", std::process::id()));
        std::fs::write(&path, "{\"root\": ").unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}
