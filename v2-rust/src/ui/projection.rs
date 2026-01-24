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
    
    let inner_response = add_contents(ui);
    let rect = inner_response.rect.expand(2.0);
    let response = ui.interact(rect, id, Sense::click());
    
    let visuals = ui.visuals();
    let bg_color = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    
    if bg_color != Color32::TRANSPARENT {
        ui.painter().set(
            where_to_put_background,
            eframe::epaint::Shape::rect_filled(rect, Rounding::same(2.0), bg_color),
        );
    }
    
    response
}

fn render_label(ui: &mut Ui, id: &Id) {
    match id {
        Id::Uuid(uuid) => { identicon(ui, 12.0, uuid); }
        Id::String(s) => { ui.label(format!("\"{}\"", s)); }
        Id::Number(n) => { ui.label(format!("{}", n)); }
    }
}

fn collapse_toggle(ui: &mut Ui, collapsed: bool) -> Response {
    let size = 12.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());

    if ui.is_rect_visible(rect) {
        let color = Color32::GRAY;
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
                identicon(ui, 16.0, uuid)
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
                ui.indent(uuid, |ui| {
                    for (label, _value) in edges.iter() {
                        let child_path = path.child(label.clone());
                        ui.horizontal(|ui| {
                            render_label(ui, label);
                            ui.label(":");
                            project_inner(ui, gid, tree, selection, &child_path, child_ancestors.clone());
                        });
                    }
                });
            }
        }
    })
    .response
}
