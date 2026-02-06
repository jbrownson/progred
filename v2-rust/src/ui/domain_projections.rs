use crate::document::{Editor, EditorWriter};
use crate::generated::semantics::*;
use crate::graph::Id;
use eframe::egui::{self, Color32, Ui};

pub fn try_domain_projection(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    id: &Id,
    descend: &mut dyn FnMut(&mut Ui, &mut EditorWriter, &Id),
) -> bool {
    if Field::try_wrap(&editor.doc.gid, id).is_some() {
        project_field_compact(ui, editor, w, id, descend);
        true
    } else {
        false
    }
}

fn project_field_compact(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    node: &Id,
    descend: &mut dyn FnMut(&mut Ui, &mut EditorWriter, &Id),
) {
    let field = Field::wrap(node.clone());
    let name = field.name(&editor.doc.gid).unwrap_or_else(|| "?".into());

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("field").color(Color32::from_rgb(150, 100, 150)).italics());
        ui.label(egui::RichText::new(format!("\"{}\"", name)).color(Color32::from_gray(60)));

        if field.type_(&editor.doc.gid).is_some() {
            ui.label(egui::RichText::new(":").color(Color32::from_gray(120)));
            descend(ui, w, &TYPE_);
        }
    });
}
