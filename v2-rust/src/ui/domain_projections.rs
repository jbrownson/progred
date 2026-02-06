// TODO: Domain projections should receive a `descend` callback instead of rendering
// children directly. Currently bypasses path tracking, cycle detection, selection state,
// and editing. See prototype's R.ts descend() pattern. The unused parameters (_w, _path,
// _ancestors, _mode) should all be used once this is properly integrated.

use crate::document::{Editor, EditorWriter};
use crate::generated::semantics::*;
use crate::graph::{Id, Path};
use eframe::egui::{self, Color32, Ui};
use im::HashSet;

use super::projection::InteractionMode;

fn type_id(s: &str) -> Id {
    Id::Uuid(uuid::Uuid::parse_str(s).unwrap())
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
    if editor.isa_of(id) == Some(&type_id(Field::TYPE_ID)) {
        project_field_compact(ui, editor, id);
        true
    } else {
        false
    }
}

/// Project a field compactly as "name: type" on one line
fn project_field_compact(ui: &mut Ui, editor: &Editor, node: &Id) {
    let field = Field::wrap(node.clone());
    let name = field.name(&editor.doc.gid).unwrap_or_else(|| "?".into());

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("field").color(Color32::from_rgb(150, 100, 150)).italics());
        ui.label(egui::RichText::new(format!("\"{}\"", name)).color(Color32::from_gray(60)));

        if let Some(type_expr) = field.type_(&editor.doc.gid) {
            ui.label(egui::RichText::new(":").color(Color32::from_gray(120)));
            render_type_ref(ui, editor, type_expr.id());
        }
    });
}

/// Render a type as a reference (just the name, or base<args> for apply types)
fn render_type_ref(ui: &mut Ui, editor: &Editor, node: &Id) {
    if editor.isa_of(node) == Some(&type_id(Apply::TYPE_ID)) {
        let apply = Apply::wrap(node.clone());

        let base_name = apply.base(&editor.doc.gid)
            .and_then(|b| editor.name_of(b.id()))
            .unwrap_or_else(|| "?".into());

        ui.label(egui::RichText::new(&base_name).color(Color32::from_gray(80)).italics());
        ui.label(egui::RichText::new("<").color(Color32::from_gray(120)));

        if let Some(args_list) = apply.args(&editor.doc.gid) {
            let args: Vec<_> = args_list.iter(&editor.doc.gid).collect();
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    ui.label(egui::RichText::new(", ").color(Color32::from_gray(120)));
                }
                let arg_name = editor.name_of(arg.id()).unwrap_or_else(|| "?".into());
                ui.label(egui::RichText::new(&arg_name).color(Color32::from_gray(80)));
            }
        }

        ui.label(egui::RichText::new(">").color(Color32::from_gray(120)));
    } else {
        let type_name = editor.name_of(node).unwrap_or_else(|| "?".into());
        ui.label(egui::RichText::new(&type_name).color(Color32::from_gray(80)).italics());
    }
}
