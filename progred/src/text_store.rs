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
#[path = "tests/text_store.rs"]
mod tests;
