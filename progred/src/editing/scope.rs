//! A composable interpretation of locations, shared by reads and writes.
//! Scopes own no editor state. Mutable access is lent only during dispatch.

use super::*;
use crate::location::Location;
use gid::{CellId, Path, Resolution};
use std::rc::Rc;

/// How a projection's occurrences name stored data. Each layer reads the
/// occurrences at and beneath one occurrence; any other falls through to the
/// scope the layer was added to. Past every layer, an occurrence's path is
/// the document's own, from its root.
#[derive(Clone, Default)]
pub(crate) struct Scope(Option<Rc<Layer>>);

struct Layer {
    occurrence: Path,
    reading: Reading,
    within: Scope,
}

enum Reading {
    /// A projection's conject to a location, as a jump reads it: the steps
    /// beneath the occurrence continue that location's.
    Located(Location<'static>),
    /// A computed value's conject: nothing at or beneath the occurrence is
    /// stored, until a cell inside it starts a location again.
    Detached,
    /// A cell's definition, followed at the occurrence: a location starts
    /// there, however the cell was reached.
    Cell {
        cell: CellId,
        resolution: Resolution,
    },
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_none() {
            "Scope(identity)"
        } else {
            "Scope(..)"
        })
    }
}

impl Scope {
    pub(crate) fn view<'scope, 'data>(
        &'scope self,
        sources: Sources<'data>,
    ) -> Read<'scope, 'data> {
        Read {
            sources,
            scope: self,
        }
    }

    fn layer(&self, occurrence: Path, reading: Reading) -> Self {
        Self(Some(Rc::new(Layer {
            occurrence,
            reading,
            within: self.clone(),
        })))
    }

    /// The occurrence reads `location`, and what's beneath it what's beneath
    /// that. Other occurrences keep their enclosing interpretation.
    pub(crate) fn located(&self, occurrence: Path, location: Location<'static>) -> Self {
        self.layer(occurrence, Reading::Located(location))
    }

    /// A computed value at this occurrence: nothing at or beneath it is
    /// stored, until a cell inside it starts a location again.
    pub(crate) fn detached(&self, occurrence: Path) -> Self {
        self.layer(occurrence, Reading::Detached)
    }

    /// The occurrence follows `cell` to its definition from `source`, where
    /// it and everything beneath it start their locations.
    pub(crate) fn in_cell(&self, occurrence: Path, cell: CellId, resolution: Resolution) -> Self {
        self.layer(occurrence, Reading::Cell { cell, resolution })
    }

    /// Where the data at `path` is stored, as this scope reads it.
    pub(crate) fn location<'p>(&self, path: &'p [Step]) -> Option<Location<'p>> {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Conject);
        match &self.0 {
            None => Some(Location::root(path)),
            Some(layer) => match path.strip_prefix(layer.occurrence.as_slice()) {
                None => layer.within.location(path),
                Some(rest) => match &layer.reading {
                    Reading::Located(location) => Some(Location {
                        start: location.start,
                        path: location.path.iter().chain(rest).cloned().collect(),
                    }),
                    Reading::Detached => None,
                    Reading::Cell { cell, resolution } => {
                        Some(Location::in_cell(*cell, *resolution, rest))
                    }
                },
            },
        }
    }

    /// Whether, beneath the occurrence the newest layer reads, an
    /// occurrence's children are its location's children, so a projection
    /// can descend into values rather than read each one again.
    pub(crate) fn descends(&self) -> bool {
        self.0
            .as_ref()
            .is_none_or(|layer| matches!(layer.reading, Reading::Cell { .. }))
    }

    /// Compare what this occurrence names, not how each scope spells it: a
    /// route through a cell and a location starting at that cell are the
    /// same data.
    pub(crate) fn same_location(&self, other: &Self, sources: &Sources, path: &[Step]) -> bool {
        match (self.location(path), other.location(path)) {
            (Some(left), Some(right)) => left
                .stored(sources)
                .zip(right.stored(sources))
                .map_or_else(|| left == right, |(left, right)| left == right),
            (left, right) => left.is_none() && right.is_none(),
        }
    }

    pub(crate) fn open<'a>(&'a self, session: Session<'a>) -> Edit<'a> {
        Edit {
            scope: self,
            session,
        }
    }
}

/// Read-only access in the same location interpretation as the writable view.
pub struct Read<'scope, 'data> {
    pub(crate) sources: Sources<'data>,
    pub(crate) scope: &'scope Scope,
}

impl<'data> Read<'_, 'data> {
    pub fn resolve_path(&self, path: &[Step]) -> Option<&'data Value> {
        self.sources.read(&self.scope.location(path)?)
    }

    pub(crate) fn writable(&self, path: &[Step]) -> bool {
        self.scope
            .location(path)
            .is_some_and(|location| crate::location::writable(&self.sources, &location))
    }
}

/// Edits read and written through a scope, acting on the session a dispatch
/// lends.
pub(crate) struct Edit<'a> {
    scope: &'a Scope,
    session: Session<'a>,
}

impl Edit<'_> {
    /// The value at `path`, read through this scope.
    pub(crate) fn value(&self, path: &[Step]) -> Option<&Value> {
        self.scope.view(self.session.sources()).resolve_path(path)
    }

    pub(crate) fn selection(&self) -> Option<&selection::Selection> {
        self.session.model.selection.as_ref()
    }

    pub(crate) fn annotation(&self, root: &Root, path: &[Step]) -> Option<&Value> {
        self.session
            .model
            .workspace
            .view(root)?
            .annotations
            .at(path)
    }

    /// Writes `value` at `path` as an undo step of its own; true, writing
    /// nothing, where it's there already.
    pub(crate) fn replace(&mut self, path: &[Step], value: Value) -> bool {
        self.scope.location(path).is_some_and(|location| {
            self.session.sources().read(&location) == Some(&value)
                || write(&mut self.session, &location, value, &mut false)
        })
    }

    /// Selects the value at `path` in `root`'s view. Selecting what is
    /// selected already, at the same location, keeps it and moves its line's
    /// caret to the end; anything else, a label being authored included,
    /// starts afresh.
    pub(crate) fn select(&mut self, root: &Root, path: &[Step]) {
        let sources = self.session.sources();
        let reselected = self
            .session
            .model
            .selection
            .as_ref()
            .is_some_and(|current| {
                current.root() == root
                    && current.path() == path
                    && current.stage(&sources) != selection::Stage::Label
                    && current.scope().same_location(self.scope, &sources, path)
            });
        let current = self.session.model.selection.take().filter(|_| reselected);
        self.session.model.selection = Some(
            match current {
                Some(mut current) => {
                    if let Some(line) = current.edit_mut() {
                        line.cursor_to_end();
                    }
                    current
                }
                None => selection::Selection::edge(root, path.to_vec()),
            }
            .with_scope(self.scope.clone()),
        );
    }

    pub(crate) fn select_payload(&mut self, root: &Root, path: Path, payload: Value) {
        let session = &mut self.session;
        let selection = selection::Selection::from_scoped_payload(
            root,
            &session.sources(),
            self.scope.clone(),
            path,
            payload,
        );
        session.model.selection = Some(selection);
    }

    /// Opens or closes the fold at `path` in `root`'s view, toggling it
    /// without `closed`.
    pub(crate) fn fold(
        &mut self,
        root: &Root,
        path: &[Step],
        default: bool,
        closed: Option<bool>,
    ) -> bool {
        self.session.set_collapsed(root, path, default, closed)
    }

    /// Leaves the value at `path` pending, for the picker to fill.
    pub(crate) fn pend(&mut self, root: &Root, path: Path) {
        self.session.model.selection =
            Some(selection::pending_value(root, path).with_scope(self.scope.clone()));
    }

    /// Replaces the state of the occurrence at `path` in `root`'s view; false
    /// when it was already `value`. UI state belongs to the displayed
    /// occurrence, not the shared source.
    pub(crate) fn annotate(&mut self, root: &Root, path: &[Step], value: Value) -> bool {
        match self.session.model.workspace.view_mut(root) {
            Some(view) if view.annotations.at(path) != Some(&value) => {
                view.annotations.set(path, Some(value));
                true
            }
            _ => false,
        }
    }

    pub(crate) fn edit_line(
        &mut self,
        root: &Root,
        path: &[Step],
        line: &crate::display::LineEdit,
        operation: &puri::edit::EditOperation<'_>,
    ) -> bool {
        let session = &mut self.session;
        session.model.selection.as_ref().is_some_and(|selection| {
            selection
                .scope()
                .same_location(self.scope, &session.sources(), path)
        }) && edit_line(session, root, path, line, operation)
    }

    pub(crate) fn grap_runtime(
        &mut self,
        root: Root,
        path: Path,
        function: grap::RuntimeValue,
        event: Value,
    ) -> bool {
        crate::site::apply_scoped_event(
            &mut self.session,
            self.scope.clone(),
            root,
            path,
            function,
            event,
        )
    }
}

#[cfg(test)]
mod tests;
