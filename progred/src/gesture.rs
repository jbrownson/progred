//! Caller-owned continuations for projection gestures. The accepting
//! press installs one; it owns motion until release or cancellation.

use crate::display::widget::gesture::Gesture;
use crate::display::widget::view::Root;
use crate::editing::Session;
use crate::selection::{self, Selection};
use crate::sources::Sources;
use gid::{Path, Value};

// Logical pixels; replace when the input adapter exposes the platform threshold.
pub(crate) const DRAG_THRESHOLD: f64 = 3.0;

pub(crate) type Active = Box<dyn Gesture>;

/// A gesture's writes to the value at one place, recorded as one undo step.
pub(crate) struct EditRun {
    root: Root,
    path: Path,
    scope: crate::editing::Scope,
    recorded: bool,
}

impl EditRun {
    /// The run of writes to the value at `path` in `root`'s view, read
    /// through `scope`.
    pub(crate) fn new(root: Root, path: Path, scope: crate::editing::Scope) -> Self {
        Self {
            root,
            path,
            scope,
            recorded: false,
        }
    }

    pub(crate) fn selected<'a>(&self, session: &'a mut Session<'_>) -> Option<&'a mut Selection> {
        session
            .model
            .selection
            .as_mut()
            .filter(|s| s.root() == &self.root && s.path() == self.path)
    }
    pub(crate) fn select(&self, session: &mut Session<'_>) -> bool {
        if session.model.selection.as_ref().is_some_and(|selection| {
            matches!(
                selection.stage(&session.sources()),
                selection::Stage::Pending | selection::Stage::Label
            )
        }) {
            false
        } else {
            self.scope
                .open(session.reborrow())
                .select(&self.root, &self.path);
            true
        }
    }

    pub(crate) fn write(&mut self, session: &mut Session<'_>, value: Value) -> bool {
        self.scope.location(&self.path).is_some_and(|location| {
            crate::editing::write(session, &location, value, &mut self.recorded)
        })
    }

    pub(crate) fn selection(&self, session: &mut Session<'_>, payload: Value) {
        let model = &mut *session.model;
        let libraries = session.libraries;
        if let Some(recorded) = model
            .selection
            .as_ref()
            .filter(|selection| selection.root() == &self.root && selection.path() == self.path)
            .map(Selection::recorded)
        {
            let mut next = Selection::from_scoped_payload(
                &self.root,
                &Sources {
                    doc: &model.doc,
                    libraries,
                },
                self.scope.clone(),
                self.path.clone(),
                payload,
            );
            next.set_recorded(recorded);
            model.selection = Some(next);
        }
    }
}

#[cfg(test)]
mod tests;
