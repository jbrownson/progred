//! The editor's durable document state: document, selection, view,
//! and history.

use crate::history;
use crate::selection;
use crate::workspace;
use gid::{Document, Path, Step};
use progred_libraries::Libraries;
use std::rc::Rc;

/// The View menu's frame inputs.
#[derive(Clone, Copy, Default)]
pub(crate) struct ViewFlags {
    /// Overlay leaf rectangles and the pointer hysteresis geometry.
    pub debug_geometry: bool,
}

pub(crate) struct Model {
    pub doc: Rc<Document>,
    saved: Rc<Document>,
    pub selection: Option<selection::Selection>,
    pub history: history::History<Snapshot>,
    pub view: ViewFlags,
    /// Session-owned views over the document. Their layout, roots,
    /// projections, and scroll positions are not GID.
    pub workspace: workspace::Workspace,
}

pub(crate) struct Snapshot {
    doc: Rc<Document>,
    selection: Option<(workspace::Root, Path)>,
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
            view: ViewFlags::default(),
            workspace: workspace::Workspace::default(),
        }
    }

    pub fn dirty(&self) -> bool {
        !Rc::ptr_eq(&self.doc, &self.saved)
    }

    pub fn replace_document(&mut self, doc: Document) {
        *self = Self {
            view: self.view,
            ..Self::new(doc)
        };
    }

    #[cfg(any(test, target_os = "macos", target_os = "linux"))]
    pub fn mark_saved(&mut self) {
        self.saved = self.doc.clone();
        selection::break_edit_run(self.selection.as_mut());
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            doc: self.doc.clone(),
            selection: self.selection.as_ref().and_then(|selection| {
                selection
                    .history_path()
                    .map(|path| (selection.root().clone(), path.to_vec()))
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
            self.workspace
                .sync_declared(&workspace::declarations(self.doc.root.as_ref()));
            self.workspace.restore_folds(restored.folds);
            self.selection = restored
                .selection
                .map(|(root, path)| selection::Selection::edge(&root, path));
            true
        } else {
            false
        }
    }

    pub fn collapse(
        &mut self,
        libraries: &Libraries,
        root: &workspace::Root,
        path: &[Step],
        closed: Option<bool>,
    ) -> bool {
        let before = self.snapshot();
        let changed = self.workspace.view_mut(root).is_some_and(|view| {
            let sources = crate::sources::Sources {
                doc: &self.doc,
                libraries,
            };
            match closed {
                Some(closed) => {
                    selection::set_collapse(&sources, &mut view.annotations, path, closed)
                }
                None => selection::toggle_collapse(&sources, &mut view.annotations, path),
            }
        });
        if changed {
            selection::break_edit_run(self.selection.as_mut());
            self.history.record(before);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations;
    use crate::sources::Sources;
    use gid::{Cells, Value};
    use kurbo::Vec2;

    fn model() -> Model {
        Model::new(Document {
            root: Some(Value::list([Value::from(vec![1])])),
            cells: Cells::new(),
        })
    }

    fn edit(model: &mut Model, value: u8) {
        let before = model.snapshot();
        assert!(selection::set_value(
            &mut model.doc,
            &Libraries::default(),
            &[],
            Value::list([Value::from(vec![value])])
        ));
        model.history.record(before);
    }

    #[test]
    fn replacing_a_document_drops_its_history_selection_and_all_view_state() {
        let (root, pane_path) =
            workspace::append(&Value::record([]), workspace::Side::Left, Value::record([]))
                .unwrap();
        let mut model = Model::new(Document {
            root: Some(root),
            cells: Cells::new(),
        });
        let old_cell = gid::new_cell_id();
        for byte in [1, 2] {
            let before = model.snapshot();
            Rc::make_mut(&mut model.doc)
                .cells
                .set_value(old_cell, Value::from(vec![byte]));
            model.history.record(before);
        }
        assert!(model.step_history(true));
        assert!(model.dirty());
        assert!(model.history.can_undo() && model.history.can_redo());
        let old_root = model.workspace.document.root.clone();
        let pane_root = model.workspace.left.panes[0].view.root.clone();
        model.selection = Some(selection::pending_with_query(
            &pane_root,
            pane_path,
            "old query",
        ));
        model
            .selection
            .as_mut()
            .unwrap()
            .edit_mut()
            .unwrap()
            .handle_ime(&puri::handler::ImeEvent::Preedit(
                "old composition".into(),
                Some((1, 1)),
            ));
        model.workspace.document.scroll = Vec2::new(12.0, 300.0);
        model.workspace.document.projection = workspace::Projection::Raw;
        model.workspace.left_width = 0.5;
        model.workspace.right_width = 0.1;
        for view in [
            &mut model.workspace.document,
            &mut model.workspace.left.panes[0].view,
        ] {
            view.annotations
                .set(&[], Some(Value::record([(old_cell, Value::from(vec![1]))])));
        }
        model.view.debug_geometry = true;
        let replacement = Document {
            root: Some(Value::record([])),
            cells: Cells::new(),
        };
        model.replace_document(replacement.clone());
        assert_eq!(model.doc.root, replacement.root);
        assert!(model.doc.cells.value(old_cell).is_none());
        assert!(!model.dirty());
        assert!(!model.history.can_undo() && !model.history.can_redo());
        assert!(model.selection.is_none());
        assert!(model.workspace.view(&old_root).is_none());
        assert!(model.workspace.view(&pane_root).is_none());
        assert!(model.workspace.left.panes.is_empty() && model.workspace.right.panes.is_empty());
        assert_eq!(model.workspace.document.scroll, Vec2::ZERO);
        assert_eq!(
            model.workspace.document.projection,
            workspace::Projection::Standard
        );
        assert!(model.workspace.document.annotations.at(&[]).is_none());
        let fresh = workspace::Workspace::default();
        assert_eq!(model.workspace.left_width, fresh.left_width);
        assert_eq!(model.workspace.right_width, fresh.right_width);
        assert!(model.view.debug_geometry);
    }

    fn select(model: &mut Model, root: &workspace::Root, path: Path) {
        model.selection = Some(selection::Selection::edge(root, path));
    }

    #[test]
    fn saved_identity_survives_undo_redo_but_not_recreating_equal_contents() {
        let mut model = model();
        let original = model.doc.clone();
        assert!(!model.dirty());
        edit(&mut model, 2);
        assert!(model.dirty());
        model.mark_saved();
        let saved = model.doc.clone();
        edit(&mut model, 3);
        assert!(model.dirty());
        assert!(model.step_history(true));
        assert!(!model.dirty());
        assert!(Rc::ptr_eq(&model.doc, &saved));
        assert!(model.step_history(true));
        assert!(model.dirty());
        assert!(Rc::ptr_eq(&model.doc, &original));
        assert!(model.step_history(false));
        assert!(!model.dirty());
        edit(&mut model, 3);
        edit(&mut model, 2);
        assert_eq!(model.doc.root, saved.root);
        assert!(model.dirty());
        assert!(model.step_history(true));
        assert!(model.step_history(true));
        assert!(!model.dirty());
    }

    #[test]
    fn branching_before_at_and_after_save_cannot_confuse_the_saved_identity() {
        for steps_back in [1, 2, 3] {
            let mut model = model();
            edit(&mut model, 2);
            model.mark_saved();
            let saved = model.doc.clone();
            edit(&mut model, 3);
            edit(&mut model, 4);
            for _ in 0..steps_back {
                assert!(model.step_history(true));
            }
            edit(&mut model, 5);
            assert!(model.dirty());
            assert!(!model.history.can_redo());
            let mut reached_save = false;
            while model.step_history(true) {
                assert_eq!(model.dirty(), !Rc::ptr_eq(&model.doc, &saved));
                reached_save |= !model.dirty();
            }
            assert_eq!(reached_save, steps_back < 3);
        }
    }

    #[test]
    fn folds_undo_independently_of_document_edits_and_never_dirty_the_document() {
        let mut model = model();
        let root = model.workspace.document_root().clone();
        select(&mut model, &root, vec![]);
        assert!(!model.collapse(&Libraries::default(), &root, &[], Some(false)));
        assert!(!model.history.can_undo());
        edit(&mut model, 2);
        let edited = model.doc.clone();
        assert!(model.collapse(&Libraries::default(), &root, &[], None));
        assert!(Rc::ptr_eq(&model.doc, &edited));
        model.mark_saved();
        assert!(model.step_history(true));
        assert!(!annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(!model.dirty());
        assert!(model.step_history(true));
        assert!(model.dirty());
        assert!(model.step_history(false));
        assert!(!model.dirty());
        assert!(model.step_history(false));
        assert!(annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(!model.dirty());
        assert!(model.collapse(&Libraries::default(), &root, &[], Some(false)));
        assert!(!model.dirty());
    }

    #[test]
    fn folds_and_selection_restore_to_their_own_pane_even_after_pane_deletion() {
        let first = workspace::append(
            &Value::record([]),
            workspace::Side::Left,
            Value::list([Value::from(vec![1])]),
        )
        .unwrap();
        let second = workspace::append(
            &first.0,
            workspace::Side::Left,
            Value::list([Value::from(vec![1])]),
        )
        .unwrap();
        let mut model = Model::new(Document {
            root: Some(second.0),
            cells: Cells::new(),
        });
        model
            .workspace
            .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
        let first_root = model.workspace.left.panes[0].view.root.clone();
        let second_root = model.workspace.left.panes[1].view.root.clone();
        select(&mut model, &first_root, first.1.clone());
        assert!(model.collapse(&Libraries::default(), &first_root, &first.1, None));
        assert!(!model.dirty());
        let before = model.snapshot();
        assert!(selection::delete_edge(
            &mut model.doc,
            &Libraries::default(),
            &first.1
        ));
        model.history.record(before);
        model
            .workspace
            .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
        select(&mut model, &second_root, second.1);
        assert!(model.workspace.view(&first_root).is_none());
        assert!(model.step_history(true));
        assert_eq!(model.selection.as_ref().unwrap().root(), &first_root);
        assert!(annotations::collapsed(
            &model.workspace.view(&first_root).unwrap().annotations,
            &first.1,
            false
        ));
        assert!(!model.dirty());
        assert!(model.step_history(true));
        assert!(!annotations::collapsed(
            &model.workspace.view(&first_root).unwrap().annotations,
            &first.1,
            false
        ));
        assert!(model.workspace.view(&second_root).is_some());
        assert!(!model.dirty());
        assert!(model.step_history(false));
        assert!(model.step_history(false));
        assert!(model.workspace.view(&first_root).is_none());
        assert_eq!(model.selection.as_ref().unwrap().root(), &second_root);
        assert!(model.dirty());
    }

    #[test]
    fn undo_does_not_restore_scroll_projection_or_unrelated_annotations() {
        let mut model = model();
        let root = model.workspace.document_root().clone();
        assert!(model.collapse(&Libraries::default(), &root, &[], None));
        let state = gid::new_cell_id();
        model
            .workspace
            .document
            .annotations
            .set_field(&[], state, Some(Value::from(vec![5])));
        model
            .workspace
            .document
            .annotations
            .set(&[Step::Key(state)], Some(Value::from(vec![6])));
        model.workspace.document.scroll = Vec2::new(50.0, 75.0);
        model.workspace.document.projection = workspace::Projection::Raw;
        assert!(model.step_history(true));
        assert_eq!(model.workspace.document.scroll, Vec2::new(50.0, 75.0));
        assert_eq!(
            model.workspace.document.projection,
            workspace::Projection::Raw
        );
        assert_eq!(
            model.workspace.document.annotations.field(&[], state),
            Some(&Value::from(vec![5]))
        );
        assert_eq!(
            model.workspace.document.annotations.at(&[Step::Key(state)]),
            Some(&Value::from(vec![6]))
        );
        assert!(!model.dirty());
    }

    #[test]
    fn rejected_writes_do_not_detach_the_saved_snapshot() {
        let mut model = model();
        let bad_path = [Step::Key(gid::new_cell_id()), Step::Key(gid::new_cell_id())];
        assert!(!selection::set_value(
            &mut model.doc,
            &Libraries::default(),
            &bad_path,
            Value::record([])
        ));
        assert!(!selection::delete_edge(
            &mut model.doc,
            &Libraries::default(),
            &bad_path
        ));
        assert!(!model.dirty());
        let mut empty = Model::new(Document {
            root: None,
            cells: Cells::new(),
        });
        assert!(!selection::delete_edge(
            &mut empty.doc,
            &Libraries::default(),
            &[]
        ));
        assert!(!empty.dirty());
    }

    #[test]
    fn cell_writes_preserve_the_saved_document_without_needing_a_history_record() {
        let cell = gid::new_cell_id();
        let mut cells = Cells::new();
        cells.set_value(cell, Value::from(vec![1]));
        let mut model = Model::new(Document {
            root: Some(cell.into()),
            cells,
        });
        let saved = model.doc.clone();
        assert!(selection::set_value(
            &mut model.doc,
            &Libraries::default(),
            &[Step::Follow(gid::Resolution::Document)],
            Value::from(vec![2])
        ));
        assert!(model.dirty());
        assert_eq!(saved.cells.value(cell), Some(&Value::from(vec![1])));
        assert_eq!(model.doc.cells.value(cell), Some(&Value::from(vec![2])));
        assert_eq!(model.doc.root, saved.root);
        assert!(!model.history.can_undo());
    }

    #[test]
    fn saving_and_folding_break_typing_runs() {
        use progred_libraries::text;
        let field = gid::new_cell_id();
        let path = vec![Step::Key(field)];
        let libraries = crate::stack::load::<()>().libraries;
        let mut model = Model::new(Document {
            root: Some(Value::record([(field, text::value("a"))])),
            cells: Cells::new(),
        });
        let root = model.workspace.document_root().clone();
        model.selection = Some(selection::Selection::from_line(
            &root,
            &Sources {
                doc: &model.doc,
                libraries: &libraries,
            },
            path.clone(),
            progred_display::LineEdit {
                text: "a".into(),
                placeholder: None,
                update: progred_libraries::line_edit::native(text::edit),
                prefix: "\"".into(),
                suffix: "\"".into(),
                family: Default::default(),
            },
        ));
        let type_text = |model: &mut Model, text| {
            let before = model.snapshot();
            let selection = model.selection.as_mut().unwrap();
            selection.edit_mut().unwrap().set_text(text);
            if crate::projection::line_control::commit(
                &mut model.doc,
                &libraries,
                selection,
                &progred_libraries::line_edit::native(progred_libraries::text::edit),
            ) {
                model.history.record(before);
            }
        };
        let spelling = |model: &Model| {
            Sources {
                doc: &model.doc,
                libraries: &libraries,
            }
            .resolve_path(&path)
            .and_then(text::read)
            .unwrap()
            .to_owned()
        };
        type_text(&mut model, "ab");
        type_text(&mut model, "abc");
        model.mark_saved();
        type_text(&mut model, "abcd");
        assert!(model.collapse(&libraries, &root, &[], None));
        type_text(&mut model, "abcde");
        assert!(model.step_history(true));
        assert_eq!(spelling(&model), "abcd");
        assert!(annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(model.step_history(true));
        assert_eq!(spelling(&model), "abcd");
        assert!(!annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(model.dirty());
        assert!(model.step_history(true));
        assert_eq!(spelling(&model), "abc");
        assert!(!model.dirty());
        assert!(model.step_history(true));
        assert_eq!(spelling(&model), "a");
        assert!(!model.history.can_undo());
        assert!(model.dirty());
    }
}
