//! One projected occurrence as a place to act: the view it's drawn in, its
//! path there, and the scope that reads that path. Widgets act on their place
//! through it, without carrying the view or the path themselves.

use super::{Access, Edit, Scope, Session};
use crate::hover::Hover;
use crate::workspace::Root;
use gid::{Step, Value};
use puri::edit::LineEditState;
use std::rc::Rc;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct Site {
    root: Root,
    path: Rc<[Step]>,
    scope: Scope,
}

impl Site {
    pub(crate) fn new(root: Root, path: Rc<[Step]>, scope: Scope) -> Self {
        Self { root, path, scope }
    }

    /// The occurrence `steps` below this one, read through the same scope.
    pub(crate) fn below(&self, steps: &[Step]) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.iter().chain(steps).cloned().collect(),
            scope: self.scope.clone(),
        }
    }

    /// Whether `selection` is at this occurrence.
    pub(crate) fn holds(&self, selection: &crate::selection::Selection) -> bool {
        selection.root() == &self.root && selection.path() == &*self.path
    }

    /// Where this occurrence is stored, if anywhere: what a jump to it
    /// draws.
    pub(crate) fn location(&self) -> Option<crate::location::Location<'static>> {
        self.scope
            .location(&self.path)
            .map(crate::location::Location::into_owned)
    }

    /// What hovering this occurrence shows.
    pub(crate) fn hover(&self) -> Hover {
        Hover::Value(self.path.clone())
    }

    /// Work `computations` keeps for this occurrence across frames, made by
    /// `create` the first time.
    pub(crate) fn memo<T: 'static>(
        &self,
        computations: &crate::computations::Computations,
        create: impl FnOnce() -> T,
    ) -> Rc<T> {
        computations.at(&self.root, &self.path, create)
    }

    /// A drag's writes to the value here, recorded as one undo step.
    pub(crate) fn edit_run(&self) -> crate::gesture::EditRun {
        crate::gesture::scoped_value_edit(self.root.clone(), self.path.to_vec(), self.scope.clone())
    }

    /// Edits of this occurrence, through the session a dispatch lends.
    pub(crate) fn open<'s>(&'s self, session: &'s mut Session<'_>) -> SiteEdit<'s> {
        SiteEdit {
            site: self,
            edit: self.scope.open(Access::of(session)),
        }
    }
}

/// Edits of one occurrence, bound to the session they act through.
pub(crate) struct SiteEdit<'s> {
    site: &'s Site,
    edit: Edit<'s>,
}

/// Whether an occurrence is selected.
pub(crate) enum Selected<'s> {
    Elsewhere,
    Here,
    /// Selected, with its value's text being typed.
    Typing(&'s LineEditState),
}

impl SiteEdit<'_> {
    /// The value stored here.
    pub(crate) fn value(&self) -> Option<&Value> {
        self.edit.value(&self.site.path)
    }

    /// Whether this occurrence is selected, and the line being typed in its
    /// value if one is.
    pub(crate) fn selected(&self) -> Selected<'_> {
        match self
            .edit
            .selection()
            .filter(|selection| self.site.holds(selection))
        {
            None => Selected::Elsewhere,
            Some(selection) => selection
                .value_edit()
                .map_or(Selected::Here, Selected::Typing),
        }
    }

    /// This occurrence's state, which its view keeps.
    pub(crate) fn state(&self) -> Option<&Value> {
        self.edit.annotation(&self.site.root, &self.site.path)
    }

    /// Replaces this occurrence's state; false when it was already `state`.
    pub(crate) fn set_state(&mut self, state: Value) -> bool {
        self.edit.annotate(&self.site.root, &self.site.path, state)
    }

    /// Writes `value` where this occurrence is stored.
    pub(crate) fn replace(&mut self, value: Value) -> bool {
        self.edit.replace(&self.site.path, value)
    }

    pub(crate) fn select(&mut self) {
        self.edit.select(&self.site.root, &self.site.path);
    }

    /// Opens or closes this occurrence's fold, whose state is `default`
    /// until set, toggling it without `closed`.
    pub(crate) fn fold(&mut self, default: bool, closed: Option<bool>) -> bool {
        self.edit
            .fold(&self.site.root, &self.site.path, default, closed)
    }

    /// Leaves the value `steps` below this occurrence pending, for the
    /// picker to fill.
    pub(crate) fn pend(&mut self, steps: &[Step]) {
        self.edit.pend(
            &self.site.root,
            self.site.path.iter().chain(steps).cloned().collect(),
        );
    }

    /// Selects this occurrence with a projection's own selection payload.
    pub(crate) fn select_with(&mut self, payload: Value) {
        self.edit
            .select_payload(&self.site.root, self.site.path.to_vec(), payload);
    }

    pub(crate) fn edit_line(
        &mut self,
        line: &crate::display::LineEdit,
        operation: &puri::edit::EditOperation<'_>,
    ) -> bool {
        self.edit
            .edit_line(&self.site.root, &self.site.path, line, operation)
    }

    /// Runs a Grap event handler here, with `event` as its argument.
    pub(crate) fn run_handler(&mut self, function: grap::RuntimeValue, event: Value) -> bool {
        self.edit.grap_runtime(
            self.site.root.clone(),
            self.site.path.to_vec(),
            function,
            event,
        )
    }
}
