use crate::d::D;
use crate::editor::Editor;
use crate::graph::{Id, Path, RootSlot, Selection};
use eframe::egui::{self, Color32, Context, RichText, Sense, Ui};
use std::collections::HashSet;

use super::identicon;
use super::layout::TREE_MARGIN;
use super::placeholder::PlaceholderResult;
use super::{insertion_point, render_d, DContext, InteractionMode};

pub fn generate(editor: &Editor) -> Vec<Option<D>> {
    editor.doc.roots.iter()
        .map(|root_slot| {
            let path = Path::new(root_slot);
            let id = editor.doc.node(&path)?;
            Some(crate::render::render(editor, &path, &id))
        })
        .collect()
}

pub fn render(ui: &mut Ui, ctx: &Context, editor: &mut Editor, d_trees: &[Option<D>], orphan_ids: &HashSet<Id>) {
    let modifiers = ctx.input(|i| i.modifiers);
    let selected_path = editor.selection.as_ref().and_then(|s| s.edge_path()).cloned();
    let mode = if modifiers.alt {
        match selected_path {
            Some(path) => InteractionMode::Assign(path),
            _ => InteractionMode::Normal,
        }
    } else if modifiers.ctrl {
        match selected_path {
            Some(ref path) if matches!(editor.doc.node(path), Some(Id::Uuid(_))) => {
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
                render_root_insertion(ui, editor, 0, true);
            } else {
                let root_count = editor.doc.roots.len();
                for i in 0..root_count {
                    render_root_insertion(ui, editor, i, false);
                    if let Some(Some(d)) = d_trees.get(i) {
                        let path = Path::new(&editor.doc.roots[i]);
                        let push_id = egui::Id::new(&editor.doc.roots[i]);
                        ui.push_id(push_id, |ui| {
                            let ctx = DContext { path: path.clone() };
                            render_d(ui, editor, d, &mode, &ctx);
                        });
                    }
                }
                render_root_insertion(ui, editor, root_count, false);
            }
        });

        if !orphan_ids.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("orphans").color(Color32::from_gray(100)).italics().size(11.0));
            ui.add_space(4.0);
            let mut sorted_orphans: Vec<_> = orphan_ids.iter().collect();
            sorted_orphans.sort();
            for orphan_id in sorted_orphans {
                if let Id::Uuid(uuid) = orphan_id {
                    identicon(ui, 18.0, uuid);
                }
                ui.add_space(2.0);
            }
        }

        if bg_response.clicked() {
            editor.selection = None;
        }
        });
    });
}

fn render_root_insertion(ui: &mut Ui, editor: &mut Editor, index: usize, empty_doc: bool) {
    let active_placeholder = matches!(
        &editor.selection,
        Some(Selection::InsertRoot(idx, _)) if *idx == index
    );

    if active_placeholder {
        if let Some(Selection::InsertRoot(_, ps)) = &editor.selection {
            let mut ps = ps.clone();
            match super::placeholder::render(ui, &mut ps) {
                PlaceholderResult::Commit(id) => {
                    editor.doc.roots.insert(index, RootSlot::new(id));
                    editor.selection = None;
                }
                PlaceholderResult::Dismiss => editor.selection = None,
                PlaceholderResult::Active => {
                    if let Some(Selection::InsertRoot(_, ref mut wps)) = editor.selection {
                        *wps = ps;
                    }
                }
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
            editor.selection = Some(Selection::insert_root(index));
        }
    } else if insertion_point(ui).clicked() {
        editor.selection = Some(Selection::insert_root(index));
    }
}
