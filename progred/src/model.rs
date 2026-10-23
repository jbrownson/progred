//! The editor's durable document state: document, selection, view,
//! and history.

use crate::display::widget::view::ViewId;
use crate::history;
use crate::occurrence::Step;
use crate::selection;
use crate::workspace;
use gid::Document;
use std::rc::Rc;

pub(crate) struct Model {
    pub doc: Rc<Document>,
    saved: Rc<Document>,
    pub selection: Option<selection::Selection>,
    pub history: history::History<Snapshot>,
    /// Session-owned views over the document. Their layout, roots,
    /// projections, and scroll positions are not GID.
    pub workspace: workspace::Workspace,
}

pub(crate) struct Snapshot {
    doc: Rc<Document>,
    /// The selection's view and path.
    selection: Option<(ViewId, crate::occurrence::Path)>,
    folds: workspace::Folds,
}

impl Model {
    pub fn new(doc: Document) -> Self {
        let doc = Rc::new(doc);
        Self {
            saved: doc.clone(),
            doc,
            selection: None,
            history: history::History::default(),
            workspace: workspace::Workspace::default(),
        }
    }

    pub fn dirty(&self) -> bool {
        !Rc::ptr_eq(&self.doc, &self.saved)
    }

    pub fn replace_document(&mut self, doc: Document) {
        *self = Self::new(doc);
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.doc.clone();
        self.break_edit_run();
    }

    /// Breaks the open edit run: the next write records a fresh undo
    /// step. Runs must not straddle a save or a view-history step.
    fn break_edit_run(&mut self) {
        if let Some(selection) = &mut self.selection {
            selection.set_recorded(false);
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            doc: self.doc.clone(),
            selection: self.selection.as_ref().and_then(|selection| {
                selection
                    .history_path()
                    .map(|path| (selection.view(), path.to_vec()))
            }),
            folds: self.workspace.folds(),
        }
    }

    pub fn step_history(&mut self, back: bool) -> bool {
        let current = self.snapshot();
        let restored = if back {
            self.history.undo(current)
        } else {
            self.history.redo(current)
        };
        if let Some(restored) = restored {
            self.doc = restored.doc;
            self.workspace.sync(self.doc.root.as_ref());
            self.workspace.restore_folds(restored.folds);
            self.selection = restored
                .selection
                .map(|(view, path)| selection::Selection::edge(view, path));
            true
        } else {
            false
        }
    }

    pub(crate) fn set_folded(
        &mut self,
        view: ViewId,
        path: &[Step],
        default: bool,
        closed: Option<bool>,
    ) -> bool {
        let before = self.snapshot();
        let changed = self.workspace.view_at_mut(view, path).is_some_and(|view| {
            let previous = crate::annotations::folded(view.annotations.at(path), default);
            let next = closed.unwrap_or(!previous);
            if previous == next {
                false
            } else {
                crate::annotations::set_folded(&mut view.annotations, path, default, next);
                true
            }
        });
        if changed {
            self.break_edit_run();
            self.history.record(before);
        }
        changed
    }
}

#[cfg(test)]
mod tests;
