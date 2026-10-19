//! Caller-owned continuations for projection gestures. The accepting
//! press installs one; it owns motion until release or cancellation. A
//! gesture writes through an edit run, which records its writes as one undo
//! step.

use crate::display::widget::gesture::Gesture;
use crate::editing::{Session, Site};
use crate::selection::{self, Selection};
use gid::Value;

/// How far a press moves, in logical pixels, before it drags.
pub(crate) const DRAG_THRESHOLD: f64 = 3.0;

pub(crate) type Active = Box<dyn Gesture>;

/// A gesture's writes to the value at one place, recorded as one undo step.
pub(crate) struct EditRun {
    site: Site,
    recorded: bool,
}

impl EditRun {
    /// The run of writes to the value `site` names.
    pub(crate) fn new(site: Site) -> Self {
        Self {
            site,
            recorded: false,
        }
    }

    pub(crate) fn selected<'a>(&self, session: &'a mut Session<'_>) -> Option<&'a mut Selection> {
        session
            .model
            .selection
            .as_mut()
            .filter(|selection| self.site.holds(selection))
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
            self.site.open(session).select();
            true
        }
    }

    /// Writes `value` here, unless it's there already: the run's first
    /// write records its undo step, and the rest join it. Whether the
    /// document changed.
    pub(crate) fn write(&mut self, session: &mut Session<'_>, value: Value) -> bool {
        self.site.location().is_some_and(|location| {
            if session.sources().read(&location) == Some(&value) {
                false
            } else if self.recorded {
                crate::location::set_value(
                    &mut session.model.doc,
                    session.libraries,
                    &location,
                    value,
                )
            } else {
                self.recorded = crate::editing::write(session, &location, value);
                self.recorded
            }
        })
    }

    pub(crate) fn selection(&self, session: &mut Session<'_>, payload: Value) {
        if let Some(recorded) = self.selected(session).map(|selection| selection.recorded()) {
            self.site.open(session).select_with(payload);
            if let Some(selection) = &mut session.model.selection {
                selection.set_recorded(recorded);
            }
        }
    }
}

#[cfg(test)]
mod tests;
