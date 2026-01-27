use crate::graph::{Gid, Id, Path, Selection, SpanningTree};
use eframe::egui::{pos2, Color32, Response, Rounding, Sense, Ui, Vec2};
use im::HashSet;

use super::identicon;

fn selectable_widget(
    ui: &mut Ui,
    selected: bool,
    shift_target: bool,
    add_contents: impl FnOnce(&mut Ui) -> Response,
) -> Response {
    let id = ui.next_auto_id();

    let where_to_put_background = ui.painter().add(eframe::epaint::Shape::Noop);
    let where_to_put_border = ui.painter().add(eframe::epaint::Shape::Noop);

    let inner_response = add_contents(ui);
    let rect = inner_response.rect.expand(2.0);
    let response = ui.interact(rect, id, Sense::click());

    let rounding = Rounding::same(3.0);

    let (bg, border) = if selected {
        let color = Color32::from_rgb(59, 130, 246);
        (Some(color.gamma_multiply(0.3)), Some(eframe::epaint::Stroke::new(1.5, color)))
    } else if shift_target {
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
        ui.painter().set(where_to_put_border, eframe::epaint::Shape::rect_stroke(rect, rounding, border));
    }

    response
}

pub fn insertion_point(ui: &mut Ui, selected: bool) -> Response {
    let width = ui.available_width().min(200.0);

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 0.0), Sense::hover());

    let hit_rect = eframe::egui::Rect::from_center_size(rect.center(), Vec2::new(width, 10.0));
    let response = ui.interact(hit_rect, ui.next_auto_id(), Sense::click());

    if selected || response.hovered() {
        let color = if selected {
            Color32::from_rgb(59, 130, 246)
        } else {
            Color32::from_gray(150)
        };

        let caret_size = 10.0;
        let center_y = rect.center().y;
        let left_x = rect.min.x - 5.0;

        ui.painter().add(eframe::epaint::Shape::convex_polygon(
            vec![
                pos2(left_x, center_y - caret_size * 0.4),
                pos2(left_x + caret_size * 0.6, center_y),
                pos2(left_x, center_y + caret_size * 0.4),
            ],
            color,
            eframe::epaint::Stroke::NONE,
        ));
    }

    response
}

fn render_label(ui: &mut Ui, id: &Id, shift_target: bool) -> Response {
    let label_color = Color32::from_gray(120);
    selectable_widget(ui, false, shift_target, |ui| match id {
        Id::Uuid(uuid) => identicon(ui, 12.0, uuid),
        Id::String(s) => ui.label(eframe::egui::RichText::new(s.to_string()).color(label_color).italics()),
        Id::Number(n) => ui.label(eframe::egui::RichText::new(n.to_string()).color(label_color).italics()),
    })
}

fn label_arrow(ui: &mut Ui) {
    let width = 12.0;
    let height = 10.0;
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());

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
            ui.painter().rect_filled(rect.expand(1.0), Rounding::same(2.0), Color32::from_gray(220));
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

pub fn project(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    shift_source: Option<&Path>,
) {
    project_inner(ui, gid, tree, selection, path, HashSet::new(), shift_source)
}

fn project_inner(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    ancestors: HashSet<Id>,
    shift_source: Option<&Path>,
) {
    match path.node(gid) {
        Some(id) => project_id(ui, gid, tree, selection, path, id, ancestors, shift_source),
        None => project_placeholder(ui, selection, path),
    }
}

fn project_placeholder(ui: &mut Ui, selection: &mut Option<Selection>, path: &Path) {
    let selected = matches!(selection, Some(Selection::Edge(p)) if p == path);
    let response = selectable_widget(ui, selected, false, |ui| ui.label("(empty)"));
    if response.clicked() {
        *selection = Some(Selection::Edge(path.clone()));
    }
}

fn project_id(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    id: &Id,
    ancestors: HashSet<Id>,
    shift_source: Option<&Path>,
) {
    match id {
        Id::Uuid(uuid) => project_uuid(ui, gid, tree, selection, path, uuid, ancestors, shift_source),
        Id::String(s) => project_leaf(ui, selection, path, format!("\"{}\"", s)),
        Id::Number(n) => project_leaf(ui, selection, path, n.to_string()),
    }
}

fn project_leaf(ui: &mut Ui, selection: &mut Option<Selection>, path: &Path, text: String) {
    let selected = matches!(selection, Some(Selection::Edge(p)) if p == path);
    let response = selectable_widget(ui, selected, false, |ui| ui.label(text));
    if response.clicked() {
        *selection = Some(Selection::Edge(path.clone()));
    }
}

fn project_uuid(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    uuid: &uuid::Uuid,
    ancestors: HashSet<Id>,
    shift_source: Option<&Path>,
) {
    let id = Id::Uuid(*uuid);
    let edges = gid.edges(&id);
    let new_edge_label = match selection {
        Some(Selection::Edge(sel)) => sel.pop()
            .filter(|(parent, _)| parent == path)
            .map(|(_, label)| label)
            .filter(|label| !edges.map(|e| e.contains_key(label)).unwrap_or(false)),
        _ => None,
    };
    let all_labels: Vec<Id> = new_edge_label.into_iter()
        .chain(edges.into_iter().flat_map(|e| e.keys().cloned()))
        .collect();
    let is_collapsed = tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));
    let selected = matches!(selection, Some(Selection::Edge(p)) if p == path);

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let response = selectable_widget(ui, selected, shift_source.is_some(), |ui| identicon(ui, 18.0, uuid));

            if response.clicked() {
                *selection = Some(Selection::Edge(shift_source.map(|s| s.child(id.clone())).unwrap_or_else(|| path.clone())));
            }

            if !all_labels.is_empty() && collapse_toggle(ui, is_collapsed).clicked() {
                *tree = tree.set_collapsed_at_path(path, !is_collapsed);
            }
        });

        if !is_collapsed && !all_labels.is_empty() {
            let child_ancestors = ancestors.update(id.clone());
            ui.add_space(2.0);
            ui.indent("edges", |ui| {
                for label in &all_labels {
                    let child_path = path.child(label.clone());

                    ui.horizontal(|ui| {
                        let label_response = render_label(ui, label, shift_source.is_some());

                        if let Some(source) = shift_source {
                            if label_response.clicked() {
                                *selection = Some(Selection::Edge(source.child(label.clone())));
                            }
                        }

                        label_arrow(ui);
                        project_inner(ui, gid, tree, selection, &child_path, child_ancestors.clone(), shift_source);
                    });
                    ui.add_space(2.0);
                }
            });
        }
    });
}
