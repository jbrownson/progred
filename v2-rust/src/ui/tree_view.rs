use crate::document::{Editor, EditorWriter};
use crate::graph::{Id, Path, Selection, SelectionTarget};
use eframe::egui::{self, Color32, Context, Sense, Ui};

use super::placeholder::PlaceholderResult;
use super::{insertion_point, project, InteractionMode};

pub fn render(ui: &mut Ui, ctx: &Context, editor: &Editor, w: &mut EditorWriter) {
    let bg_response = ui.interact(
        ui.max_rect(),
        ui.id().with("background"),
        Sense::click(),
    );

    let modifiers = ctx.input(|i| i.modifiers);
    let mode = if modifiers.alt {
        match editor.selection.as_ref().and_then(|s| s.edge_path()) {
            Some(path) => InteractionMode::Assign(path.clone()),
            _ => InteractionMode::Normal,
        }
    } else if modifiers.ctrl {
        match editor.selection.as_ref().and_then(|s| s.edge_path()) {
            Some(path) if matches!(path.node(&editor.doc.gid), Some(Id::Uuid(_))) => {
                InteractionMode::SelectUnder(path.clone())
            }
            _ => InteractionMode::Normal,
        }
    } else {
        InteractionMode::Normal
    };

    if editor.doc.roots.is_empty() {
        render_root_insertion(ui, editor, w, 0, true);
    } else {
        for (i, root_slot) in editor.doc.roots.iter().enumerate() {
            render_root_insertion(ui, editor, w, i, false);
            ui.push_id(root_slot, |ui| {
                project(ui, editor, w, &Path::new(root_slot.clone()), &mode);
            });
        }
        render_root_insertion(ui, editor, w, editor.doc.roots.len(), false);
    }

    let orphan_roots = editor.doc.orphan_roots();
    if !orphan_roots.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("orphans").color(Color32::from_gray(100)).italics().size(11.0));
        ui.add_space(4.0);
        for orphan_id in orphan_roots {
            ui.push_id(&orphan_id, |ui| {
                let orphan_slot = crate::graph::RootSlot::new(orphan_id.clone());
                project(ui, editor, w, &Path::new(orphan_slot), &mode);
            });
            ui.add_space(2.0);
        }
    }

    if bg_response.clicked() {
        w.select(None);
    }
}

fn render_root_insertion(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, index: usize, empty_doc: bool) {
    let active_placeholder = matches!(
        &editor.selection,
        Some(Selection { target: SelectionTarget::InsertRoot(idx), .. }) if *idx == index
    );

    if active_placeholder {
        if let Some(ps) = w.placeholder_state() {
            match super::placeholder::render(ui, ps) {
                PlaceholderResult::Commit(id) => {
                    w.insert_root(index, crate::graph::RootSlot::new(id));
                    w.select(None);
                }
                PlaceholderResult::Dismiss => w.select(None),
                PlaceholderResult::Active => {}
            }
        }
    } else if empty_doc {
        let response = ui.add(
            egui::Label::new(
                egui::RichText::new("(empty)")
                    .color(Color32::from_gray(120))
                    .italics()
            ).sense(Sense::click())
        );
        if response.clicked() {
            w.select(Some(Selection::insert_root(index)));
        }
    } else if insertion_point(ui).clicked() {
        w.select(Some(Selection::insert_root(index)));
    }
}
