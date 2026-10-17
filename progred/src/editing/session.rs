//! What a handler acts on while an event is dispatched: the model, read over
//! the loaded libraries, the drag a press starts, and the text tools a line
//! editor needs. Nothing reachable from it leads to the window or the menus;
//! the shell shows what changed once dispatch returns.

use crate::gesture::Active;
use crate::library::Libraries;
use crate::model::Model;
use crate::sources::Sources;
use crate::workspace::Root;
use gid::Step;
use kurbo::Point;
use parley::{FontContext, LayoutContext};
use peniko::Brush;
use puri::edit::TextClipboard;

/// Its fields are the core's, which acts on the model through them.
pub(crate) struct Session<'a> {
    pub(crate) model: &'a mut Model,
    pub(crate) libraries: &'a Libraries,
    /// The drag a press started, which owns motion until release.
    pub(crate) gesture: &'a mut Option<Active>,
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

    /// The same capabilities, lent on for a shorter while.
    pub(crate) fn reborrow(&mut self) -> Session<'_> {
        Session {
            model: &mut *self.model,
            libraries: self.libraries,
            gesture: &mut *self.gesture,
            text: TextTools {
                fonts: &mut *self.text.fonts,
                layouts: &mut *self.text.layouts,
                clipboard: &mut *self.text.clipboard,
            },
        }
    }

    /// Hands motion to `gesture` until release, starting with `samples`.
    /// A gesture already running finishes first.
    pub(crate) fn start_gesture(&mut self, gesture: Active, samples: &[Point]) {
        self.finish_gesture();
        *self.gesture = Some(gesture);
        self.advance_gesture(samples);
    }

    /// Feeds the running gesture `samples`, answering whether it changed
    /// anything; `None` when no gesture runs.
    pub(crate) fn advance_gesture(&mut self, samples: &[Point]) -> Option<bool> {
        let mut gesture = self.gesture.take()?;
        let changed = gesture.advance(&mut self.reborrow(), samples);
        *self.gesture = Some(gesture);
        Some(changed)
    }

    /// Ends the running gesture; false when none runs.
    pub(crate) fn finish_gesture(&mut self) -> bool {
        let Some(mut gesture) = self.gesture.take() else {
            return false;
        };
        gesture.finish(&mut self.reborrow());
        true
    }

    /// Opens or closes the fold at `path` in `root`'s view, toggling it
    /// without `closed`. Fold state belongs to an occurrence: projection
    /// supplies its default, and no document lookup or writable source is
    /// needed to change it. A change ends any drag.
    pub(crate) fn set_collapsed(
        &mut self,
        root: &Root,
        path: &[Step],
        default: bool,
        closed: Option<bool>,
    ) -> bool {
        let changed = self.model.set_collapsed(root, path, default, closed);
        if changed {
            self.finish_gesture();
        }
        changed
    }
}

impl crate::display::Host for crate::Editor {
    fn session(&mut self) -> Session<'_> {
        crate::Editor::session(self)
    }
}

impl crate::Editor {
    /// The editor's capabilities for one dispatch.
    pub(crate) fn session(&mut self) -> Session<'_> {
        Session {
            model: &mut self.model,
            libraries: &self.stack.libraries,
            gesture: &mut self.gesture,
            text: TextTools {
                fonts: &mut self.font_cx,
                layouts: &mut self.layout_cx,
                clipboard: &mut self.text_clipboard,
            },
        }
    }
}
