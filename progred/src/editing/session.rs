//! What a handler acts on while an event is dispatched: the model, read over
//! the loaded libraries, and the text tools a line editor needs. Nothing
//! reachable from it leads to the window or the menus; the shell shows what
//! changed once dispatch returns.

use crate::library::Libraries;
use crate::model::Model;
use crate::sources::Sources;
use parley::{FontContext, LayoutContext};
use peniko::Brush;
use puri::edit::TextClipboard;

/// Its fields are the core's, which acts on the model through them.
pub(crate) struct Session<'a> {
    pub(crate) model: &'a mut Model,
    pub(crate) libraries: &'a Libraries,
    pub(crate) text: TextTools<'a>,
}

/// What editing a line of text needs: fonts to shape it, and the clipboard.
pub(crate) struct TextTools<'a> {
    pub(crate) fonts: &'a mut FontContext,
    pub(crate) layouts: &'a mut LayoutContext<Brush>,
    pub(crate) clipboard: &'a mut dyn TextClipboard,
}

impl Session<'_> {
    pub(crate) fn sources(&self) -> Sources<'_> {
        Sources {
            doc: &self.model.doc,
            libraries: self.libraries,
        }
    }
}

impl crate::Editor {
    /// The editor's capabilities for one dispatch.
    pub(crate) fn session(&mut self) -> Session<'_> {
        Session {
            model: &mut self.model,
            libraries: &self.stack.libraries,
            text: TextTools {
                fonts: &mut self.font_cx,
                layouts: &mut self.layout_cx,
                clipboard: &mut self.text_clipboard,
            },
        }
    }
}
