//! Where copies go: the system clipboard, or one the editor keeps to itself.

use gid::Value;
use puri::edit::TextClipboard;

pub(crate) enum Clipboard {
    /// The platform's, shared with other apps.
    #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
    System,
    /// The editor's own, for tests, browsers, and sessions with no display.
    Own {
        text: Option<String>,
        /// The value a structural copy carries beside its text.
        structure: Option<Value>,
    },
}

/// An empty clipboard of the editor's own.
impl Default for Clipboard {
    fn default() -> Self {
        Self::Own {
            text: None,
            structure: None,
        }
    }
}

impl Clipboard {
    /// Holds `text`, and `structure` beside it when the copy is structural;
    /// whether the copy took.
    pub(crate) fn copy(&mut self, text: String, structure: Option<&Value>) -> bool {
        match self {
            #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
            Self::System => system::copy(text, structure.is_some()),
            Self::Own {
                text: held,
                structure: carried,
            } => {
                *held = Some(text);
                *carried = structure.cloned();
                true
            }
        }
    }

    /// The structure a structural copy left, when the clipboard carries one.
    pub(crate) fn structure(&mut self) -> Option<Value> {
        match self {
            #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
            Self::System => system::structure(),
            Self::Own { structure, .. } => structure.clone(),
        }
    }
}

impl TextClipboard for Clipboard {
    fn get_text(&mut self) -> Option<String> {
        match self {
            #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
            Self::System => system::text(),
            Self::Own { text, .. } => text.clone(),
        }
    }

    fn set_text(&mut self, text: &str) {
        self.copy(text.to_string(), None);
    }

    fn holds_structure(&mut self) -> bool {
        match self {
            #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
            Self::System => system::holds_structure(),
            Self::Own { structure, .. } => structure.is_some(),
        }
    }
}

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
mod system {
    use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext, ContentFormat};
    use gid::Value;

    /// The pasteboard type structural copies ride under, beside their
    /// plain text; its PRESENCE is the structure/text distinction, so
    /// text that merely spells Value JSON is never mistaken for a copy.
    const FORMAT: &str = "com.progred.value";

    pub(super) fn text() -> Option<String> {
        ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_text().ok())
    }

    pub(super) fn copy(text: String, structural: bool) -> bool {
        ClipboardContext::new()
            .and_then(|cb| {
                if structural {
                    // Both representations: the private format says
                    // "structure", the text reads anywhere.
                    cb.set(vec![
                        ClipboardContent::Other(FORMAT.to_string(), text.clone().into_bytes()),
                        ClipboardContent::Text(text),
                    ])
                } else {
                    cb.set_text(text)
                }
            })
            .is_ok()
    }

    pub(super) fn holds_structure() -> bool {
        ClipboardContext::new().is_ok_and(|cb| cb.has(ContentFormat::Other(FORMAT.to_string())))
    }

    pub(super) fn structure() -> Option<Value> {
        ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_buffer(FORMAT).ok())
            .and_then(|bytes| crate::selection::from_structure(&bytes))
    }
}
