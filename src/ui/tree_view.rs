use progred_core::d::{D, DEvent};
use progred_core::editor::{Editor, InteractionMode};
use progred_core::graph::Id;
use progred_core::path::Path;
use progred_core::selection::Selection;
use eframe::egui::{self, Color32, RichText, Sense, Ui};
use std::collections::HashSet;

use super::identicon;
use super::layout::TREE_MARGIN;
use super::placeholder::PlaceholderOutcome;
use super::{insertion_point, render_d, DContext};

pub fn generate(editor: &Editor) -> Vec<Option<D>> {
    editor.doc.roots.iter()
        .map(|root_slot| {
            let path = Path::new(root_slot);
            let id = editor.doc.node(&path)?;
            Some(progred_core::render::render(editor, &path, &id))
        })
        .collect()
}

pub fn render<'a>(ui: &mut Ui, editor: &Editor, d_trees: &'a [Option<D>], orphan_ids: &HashSet<Id>, mode: &InteractionMode, events: &mut Vec<DEvent<'a>>) {
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
                render_root_insertion(ui, editor, 0, true, events);
            } else {
                let root_count = editor.doc.roots.len();
                for i in 0..root_count {
                    render_root_insertion(ui, editor, i, false, events);
                    if let Some(Some(d)) = d_trees.get(i) {
                        let path = Path::new(&editor.doc.roots[i]);
                        let push_id = egui::Id::new(&editor.doc.roots[i]);
                        ui.push_id(push_id, |ui| {
                            let ctx = DContext { path: path.clone(), selection: Selection::edge(path) };
                            render_d(ui, editor, d, mode, &ctx, events);
                        });
                    }
                }
                render_root_insertion(ui, editor, root_count, false, events);
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
            events.push(DEvent::ClickedBackground);
        }
        });
    });
}

fn render_root_insertion(ui: &mut Ui, editor: &Editor, index: usize, empty_doc: bool, events: &mut Vec<DEvent<'_>>) {
    let active_placeholder = matches!(
        &editor.selection,
        Some(Selection::InsertRoot(idx, _)) if *idx == index
    );

    if active_placeholder {
        if let Some(Selection::InsertRoot(_, ps)) = &editor.selection {
            let result = super::placeholder::render(ui, editor, ps);
            match result.outcome {
                PlaceholderOutcome::Commit(value) => {
                    events.push(DEvent::RootPlaceholderCommitted { index, value });
                }
                PlaceholderOutcome::Dismiss => {
                    events.push(DEvent::RootPlaceholderDismissed);
                }
                PlaceholderOutcome::Active => {
                    if let Some(text) = result.text_changed {
                        events.push(DEvent::RootPlaceholderTextChanged(text));
                    }
                    if let Some(idx) = result.selection_moved {
                        events.push(DEvent::RootPlaceholderSelectionMoved(idx));
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
            events.push(DEvent::ClickedRootInsertionPoint(index));
        }
    } else if insertion_point(ui).clicked() {
        events.push(DEvent::ClickedRootInsertionPoint(index));
    }
}
