//! A composable interpretation of locations, shared by reads and writes.
//! Scopes own no editor state. Mutable access is lent only during dispatch.

use super::*;
use crate::location::Location;
use gid::{CellId, Resolution};
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
    /// A projection's own reading of a location through its conject, which
    /// maps the steps beneath the occurrence onto that location's.
    Conject {
        location: Location<'static>,
        conject: crate::display::Conject,
    },
    /// A cell's definition, followed at the occurrence: a location starts
    /// there, however the cell was reached.
    Cell { cell: CellId, source: Resolution },
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

    /// Install a conject for this occurrence. Other occurrences keep their
    /// enclosing interpretation; the function sees the suffix below this one.
    pub(crate) fn with_conject(
        &self,
        occurrence: Path,
        location: Location<'static>,
        conject: crate::display::Conject,
    ) -> Self {
        self.layer(occurrence, Reading::Conject { location, conject })
    }

    /// A computed value at this occurrence: nothing at or beneath it is
    /// stored, until a cell inside it starts a location again.
    pub(crate) fn detached(&self, occurrence: Path) -> Self {
        self.with_conject(
            occurrence,
            Location::root(Vec::new()),
            crate::display::Conject::detached(),
        )
    }

    /// The occurrence follows `cell` to its definition from `source`, where
    /// it and everything beneath it start their locations.
    pub(crate) fn in_cell(&self, occurrence: Path, cell: CellId, source: Resolution) -> Self {
        self.layer(occurrence, Reading::Cell { cell, source })
    }

    pub(crate) fn source<'p>(&self, path: &'p [Step]) -> Option<Location<'p>> {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Conject);
        match &self.0 {
            None => Some(Location::root(path)),
            Some(layer) => match path.strip_prefix(layer.occurrence.as_slice()) {
                None => layer.within.source(path),
                Some(rest) => match &layer.reading {
                    Reading::Conject { location, conject } => {
                        conject.apply(rest, &location.path).map(|path| Location {
                            start: location.start,
                            path: path.into(),
                        })
                    }
                    Reading::Cell { cell, source } => Some(Location::in_cell(*cell, *source, rest)),
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
        match (self.source(path), other.source(path)) {
            (Some(left), Some(right)) => left
                .stored(sources)
                .zip(right.stored(sources))
                .map_or_else(|| left == right, |(left, right)| left == right),
            (left, right) => left.is_none() && right.is_none(),
        }
    }

    pub(crate) fn read<'a>(&self, sources: &Sources<'a>, path: &[Step]) -> Option<&'a Value> {
        self.view(*sources).resolve_path(path)
    }

    pub(crate) fn writable(&self, sources: &Sources<'_>, path: &[Step]) -> bool {
        self.source(path)
            .is_some_and(|location| crate::location::writable(sources, &location))
    }

    pub(crate) fn open<'a>(&'a self, access: Access<'a>) -> Edit<'a> {
        Edit {
            scope: self,
            access,
        }
    }
}

/// Read-only access in the same location interpretation as the writable view.
pub struct Read<'scope, 'data> {
    pub(crate) sources: Sources<'data>,
    scope: &'scope Scope,
}

impl<'data> Read<'_, 'data> {
    pub fn resolve_path(&self, path: &[Step]) -> Option<&'data Value> {
        self.sources.read(&self.scope.source(path)?)
    }

    pub(crate) fn writable(&self, path: &[Step]) -> bool {
        self.scope.writable(&self.sources, path)
    }
}

/// Dispatch-time access. Widgets cannot extract the root editor from this token.
pub(crate) struct Access<'a> {
    session: Session<'a>,
}

impl<'a> Access<'a> {
    pub(crate) fn new(editor: &'a mut Editor) -> Self {
        Self {
            session: editor.session(),
        }
    }

    /// Access through a session already lent, as a gesture has one.
    pub(crate) fn of(session: &'a mut Session<'_>) -> Self {
        Self {
            session: session.reborrow(),
        }
    }
}

/// A borrowed interface, not an editor copy or an allocated callback dictionary.
pub(crate) struct Edit<'a> {
    scope: &'a Scope,
    access: Access<'a>,
}

impl Edit<'_> {
    pub(crate) fn annotation(&self, root: &Root, path: &[Step]) -> Option<&Value> {
        self.access
            .session
            .model
            .workspace
            .view(root)?
            .annotations
            .at(path)
    }

    pub(crate) fn replace(&mut self, path: &[Step], value: Value) -> bool {
        let session = &mut self.access.session;
        let Some(source) = self.scope.source(path) else {
            return false;
        };
        if self.scope.read(&session.sources(), path) == Some(&value) {
            return true;
        }
        let before = session.model.snapshot();
        if !selection::set_value(&mut session.model.doc, session.libraries, &source, value) {
            return false;
        }
        session.model.history.record(before);
        true
    }

    pub(crate) fn select(&mut self, root: &Root, path: &[Step]) {
        let session = &mut self.access.session;
        if session.model.selection.as_ref().is_some_and(|selection| {
            selection.root() == root
                && selection.path() == path
                && !selection
                    .scope()
                    .same_location(self.scope, &session.sources(), path)
        }) {
            session.model.selection = None;
        }
        select(session, root, path);
        if let Some(selection) = &mut session.model.selection {
            selection.set_scope(self.scope.clone());
        }
    }

    pub(crate) fn select_payload(&mut self, root: &Root, path: Path, payload: Value) {
        let session = &mut self.access.session;
        let selection = selection::Selection::from_scoped_payload(
            root,
            &session.sources(),
            self.scope.clone(),
            path,
            payload,
        );
        session.model.selection = Some(selection);
    }

    pub(crate) fn annotate(&mut self, root: &Root, path: &[Step], value: Value) -> bool {
        // UI state belongs to the displayed occurrence, not the shared source.
        annotate(&mut self.access.session, root, path, value)
    }

    pub(crate) fn edit_line(
        &mut self,
        root: &Root,
        path: &[Step],
        line: &crate::display::LineEdit,
        operation: &puri::edit::EditOperation<'_>,
    ) -> bool {
        let session = &mut self.access.session;
        if session.model.selection.as_ref().is_none_or(|selection| {
            !selection
                .scope()
                .same_location(self.scope, &session.sources(), path)
        }) {
            return false;
        }
        edit_line(session, root, path, line, operation)
    }

    #[cfg(test)]
    pub(crate) fn grap(&mut self, root: Root, path: Path, function: Value, event: Value) -> bool {
        let function = grap::evaluate(
            &function,
            &self.access.session.sources(),
            grap::DEFAULT_FUEL,
        )
        .result;
        self.grap_runtime(root, path, function, event)
    }

    pub(crate) fn grap_runtime(
        &mut self,
        root: Root,
        path: Path,
        function: grap::RuntimeValue,
        event: Value,
    ) -> bool {
        crate::site::apply_scoped_event(
            &mut self.access.session,
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
