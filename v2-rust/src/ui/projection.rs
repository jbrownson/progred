use crate::document::{Editor, EditorWriter};
use crate::graph::{Gid, Id, Path, Selection};
use eframe::egui::{pos2, Color32, CornerRadius, Response, Sense, Ui, Vec2};
use im::HashSet;

use super::identicon;
use super::placeholder::PlaceholderResult;

pub enum InteractionMode {
    Normal,
    SelectUnder(Path),
    Assign(Path),
}

fn selectable_widget(
    ui: &mut Ui,
    selected: bool,
    secondary_selected: bool,
    select_under: bool,
    assign: bool,
    add_contents: impl FnOnce(&mut Ui) -> Response,
) -> Response {
    let id = ui.next_auto_id();

    let where_to_put_background = ui.painter().add(eframe::epaint::Shape::Noop);
    let where_to_put_border = ui.painter().add(eframe::epaint::Shape::Noop);

    let rect = add_contents(ui).rect.expand(2.0);
    let response = ui.interact(rect, id, Sense::click());

    let rounding = CornerRadius::same(3);

    let (bg, border) = if selected {
        let color = Color32::from_rgb(59, 130, 246);
        (Some(color.gamma_multiply(0.3)), Some(eframe::epaint::Stroke::new(1.5, color)))
    } else if secondary_selected {
        let color = Color32::from_rgb(59, 130, 246);
        (Some(color.gamma_multiply(0.15)), Some(eframe::epaint::Stroke::new(1.0, color.gamma_multiply(0.4))))
    } else if assign {
        let color = Color32::from_rgb(234, 179, 8);
        let intensity = if response.hovered() { 0.4 } else { 0.2 };
        (Some(color.gamma_multiply(intensity)), Some(eframe::epaint::Stroke::new(1.0, color.gamma_multiply(0.6))))
    } else if select_under {
        let color = Color32::from_rgb(34, 197, 94);
        let intensity = if response.hovered() { 0.4 } else { 0.2 };
        (Some(color.gamma_multiply(intensity)), Some(eframe::epaint::Stroke::new(1.0, color.gamma_multiply(0.6))))
    } else if response.hovered() {
        (Some(Color32::from_gray(200).gamma_multiply(0.5)), None)
    } else {
        (None, None)
    };

    if let Some(bg) = bg {
        ui.painter().set(where_to_put_background, eframe::epaint::Shape::rect_filled(rect, rounding, bg));
    }
    if let Some(border) = border {
        ui.painter().set(where_to_put_border, eframe::epaint::Shape::rect_stroke(rect, rounding, border, eframe::epaint::StrokeKind::Middle));
    }

    response
}

pub fn insertion_point(ui: &mut Ui) -> Response {
    let width = ui.available_width().min(200.0);

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 0.0), Sense::hover());

    let hit_rect = eframe::egui::Rect::from_center_size(rect.center(), Vec2::new(width, 10.0));
    let response = ui.interact(hit_rect, ui.next_auto_id(), Sense::click());

    if response.hovered() {
        let caret_size = 10.0;
        let center_y = rect.center().y;
        let left_x = rect.min.x - 5.0;

        ui.painter().add(eframe::epaint::Shape::convex_polygon(
            vec![
                pos2(left_x, center_y - caret_size * 0.4),
                pos2(left_x + caret_size * 0.6, center_y),
                pos2(left_x, center_y + caret_size * 0.4),
            ],
            Color32::from_gray(150),
            eframe::epaint::Stroke::NONE,
        ));
    }

    response
}

fn render_label(ui: &mut Ui, id: &Id, secondary_selected: bool, select_under: bool, assign: bool) -> Response {
    let label_color = Color32::from_gray(120);
    selectable_widget(ui, false, secondary_selected, select_under, assign, |ui| match id {
        Id::Uuid(uuid) => identicon(ui, 12.0, uuid),
        Id::String(s) => ui.label(eframe::egui::RichText::new(s.to_string()).color(label_color).italics()),
        Id::Number(n) => ui.label(eframe::egui::RichText::new(n.to_string()).color(label_color).italics()),
    })
}

fn label_arrow(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 10.0), Sense::hover());

    if ui.is_rect_visible(rect) {
        let color = Color32::from_gray(160);
        let stroke = eframe::epaint::Stroke::new(1.0, color);
        let center_y = rect.center().y;
        let left = rect.min.x + 1.0;
        let right = rect.max.x - 2.0;

        ui.painter().line_segment([pos2(left, center_y), pos2(right, center_y)], stroke);

        let arrow_size = 3.0;
        ui.painter().line_segment([pos2(right - arrow_size, center_y - arrow_size), pos2(right, center_y)], stroke);
        ui.painter().line_segment([pos2(right - arrow_size, center_y + arrow_size), pos2(right, center_y)], stroke);
    }
}

fn collapse_toggle(ui: &mut Ui, collapsed: bool) -> Response {
    let size = 12.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());

    if ui.is_rect_visible(rect) {
        let color = if response.hovered() { Color32::from_gray(80) } else { Color32::from_gray(150) };

        if response.hovered() {
            ui.painter().rect_filled(rect.expand(1.0), CornerRadius::same(2), Color32::from_gray(220));
        }
        let center = rect.center();
        let half = size * 0.3;

        let points = if collapsed {
            vec![
                pos2(center.x - half * 0.5, center.y - half),
                pos2(center.x - half * 0.5, center.y + half),
                pos2(center.x + half, center.y),
            ]
        } else {
            vec![
                pos2(center.x - half, center.y - half * 0.5),
                pos2(center.x + half, center.y - half * 0.5),
                pos2(center.x, center.y + half),
            ]
        };

        ui.painter().add(eframe::epaint::Shape::convex_polygon(points, color, eframe::epaint::Stroke::NONE));
    }

    response
}

pub fn project(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, path: &Path, mode: &InteractionMode) {
    if let Some(id) = path.node(&editor.doc.gid).cloned() {
        project_id(ui, editor, w, path, &id, HashSet::new(), mode);
    }
}

fn project_id(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    id: &Id,
    ancestors: HashSet<Id>,
    mode: &InteractionMode,
) {
    match id {
        Id::Uuid(uuid) => project_uuid(ui, editor, w, path, uuid, ancestors, mode),
        Id::String(s) => project_leaf(ui, editor, w, path, id, format!("\"{}\"", s)),
        Id::Number(n) => project_leaf(ui, editor, w, path, id, n.to_string()),
    }
}

fn project_leaf(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, path: &Path, id: &Id, text: String) {
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary
        && editor.selection.as_ref().and_then(|s| s.selected_node_id(&editor.doc.gid)) == Some(id);
    if selectable_widget(ui, primary, secondary, false, false, |ui| ui.label(text)).clicked() {
        w.select(Some(Selection::edge(path.clone())));
    }
}

fn handle_pick(w: &mut EditorWriter, mode: &InteractionMode, value: Id, path: &Path) {
    match mode {
        InteractionMode::Assign(target) => {
            w.set_edge(target, value);
            w.select(None);
        }
        InteractionMode::SelectUnder(source) => {
            w.select(Some(Selection::edge(source.child(value))));
        }
        InteractionMode::Normal => {
            w.select(Some(Selection::edge(path.clone())));
        }
    }
}

fn project_uuid(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    uuid: &uuid::Uuid,
    ancestors: HashSet<Id>,
    mode: &InteractionMode,
) {
    let id = Id::Uuid(*uuid);
    let edges = editor.doc.gid.edges(&id);
    let new_edge_label = editor.selection.as_ref()
        .and_then(|s| s.edge_path())
        .and_then(|sel| sel.pop())
        .filter(|(parent, _)| parent == path)
        .map(|(_, label)| label)
        .filter(|label| !edges.map(|e| e.contains_key(label)).unwrap_or(false));
    let all_edges: Vec<(Id, Id)> = edges.into_iter()
        .flat_map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())))
        .collect();
    let has_content = !all_edges.is_empty() || new_edge_label.is_some();
    let is_collapsed = editor.tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));
    let select_under = matches!(mode, InteractionMode::SelectUnder(_));
    let assign = matches!(mode, InteractionMode::Assign(_));
    let selected_node = editor.selection.as_ref().and_then(|s| s.selected_node_id(&editor.doc.gid));
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && selected_node == Some(&id);

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            if selectable_widget(
                ui, primary, secondary,
                select_under, assign,
                |ui| identicon(ui, 18.0, uuid),
            ).clicked() {
                handle_pick(w, mode, Id::Uuid(*uuid), path);
            }

            if has_content && collapse_toggle(ui, is_collapsed).clicked() {
                w.set_collapsed(path, !is_collapsed);
            }
        });

        if !is_collapsed && has_content {
            let child_ancestors = ancestors.update(id.clone());
            ui.add_space(2.0);
            ui.indent("edges", |ui| {
                for (label, value) in &all_edges {
                    let label_secondary = selected_node == Some(label);
                    ui.horizontal(|ui| {
                        if render_label(ui, label, label_secondary, select_under, assign).clicked()
                            && !matches!(mode, InteractionMode::Normal)
                        {
                            handle_pick(w, mode, label.clone(), path);
                        }

                        label_arrow(ui);
                        project_id(ui, editor, w, &path.child(label.clone()), value, child_ancestors.clone(), mode);
                    });
                    ui.add_space(2.0);
                }
                if let Some(ref new_label) = new_edge_label {
                    ui.horizontal(|ui| {
                        render_label(ui, new_label, false, false, false);
                        label_arrow(ui);
                        if let Some(ps) = w.placeholder_state() {
                            match super::placeholder::render(ui, ps) {
                                PlaceholderResult::Commit(value) => {
                                    w.set_edge(&path.child(new_label.clone()), value);
                                    w.select(None);
                                }
                                PlaceholderResult::Dismiss => w.select(None),
                                PlaceholderResult::Active => {}
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
            });
        }
    });
}
