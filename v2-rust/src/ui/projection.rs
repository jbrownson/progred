use crate::graph::{Gid, Id, Path, Selection, SpanningTree};
use eframe::egui::{pos2, Color32, Response, Rounding, Sense, Ui, Vec2};
use im::HashSet;

use super::identicon;

fn is_selected(selection: &Option<Selection>, path: &Path) -> bool {
    match selection {
        Some(Selection::Edge(p)) => p == path,
        _ => false,
    }
}

fn selectable_widget(
    ui: &mut Ui,
    selected: bool,
    add_contents: impl FnOnce(&mut Ui) -> Response,
) -> Response {
    let id = ui.next_auto_id();
    
    let where_to_put_background = ui.painter().add(eframe::epaint::Shape::Noop);
    let where_to_put_border = ui.painter().add(eframe::epaint::Shape::Noop);
    
    let inner_response = add_contents(ui);
    let rect = inner_response.rect.expand(2.0);
    let response = ui.interact(rect, id, Sense::click());
    
    let rounding = Rounding::same(3.0);
    
    if selected {
        ui.painter().set(
            where_to_put_background,
            eframe::epaint::Shape::rect_filled(rect, rounding, Color32::from_rgb(59, 130, 246).gamma_multiply(0.3)),
        );
        ui.painter().set(
            where_to_put_border,
            eframe::epaint::Shape::rect_stroke(rect, rounding, eframe::epaint::Stroke::new(1.5, Color32::from_rgb(59, 130, 246))),
        );
    } else if response.hovered() {
        ui.painter().set(
            where_to_put_background,
            eframe::epaint::Shape::rect_filled(rect, rounding, Color32::from_gray(200).gamma_multiply(0.5)),
        );
    }
    
    response
}

pub fn insertion_point(ui: &mut Ui, selection: &mut Option<Selection>, index: usize) -> Response {
    let selected = matches!(selection, Some(Selection::InsertRoot(i)) if *i == index);
    let width = ui.available_width().min(200.0);
    
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 0.0), Sense::hover());
    
    let hit_rect = eframe::egui::Rect::from_center_size(rect.center(), Vec2::new(width, 10.0));
    let response = ui.interact(hit_rect, ui.id().with(("insertion", index)), Sense::click());
    
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
    
    if response.clicked() {
        *selection = Some(Selection::InsertRoot(index));
    }
    
    response
}

fn render_label(ui: &mut Ui, id: &Id) {
    let label_color = Color32::from_gray(120);
    match id {
        Id::Uuid(uuid) => {
            identicon(ui, 12.0, uuid);
        }
        Id::String(s) => {
            ui.label(
                eframe::egui::RichText::new(format!("{}", s))
                    .color(label_color)
                    .italics()
            );
        }
        Id::Number(n) => {
            ui.label(
                eframe::egui::RichText::new(format!("{}", n))
                    .color(label_color)
                    .italics()
            );
        }
    }
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
        let color = if response.hovered() {
            Color32::from_gray(80)
        } else {
            Color32::from_gray(150)
        };
        
        if response.hovered() {
            ui.painter().rect_filled(
                rect.expand(1.0),
                Rounding::same(2.0),
                Color32::from_gray(220),
            );
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

        ui.painter().add(eframe::epaint::Shape::convex_polygon(
            points,
            color,
            eframe::epaint::Stroke::NONE,
        ));
    }

    response
}

pub fn project(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
) -> Response {
    project_inner(ui, gid, tree, selection, path, HashSet::new())
}

fn project_inner(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    ancestors: HashSet<Id>,
) -> Response {
    let node = path.node(gid);

    match node {
        Some(id) => project_id(ui, gid, tree, selection, path, id, ancestors),
        None => project_placeholder(ui, selection, path),
    }
}

fn project_placeholder(ui: &mut Ui, selection: &mut Option<Selection>, path: &Path) -> Response {
    let selected = is_selected(selection, path);
    let response = selectable_widget(ui, selected, |ui| {
        ui.label("(empty)")
    });
    if response.clicked() {
        *selection = Some(Selection::Edge(path.clone()));
    }
    response
}

fn project_id(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    id: &Id,
    ancestors: HashSet<Id>,
) -> Response {
    match id {
        Id::Uuid(uuid) => project_uuid(ui, gid, tree, selection, path, uuid, ancestors),
        Id::String(s) => project_leaf(ui, selection, path, format!("\"{}\"", s)),
        Id::Number(n) => project_leaf(ui, selection, path, format!("{}", n)),
    }
}

fn project_leaf(ui: &mut Ui, selection: &mut Option<Selection>, path: &Path, text: String) -> Response {
    let selected = is_selected(selection, path);
    let response = selectable_widget(ui, selected, |ui| {
        ui.label(text)
    });
    if response.clicked() {
        *selection = Some(Selection::Edge(path.clone()));
    }
    response
}

fn project_uuid(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    selection: &mut Option<Selection>,
    path: &Path,
    uuid: &uuid::Uuid,
    ancestors: HashSet<Id>,
) -> Response {
    let id = Id::Uuid(*uuid);
    let edges = gid.edges(&id);
    let has_children = edges.map(|e| !e.is_empty()).unwrap_or(false);
    let is_collapsed = tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));
    let selected = is_selected(selection, path);

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let response = selectable_widget(ui, selected, |ui| {
                identicon(ui, 18.0, uuid)
            });
            if response.clicked() {
                *selection = Some(Selection::Edge(path.clone()));
            }

            if has_children {
                if collapse_toggle(ui, is_collapsed).clicked() {
                    *tree = tree.set_collapsed_at_path(path, !is_collapsed);
                }
            }
        });

        if !is_collapsed {
            if let Some(edges) = edges {
                let child_ancestors = ancestors.update(id.clone());
                ui.add_space(2.0);
                ui.indent(uuid, |ui| {
                    for (label, _value) in edges.iter() {
                        let child_path = path.child(label.clone());
                        ui.horizontal(|ui| {
                            render_label(ui, label);
                            label_arrow(ui);
                            project_inner(ui, gid, tree, selection, &child_path, child_ancestors.clone());
                        });
                        ui.add_space(2.0);
                    }
                });
            }
        }
    })
    .response
}
