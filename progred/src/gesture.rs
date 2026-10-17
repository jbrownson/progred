//! Caller-owned continuations for projection gestures. The accepting
//! press installs one; it owns motion until release or cancellation.

use crate::display::widget::gesture::Gesture;
use crate::editing::Session;
use crate::library::Libraries;
use crate::location::Location;
use crate::model::Model;
use crate::selection::{self, Selection};
use crate::sources::Sources;
use crate::workspace::Root;
use gid::{Path, Value};
#[cfg(test)]
use kurbo::Point;

// Logical pixels; replace when the input adapter exposes the platform threshold.
pub(crate) const DRAG_THRESHOLD: f64 = 3.0;

pub(crate) type Active = Box<dyn Gesture>;

pub(crate) struct EditRun {
    root: Root,
    path: Path,
    scope: crate::editing::Scope,
    recorded: bool,
}

#[cfg(test)]
pub(crate) fn value_edit(root: Root, path: Path) -> EditRun {
    scoped_value_edit(root, path, Default::default())
}

pub(crate) fn scoped_value_edit(root: Root, path: Path, scope: crate::editing::Scope) -> EditRun {
    EditRun {
        root,
        path,
        scope,
        recorded: false,
    }
}

impl EditRun {
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
                .open(crate::editing::Access::of(session))
                .select(&self.root, &self.path);
            true
        }
    }

    pub(crate) fn write(&mut self, session: &mut Session<'_>, value: Value) -> bool {
        let Some(source) = self.scope.source(&self.path) else {
            return false;
        };
        write_value(
            session.model,
            session.libraries,
            &source,
            value,
            &mut self.recorded,
        )
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
            next.preserve_recorded(recorded);
            model.selection = Some(next);
        }
    }
}

fn write_value(
    model: &mut Model,
    libraries: &Libraries,
    location: &Location,
    replacement: Value,
    recorded: &mut bool,
) -> bool {
    if (Sources {
        doc: &model.doc,
        libraries,
    })
    .read(location)
        == Some(&replacement)
    {
        false
    } else {
        let before = (!*recorded).then(|| model.snapshot());
        let wrote = selection::set_value(&mut model.doc, libraries, location, replacement);
        if wrote && let Some(before) = before {
            model.history.record(before);
            *recorded = true;
        }
        wrote
    }
}

#[cfg(test)]
mod tests;
