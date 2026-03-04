use progred_core::d::{D, DEvent};
use progred_core::editor::{Editor, InteractionMode};
use progred_core::graph::Id;
use progred_core::path::Path;
use progred_core::selection::Selection;
use eframe::egui::{self, Color32, RichText, Sense, Ui};
use std::collections::HashSet;

use super::identicon;
use super::layout::TREE_MARGIN;
use super::{render_d, DContext};

pub fn generate(editor: &Editor) -> D {
    let path = Path::root();
    match editor.doc.node(&path) {
        Some(id) => progred_core::render::render(editor, &path, &id),
        None => {
            let commit_path = path.clone();
            D::Descend {
                path: path.clone(),
                selection: Selection::edge(path),
                child: Box::new(D::Placeholder {
                    on_commit: Box::new(move |w: &mut Editor, value| {
                        w.doc.set_edge(&commit_path, value);
                    }),
                }),
            }
        }
    }
}

pub fn render<'a>(ui: &mut Ui, editor: &Editor, d_tree: &'a D, orphan_ids: &HashSet<Id>, mode: &InteractionMode, events: &mut Vec<DEvent<'a>>) {
    let margin = egui::Margin::same(TREE_MARGIN as i8);
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
        let bg_response = ui.interact(
            ui.clip_rect(),
            ui.id().with("background"),
            Sense::click(),
        );

        let path = Path::root();
        let ctx = DContext { path: path.clone(), selection: Selection::edge(path) };
        render_d(ui, editor, d_tree, mode, &ctx, events);

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
