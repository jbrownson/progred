use crate::document::{Editor, EditorWriter};
use crate::graph::{Id, Path, Selection};
use eframe::egui::{self, Color32, Context, RichText, Sense, Ui};

use super::layout::TREE_MARGIN;
use super::placeholder::PlaceholderResult;
use super::{insertion_point, render_d, InteractionMode};

// TODO: separate D generation from egui chrome (scroll area, margin, background click).
// Once roots become a single list node, this simplifies to: generate D, render in scroll area.
pub fn render(ui: &mut Ui, ctx: &Context, editor: &Editor, w: &mut EditorWriter) {
    let modifiers = ctx.input(|i| i.modifiers);
    let selected_path = editor.selection.as_ref().and_then(|s| s.edge_path());
    let mode = if modifiers.alt {
        match selected_path {
            Some(path) => InteractionMode::Assign(path.clone()),
            _ => InteractionMode::Normal,
        }
    } else if modifiers.ctrl {
        match selected_path {
            Some(path) if matches!(editor.doc.node(path), Some(Id::Uuid(_))) => {
                InteractionMode::SelectUnder(path.clone())
            }
            _ => InteractionMode::Normal,
        }
    } else {
        InteractionMode::Normal
    };

    let margin = egui::Margin::same(TREE_MARGIN as i8);
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
        let bg_response = ui.interact(
            ui.clip_rect(),
            ui.id().with("background"),
            Sense::click(),
        );

        ui.push_id("roots", |ui| {
            if editor.doc.roots.is_empty() {
                render_root_insertion(ui, editor, w, 0, true);
            } else {
                for (i, root_slot) in editor.doc.roots.iter().enumerate() {
                    render_root_insertion(ui, editor, w, i, false);
                    ui.push_id(root_slot, |ui| {
                        let path = Path::new(root_slot);
                        if let Some(id) = editor.doc.node(&path) {
                            let d = crate::render::render(editor, &path, &id);
                            render_d(ui, editor, w, &d, &mode);
                        }
                    });
                }
                render_root_insertion(ui, editor, w, editor.doc.roots.len(), false);
            }
        });

        let orphan_ids = editor.orphan_roots();
        if !orphan_ids.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("orphans").color(Color32::from_gray(100)).italics().size(11.0));
            ui.add_space(4.0);
            for orphan_id in orphan_ids {
                ui.push_id(orphan_id, |ui| {
                    let path = Path::orphan(orphan_id.clone());
                    if let Some(id) = editor.doc.node(&path) {
                        let d = crate::render::render(editor, &path, &id);
                        render_d(ui, editor, w, &d, &mode);
                    }
                });
                ui.add_space(2.0);
            }
        }

        if bg_response.clicked() {
            w.select(None);
        }
        });
    });
}

fn render_root_insertion(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, index: usize, empty_doc: bool) {
    let active_placeholder = matches!(
        &editor.selection,
        Some(Selection::InsertRoot(idx, _)) if *idx == index
    );

    if active_placeholder {
        if let Some(Selection::InsertRoot(_, ps)) = &editor.selection {
            let mut ps = ps.clone();
            match super::placeholder::render(ui, &mut ps) {
                PlaceholderResult::Commit(id) => {
                    w.insert_root(index, id);
                    w.select(None);
                }
                PlaceholderResult::Dismiss => w.select(None),
                PlaceholderResult::Active => w.set_placeholder_state(ps),
            }
        }
    } else if empty_doc {
        let response = ui.add(
            egui::Button::new(
                egui::RichText::new("(empty)")
                    .color(Color32::from_gray(120))
                    .italics()
            ).frame(false)
        ).on_hover_cursor(egui::CursorIcon::Default);
        if response.clicked() {
            w.select(Some(Selection::insert_root(index)));
        }
    } else if insertion_point(ui).clicked() {
        w.select(Some(Selection::insert_root(index)));
    }
}
