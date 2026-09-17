//! A composable interpretation of locations, shared by reads and writes.
//! Scopes own no editor state. Mutable access is lent only during dispatch.

use super::*;
use std::borrow::Cow;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct Scope(Option<Rc<Route>>);

#[derive(Debug)]
struct Route {
    parent: Scope,
    occurrence: Path,
    document: Path,
    conject: crate::display::Conject,
}

impl Scope {
    pub(crate) fn is_identity(&self) -> bool {
        self.0.is_none()
    }
    pub(crate) fn view<'scope, 'data>(
        &'scope self,
        sources: Sources<'data>,
    ) -> Read<'scope, 'data> {
        Read {
            sources,
            scope: self,
        }
    }
    /// Install a conject for this occurrence. Other occurrences keep their
    /// enclosing interpretation; the function sees the suffix below this one.
    pub(crate) fn with_conject(
        &self,
        occurrence: Path,
        document: Path,
        conject: crate::display::Conject,
    ) -> Self {
        Self(Some(Rc::new(Route {
            parent: self.clone(),
            occurrence,
            document,
            conject,
        })))
    }

    pub(crate) fn detached(&self, occurrence: Path) -> Self {
        self.with_conject(occurrence, Vec::new(), crate::display::Conject::detached())
    }

    pub(crate) fn source<'p>(&self, path: &'p [Step]) -> Option<Cow<'p, [Step]>> {
        let Some(route) = &self.0 else {
            return Some(Cow::Borrowed(path));
        };
        match path.strip_prefix(route.occurrence.as_slice()) {
            Some(rest) => route.conject.apply(rest, &route.document).map(Cow::Owned),
            None => route.parent.source(path),
        }
    }

    /// Compare the meaning of this occurrence, not closure allocation identity.
    pub(crate) fn same_location(&self, other: &Self, path: &[Step]) -> bool {
        self.source(path) == other.source(path)
    }

    pub(crate) fn read<'a>(&self, sources: &Sources<'a>, path: &[Step]) -> Option<&'a Value> {
        self.view(*sources).resolve_path(path)
    }

    pub(crate) fn writable(&self, sources: &Sources<'_>, path: &[Step]) -> bool {
        self.source(path)
            .is_some_and(|path| selection::writable_at(sources, &path))
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
        self.sources.resolve_path(&self.scope.source(path)?)
    }

    pub(crate) fn writable(&self, path: &[Step]) -> bool {
        self.scope.writable(&self.sources, path)
    }
}

/// Dispatch-time access. Widgets cannot extract the root editor from this token.
pub(crate) struct Access<'a> {
    editor: &'a mut Editor,
}

impl<'a> Access<'a> {
    pub(crate) fn new(editor: &'a mut Editor) -> Self {
        Self { editor }
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
            .editor
            .model
            .workspace
            .view(root)?
            .annotations
            .at(path)
    }

    pub(crate) fn replace(&mut self, path: &[Step], value: Value) -> bool {
        let app = &mut self.access.editor;
        let Some(source) = self.scope.source(path) else {
            return false;
        };
        if self.scope.read(&app.sources(), path) == Some(&value) {
            return true;
        }
        let before = app.model.snapshot();
        if !selection::set_value(&mut app.model.doc, &app.stack.libraries, &source, value) {
            return false;
        }
        app.model.history.record(before);
        app.refresh_title();
        true
    }

    pub(crate) fn select(&mut self, root: &Root, path: &[Step]) {
        if self
            .access
            .editor
            .model
            .selection
            .as_ref()
            .is_some_and(|selection| {
                selection.root() == root
                    && selection.path() == path
                    && !selection.scope().same_location(self.scope, path)
            })
        {
            self.access.editor.model.selection = None;
        }
        select(self.access.editor, root, path);
        if let Some(selection) = &mut self.access.editor.model.selection {
            selection.set_scope(self.scope.clone());
        }
    }

    pub(crate) fn select_payload(&mut self, root: &Root, path: Path, payload: Value) {
        self.access.editor.model.selection = Some(selection::Selection::from_scoped_payload(
            root,
            &self.access.editor.sources(),
            self.scope.clone(),
            path,
            payload,
        ));
    }

    pub(crate) fn annotate(&mut self, root: &Root, path: &[Step], value: Value) -> bool {
        // UI state belongs to the displayed occurrence, not the shared source.
        annotate(self.access.editor, root, path, value)
    }

    pub(crate) fn edit_line(
        &mut self,
        root: &Root,
        path: &[Step],
        line: &crate::display::LineEdit,
        operation: &puri::edit::EditOperation<'_>,
    ) -> bool {
        if self
            .access
            .editor
            .model
            .selection
            .as_ref()
            .is_none_or(|selection| !selection.scope().same_location(self.scope, path))
        {
            return false;
        }
        edit_line(self.access.editor, root, path, line, operation)
    }

    pub(crate) fn grap(&mut self, root: Root, path: Path, function: Value, event: Value) -> bool {
        crate::site::apply_scoped_event(
            self.access.editor,
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
