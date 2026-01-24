use crate::graph::{Gid, Id, Path, SpanningTree};
use eframe::egui::{pos2, Color32, Response, Sense, Ui, Vec2};
use im::HashSet;

use super::identicon;

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
            // Right-pointing triangle ▸
            vec![
                pos2(center.x - half * 0.5, center.y - half),
                pos2(center.x - half * 0.5, center.y + half),
                pos2(center.x + half, center.y),
            ]
        } else {
            // Down-pointing triangle ▾
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

pub fn project(ui: &mut Ui, gid: &impl Gid, tree: &mut SpanningTree, path: &Path) -> Response {
    project_inner(ui, gid, tree, path, HashSet::new())
}

fn project_inner(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    path: &Path,
    ancestors: HashSet<Id>,
) -> Response {
    let node = path.node(gid);

    match node {
        Some(id) => project_id(ui, gid, tree, path, id, ancestors),
        None => ui.label("(invalid path)"),
    }
}

fn project_id(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    path: &Path,
    id: &Id,
    ancestors: HashSet<Id>,
) -> Response {
    match id {
        Id::Uuid(uuid) => project_uuid(ui, gid, tree, path, uuid, ancestors),
        Id::String(s) => ui.label(format!("\"{}\"", s)),
        Id::Number(n) => ui.label(format!("{}", n)),
    }
}

fn project_uuid(
    ui: &mut Ui,
    gid: &impl Gid,
    tree: &mut SpanningTree,
    path: &Path,
    uuid: &uuid::Uuid,
    ancestors: HashSet<Id>,
) -> Response {
    let id = Id::Uuid(*uuid);
    let edges = gid.edges(&id);
    let has_children = edges.map(|e| !e.is_empty()).unwrap_or(false);
    let is_collapsed = tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            identicon(ui, 16.0, uuid);
            
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
                            project_inner(ui, gid, tree, &child_path, child_ancestors.clone());
                        });
                    }
                });
            }
        }
    })
    .response
}
