//! Reads and writes of drawn occurrences, by their paths, which say where
//! each one's data is ([`crate::occurrence`]). Mutable access is lent only
//! during dispatch.

use super::*;
use crate::occurrence::Path;

/// Read-only access to the data occurrences show.
#[derive(Clone, Copy)]
pub struct Read<'data> {
    pub(crate) sources: Sources<'data>,
}

impl<'data> Read<'data> {
    pub fn resolve_path(&self, path: &[Step]) -> Option<&'data Value> {
        crate::occurrence::read(&self.sources, path)
    }

    pub(crate) fn writable(&self, path: &[Step]) -> bool {
        crate::occurrence::writable(&self.sources, path)
    }
}

/// Edits of occurrences, acting on the session a dispatch lends.
pub(crate) struct Edit<'a> {
    session: Session<'a>,
}

impl<'a> Edit<'a> {
    pub(crate) fn new(session: Session<'a>) -> Self {
        Self { session }
    }
}

impl Edit<'_> {
    /// The value at `path`.
    pub(crate) fn value(&self, path: &[Step]) -> Option<&Value> {
        crate::occurrence::read(&self.session.sources(), path)
    }

    pub(crate) fn selection(&self) -> Option<&selection::Selection> {
        self.session.model.selection.as_ref()
    }

    /// Whether the selection is at `path` or below it, resting on a value
    /// rather than picking one or typing one.
    pub(crate) fn rests_within(&self, path: &[Step]) -> bool {
        let sources = self.session.sources();
        self.selection().is_some_and(|current| {
            current.path().starts_with(path)
                && current.stage(&sources) == selection::Stage::Edge
                && current.value_edit().is_none()
        })
    }

    /// The state of the occurrence at `path`, which its view keeps.
    pub(crate) fn annotation(&self, path: &[Step]) -> Option<&Value> {
        self.session
            .model
            .workspace
            .view_at(path)?
            .annotations
            .at(path)
    }

    /// Writes `value` at `path` as an undo step of its own; true, writing
    /// nothing, where it's there already.
    pub(crate) fn replace(&mut self, path: &[Step], value: Value) -> bool {
        crate::occurrence::location(path).is_some_and(|location| {
            self.session.sources().read(&location) == Some(&value)
                || write(&mut self.session, &location, value)
        })
    }

    /// Selects the value at `path`. Selecting what is selected already keeps
    /// it and moves its line's caret to the end; anything else, a label being
    /// authored included, starts afresh.
    pub(crate) fn select(&mut self, path: &[Step]) {
        let sources = self.session.sources();
        let reselected = self
            .session
            .model
            .selection
            .as_ref()
            .is_some_and(|current| {
                current.path() == path && current.stage(&sources) != selection::Stage::Label
            });
        let current = self.session.model.selection.take().filter(|_| reselected);
        self.session.model.selection = Some(match current {
            Some(current) => current.reselected(),
            None => selection::Selection::edge(path.to_vec()),
        });
    }

    pub(crate) fn select_payload(&mut self, path: Path, payload: Value) {
        let session = &mut self.session;
        let selection = selection::Selection::from_payload(&session.sources(), path, payload);
        session.model.selection = Some(selection);
    }

    /// Opens or closes the fold at `path`, toggling it without `closed`.
    pub(crate) fn fold(&mut self, path: &[Step], default: bool, closed: Option<bool>) -> bool {
        self.session.set_folded(path, default, closed)
    }

    /// Leaves the value at `path` pending, for the picker to fill.
    pub(crate) fn pend(&mut self, path: Path) {
        self.session.model.selection = Some(selection::pending_value(path));
    }

    /// Replaces the state of the occurrence at `path`; false when it was
    /// already `value`. UI state belongs to the displayed occurrence, not the
    /// shared source.
    pub(crate) fn annotate(&mut self, path: &[Step], value: Value) -> bool {
        match self.session.model.workspace.view_at_mut(path) {
            Some(view) if view.annotations.at(path) != Some(&value) => {
                view.annotations.set(path, Some(value));
                true
            }
            _ => false,
        }
    }

    pub(crate) fn edit_line(
        &mut self,
        path: &[Step],
        line: &crate::display::LineEdit,
        operation: &puri::edit::EditOperation<'_>,
    ) -> bool {
        edit_line(&mut self.session, path, line, operation)
    }

    pub(crate) fn grap_runtime(
        &mut self,
        path: Path,
        function: grap::RuntimeValue,
        event: Value,
    ) -> bool {
        crate::site::apply_scoped_event(&mut self.session, path, function, event)
    }
}

#[cfg(test)]
mod tests;
