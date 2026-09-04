//! Temporary GID-text document storage. Write-through editing keeps the
//! model current; saving prints it with the bridge's external binder table.

use crate::gid_text::{self, Binders};
use gid::Document;
use std::path::Path;

pub fn load(path: &Path) -> Result<(Document, Binders), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    gid_text::parse(&text)
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
pub fn save(path: &Path, doc: &Document, binders: &Binders) -> Result<(), String> {
    let text = gid_text::print(doc, binders);
    save_text(path, text)
}

#[cfg(target_os = "macos")]
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

#[cfg(any(target_os = "linux", all(test, not(target_os = "macos"))))]
fn save_text(path: &Path, text: String) -> Result<(), String> {
    replace_text(path, &text).map_err(|error| error.to_string())
}

#[cfg(any(test, target_os = "linux"))]
fn replace_text(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;

    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".progred-{}.tmp", gid::new_cell_id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        std::fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_preserves_siblings_and_cleans_up_failed_saves() {
        let directory = std::env::temp_dir().join(format!("progred-store-{}", gid::new_cell_id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("document.gid");
        let sibling = directory.join("document.tmp");
        std::fs::write(&path, "old").unwrap();
        std::fs::write(&sibling, "unrelated").unwrap();

        replace_text(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "unrelated");
        let blocked = directory.join("blocked");
        std::fs::create_dir(&blocked).unwrap();
        assert!(replace_text(&blocked, "cannot replace a directory").is_err());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 3);
        std::fs::remove_dir_all(&directory).unwrap();
    }

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
