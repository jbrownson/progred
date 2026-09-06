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

    #[cfg(any(test, target_os = "macos", target_os = "linux"))]
    pub fn mark_saved(&mut self) {
        self.saved = self.doc.clone();
        selection::break_edit_run(self.selection.as_mut());
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            doc: self.doc.clone(),
            selection: self
                .selection
                .as_ref()
                .filter(|selection| selection.stage() == selection::Stage::Edge)
                .map(|selection| (selection.root().clone(), selection.path().to_vec())),
            folds: self.workspace.folds(),
        }
    }

    pub fn step_history(&mut self, back: bool, libraries: &Libraries) -> bool {
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
            self.selection = restored.selection.map(|(root, path)| {
                selection::Selection::edge(
                    &root,
                    &crate::sources::Sources {
                        doc: &self.doc,
                        libraries,
                    },
                    path,
                )
            });
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

    fn select(model: &mut Model, root: &workspace::Root, path: Path) {
        model.selection = Some(selection::Selection::edge(
            root,
            &Sources {
                doc: &model.doc,
                libraries: &Libraries::default(),
            },
            path,
        ));
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
        assert!(model.step_history(true, &Libraries::default()));
        assert!(!model.dirty());
        assert!(Rc::ptr_eq(&model.doc, &saved));
        assert!(model.step_history(true, &Libraries::default()));
        assert!(model.dirty());
        assert!(Rc::ptr_eq(&model.doc, &original));
        assert!(model.step_history(false, &Libraries::default()));
        assert!(!model.dirty());
        edit(&mut model, 3);
        edit(&mut model, 2);
        assert_eq!(model.doc.root, saved.root);
        assert!(model.dirty());
        assert!(model.step_history(true, &Libraries::default()));
        assert!(model.step_history(true, &Libraries::default()));
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
                assert!(model.step_history(true, &Libraries::default()));
            }
            edit(&mut model, 5);
            assert!(model.dirty());
            assert!(!model.history.can_redo());
            let mut reached_save = false;
            while model.step_history(true, &Libraries::default()) {
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
        assert!(model.step_history(true, &Libraries::default()));
        assert!(!annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(!model.dirty());
        assert!(model.step_history(true, &Libraries::default()));
        assert!(model.dirty());
        assert!(model.step_history(false, &Libraries::default()));
        assert!(!model.dirty());
        assert!(model.step_history(false, &Libraries::default()));
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
        assert!(model.step_history(true, &Libraries::default()));
        assert_eq!(model.selection.as_ref().unwrap().root(), &first_root);
        assert!(annotations::collapsed(
            &model.workspace.view(&first_root).unwrap().annotations,
            &first.1,
            false
        ));
        assert!(!model.dirty());
        assert!(model.step_history(true, &Libraries::default()));
        assert!(!annotations::collapsed(
            &model.workspace.view(&first_root).unwrap().annotations,
            &first.1,
            false
        ));
        assert!(model.workspace.view(&second_root).is_some());
        assert!(!model.dirty());
        assert!(model.step_history(false, &Libraries::default()));
        assert!(model.step_history(false, &Libraries::default()));
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
        assert!(model.step_history(true, &Libraries::default()));
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
                update: grap::ffi(text::vocabulary::UPDATE),
                prefix: "\"".into(),
                suffix: "\"".into(),
                family: Default::default(),
            },
        ));
        let type_text = |model: &mut Model, text| {
            let before = model.snapshot();
            let selection = model.selection.as_mut().unwrap();
            selection.edit_mut().unwrap().set_text(text);
            if selection::write_through(&mut model.doc, &libraries, selection) {
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
        assert!(model.step_history(true, &libraries));
        assert_eq!(spelling(&model), "abcd");
        assert!(annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(model.step_history(true, &libraries));
        assert_eq!(spelling(&model), "abcd");
        assert!(!annotations::collapsed(
            &model.workspace.document.annotations,
            &[],
            false
        ));
        assert!(model.dirty());
        assert!(model.step_history(true, &libraries));
        assert_eq!(spelling(&model), "abc");
        assert!(!model.dirty());
        assert!(model.step_history(true, &libraries));
        assert_eq!(spelling(&model), "a");
        assert!(!model.history.can_undo());
        assert!(model.dirty());
    }
}
