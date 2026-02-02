use crate::document::{Editor, EditorWriter};
use crate::generated::semantics::*;
use crate::graph::{Gid, Id, Path};
use eframe::egui::{self, Color32, Ui};
use im::HashSet;

use super::projection::InteractionMode;

fn id(s: &str) -> Id {
    Id::Uuid(uuid::Uuid::parse_str(s).unwrap())
}

fn isa_is(editor: &Editor, node: &Id, type_id: &str) -> bool {
    editor.isa_of(node) == Some(&id(type_id))
}

fn get_edge<'a>(editor: &'a Editor, node: &Id, field_id: &str) -> Option<&'a Id> {
    editor.doc.gid.get(node, &id(field_id))
}

/// Try domain-specific projection - only handles field type annotations specially
pub fn try_domain_projection(
    ui: &mut Ui,
    editor: &Editor,
    _w: &mut EditorWriter,
    _path: &Path,
    id: &Id,
    _ancestors: HashSet<Id>,
    _mode: &InteractionMode,
) -> bool {
    if isa_is(editor, id, Field::TYPE_ID) {
        project_field_compact(ui, editor, id);
        true
    } else {
        false
    }
}

/// Project a field compactly as "name: type" on one line
fn project_field_compact(ui: &mut Ui, editor: &Editor, node: &Id) {
    let name = editor.name_of(node).unwrap_or_else(|| "?".into());

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("field").color(Color32::from_rgb(150, 100, 150)).italics());
        ui.label(egui::RichText::new(format!("\"{}\"", name)).color(Color32::from_gray(60)));

        if let Some(type_id) = get_edge(editor, node, Field::TYPE_) {
            ui.label(egui::RichText::new(":").color(Color32::from_gray(120)));
            render_type_ref(ui, editor, type_id);
        }
    });
}

/// Render a type as a reference (just the name, or base<args> for apply types)
fn render_type_ref(ui: &mut Ui, editor: &Editor, type_id: &Id) {
    if isa_is(editor, type_id, Apply::TYPE_ID) {
        // Show as base<args>
        let base_name = get_edge(editor, type_id, Field::BASE)
            .and_then(|b| editor.name_of(b))
            .unwrap_or_else(|| "?".into());

        ui.label(egui::RichText::new(&base_name).color(Color32::from_gray(80)).italics());
        ui.label(egui::RichText::new("<").color(Color32::from_gray(120)));

        if let Some(args_list) = get_edge(editor, type_id, Field::ARGS) {
            let args = flatten_list(editor, args_list);
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    ui.label(egui::RichText::new(", ").color(Color32::from_gray(120)));
                }
                let arg_name = editor.name_of(arg).unwrap_or_else(|| "?".into());
                ui.label(egui::RichText::new(&arg_name).color(Color32::from_gray(80)));
            }
        }

        ui.label(egui::RichText::new(">").color(Color32::from_gray(120)));
    } else {
        // Just show the name
        let type_name = editor.name_of(type_id).unwrap_or_else(|| "?".into());
        ui.label(egui::RichText::new(&type_name).color(Color32::from_gray(80)).italics());
    }
}

/// Flatten a cons/empty list into a Vec
fn flatten_list(editor: &Editor, list_node: &Id) -> Vec<Id> {
    let mut result = Vec::new();
    let mut current = list_node.clone();
    let mut seen = std::collections::HashSet::new();

    while editor.is_cons(&current) && seen.insert(current.clone()) {
        if let Some(head) = get_edge(editor, &current, Field::HEAD) {
            result.push(head.clone());
        }
        match get_edge(editor, &current, Field::TAIL) {
            Some(tail) => current = tail.clone(),
            None => break,
        }
    }

    result
}
