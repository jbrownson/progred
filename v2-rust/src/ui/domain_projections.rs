use crate::document::{Editor, EditorWriter};
use crate::generated::semantics::*;
use crate::graph::{Gid, Id};
use crate::list_iter::ListIter;
use eframe::egui::{self, Color32, Ui};

const TYPE_REF_COLOR: Color32 = Color32::from_rgb(80, 130, 180);
const KEYWORD_COLOR: Color32 = Color32::from_rgb(150, 100, 150);
const PUNCT_COLOR: Color32 = Color32::from_gray(120);

pub fn try_domain_projection(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    id: &Id,
    descend: &mut dyn FnMut(&mut Ui, &mut EditorWriter, &Id),
) -> bool {
    let gid = &editor.doc.gid;
    if Field::try_wrap(gid, id).is_some() {
        project_field(ui, editor, w, id, descend);
        true
    } else if Apply::try_wrap(gid, id).is_some() {
        project_apply(ui, editor, id);
        true
    } else {
        false
    }
}

fn project_field(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    node: &Id,
    descend: &mut dyn FnMut(&mut Ui, &mut EditorWriter, &Id),
) {
    let field = Field::wrap(node.clone());
    let name = field.name(&editor.doc.gid).unwrap_or_else(|| "?".into());

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("field").color(KEYWORD_COLOR).italics());
        ui.label(egui::RichText::new(format!("\"{}\"", name)).color(Color32::from_gray(60)));

        if field.type_(&editor.doc.gid).is_some() {
            ui.label(egui::RichText::new(":").color(PUNCT_COLOR));
            descend(ui, w, &TYPE_);
        }
    });
}

fn project_apply(ui: &mut Ui, editor: &Editor, node: &Id) {
    let apply = Apply::wrap(node.clone());
    let gid = &editor.doc.gid;

    ui.horizontal(|ui| {
        let base_name = apply.base(gid)
            .and_then(|b| editor.name_of(b.id()))
            .unwrap_or_else(|| "?".into());
        ui.label(egui::RichText::new(&base_name).color(TYPE_REF_COLOR));

        if let Some(args_id) = gid.get(node, &ARGS) {
            ui.label(egui::RichText::new("<").color(PUNCT_COLOR));
            for (i, arg_id) in ListIter::new(gid, Some(args_id)).enumerate() {
                if i > 0 {
                    ui.label(egui::RichText::new(", ").color(PUNCT_COLOR));
                }
                render_type_inline(ui, editor, arg_id);
            }
            ui.label(egui::RichText::new(">").color(PUNCT_COLOR));
        }
    });
}

fn render_type_inline(ui: &mut Ui, editor: &Editor, node: &Id) {
    let gid = &editor.doc.gid;
    if let Some(apply) = Apply::try_wrap(gid, node) {
        let base_name = apply.base(gid)
            .and_then(|b| editor.name_of(b.id()))
            .unwrap_or_else(|| "?".into());
        ui.label(egui::RichText::new(&base_name).color(TYPE_REF_COLOR));

        if let Some(args_id) = gid.get(node, &ARGS) {
            ui.label(egui::RichText::new("<").color(PUNCT_COLOR));
            for (i, arg_id) in ListIter::new(gid, Some(args_id)).enumerate() {
                if i > 0 {
                    ui.label(egui::RichText::new(", ").color(PUNCT_COLOR));
                }
                render_type_inline(ui, editor, arg_id);
            }
            ui.label(egui::RichText::new(">").color(PUNCT_COLOR));
        }
    } else {
        let name = editor.name_of(node).unwrap_or_else(|| "?".into());
        ui.label(egui::RichText::new(&name).color(TYPE_REF_COLOR));
    }
}
