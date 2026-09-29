//! The system clipboard, and an in-memory stand-in for tests and browsers.

use puri::edit::TextClipboard;

/// The pasteboard type structural copies ride under, beside their
/// plain text; its PRESENCE is the structure/text distinction, so
/// text that merely spells Value JSON is never mistaken for a copy.
#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
pub(crate) const CLIPBOARD_FORMAT: &str = "com.progred.value";

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
pub(crate) struct SystemTextClipboard;

#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Default)]
pub(crate) struct SystemTextClipboard {
    pub(crate) text: Option<String>,
    pub(crate) structure: Option<gid::Value>,
}

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
impl TextClipboard for SystemTextClipboard {
    fn get_text(&mut self) -> Option<String> {
        use clipboard_rs::{Clipboard, ClipboardContext};
        ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_text().ok())
    }

    fn set_text(&mut self, text: &str) {
        use clipboard_rs::{Clipboard, ClipboardContext};
        if let Ok(cb) = ClipboardContext::new() {
            cb.set_text(text.to_string()).ok();
        }
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
impl TextClipboard for SystemTextClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.text = Some(text.to_string());
        self.structure = None;
    }
}
