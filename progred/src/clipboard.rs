//! Where copies go: the system clipboard, or one the editor keeps to itself,
//! and how a value spells there.

use ::grap::text;
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
        /// The picture of it a structural copy carries, as a PNG.
        picture: Option<Vec<u8>>,
    },
}

/// An empty clipboard of the editor's own.
impl Default for Clipboard {
    fn default() -> Self {
        Self::Own {
            text: None,
            structure: None,
            picture: None,
        }
    }
}

impl Clipboard {
    /// Holds `text`, and `structure` beside it when the copy is structural,
    /// with a `picture` of it as a PNG when there is one; whether the copy
    /// took.
    pub(crate) fn copy(
        &mut self,
        text: String,
        structure: Option<&Value>,
        picture: Option<Vec<u8>>,
    ) -> bool {
        match self {
            #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
            Self::System => system::copy(text, structure.is_some(), picture),
            Self::Own {
                text: held,
                structure: carried,
                picture: shown,
            } => {
                *held = Some(text);
                *carried = structure.cloned();
                *shown = picture;
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
        self.copy(text.to_string(), None, None);
    }

    fn holds_structure(&mut self) -> bool {
        match self {
            #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
            Self::System => system::holds_structure(),
            Self::Own { structure, .. } => structure.is_some(),
        }
    }
}

/// The clipboard spelling of a value, and whether it is STRUCTURE.
/// Values carry their own inline structure, and cell copies are
/// ALWAYS SHALLOW — a link is its identity alone, no cell values
/// travel: the value/cell boundary IS the copy boundary. Compact text
/// and blobs spell as the query language — quoted text and `0x` hex —
/// and are not structure: their text is their faithful form. Links,
/// lists, and records spell as their saved text form, a document holding
/// only them, so a paste keeps their list positions, and the paths into
/// them, and they ARE: the shell writes that spelling under the private
/// clipboard format too, whose presence is what says "structure" — never
/// the text's shape, so text that happens to spell a document stays text.
/// On the desktop a picture of what was copied joins them, as
/// [`crate::picture`] says.
pub(crate) fn spelling(value: &Value) -> (String, bool) {
    match (text::read(value), value.as_blob()) {
        (Some(text), _) => (format!("\"{text}\""), false),
        (_, Some(_)) => (value.to_string(), false),
        _ => {
            let document = gid::Document {
                root: Some(value.clone()),
                cells: gid::Cells::new(),
            };
            (crate::gid_text::print(&document, &Default::default()), true)
        }
    }
}

/// The value the private clipboard format's bytes denote.
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn from_structure(bytes: &[u8]) -> Option<Value> {
    crate::gid_text::parse(std::str::from_utf8(bytes).ok()?)
        .ok()?
        .0
        .root
}

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
mod system {
    use clipboard_rs::common::RustImage;
    use clipboard_rs::{
        Clipboard, ClipboardContent, ClipboardContext, ContentFormat, RustImageData,
    };
    use gid::Value;

    /// The pasteboard type structural copies ride under, beside their
    /// plain text; its PRESENCE is the structure/text distinction, so
    /// text that merely spells a document is never mistaken for a copy.
    const FORMAT: &str = "com.progred.value";

    pub(super) fn text() -> Option<String> {
        ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_text().ok())
    }

    pub(super) fn copy(text: String, structural: bool, picture: Option<Vec<u8>>) -> bool {
        ClipboardContext::new()
            .and_then(|cb| {
                if structural {
                    // Every representation, richest first, as apps pick the
                    // first they can read: the private format says
                    // "structure", the picture shows what was copied to an
                    // app that takes one, and the text reads anywhere.
                    cb.set(
                        [
                            Some(ClipboardContent::Other(
                                FORMAT.to_string(),
                                text.clone().into_bytes(),
                            )),
                            picture
                                .and_then(|png| RustImageData::from_bytes(&png).ok())
                                .map(ClipboardContent::Image),
                            Some(ClipboardContent::Text(text)),
                        ]
                        .into_iter()
                        .flatten()
                        .collect(),
                    )
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
            .and_then(|bytes| super::from_structure(&bytes))
    }
}

#[cfg(test)]
mod tests;
