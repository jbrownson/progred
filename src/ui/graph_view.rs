use crate::document::{Document, Editor, EditorWriter};
use crate::graph::{Gid, Id, MutGid, Selection, SelectionTarget};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, Vec2};
use super::colors;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault};

const NODE_RADIUS: f32 = 20.0;
const REPULSION_K: f32 = 8000.0;
const ATTRACTION_K: f32 = 0.02;
const REST_LENGTH: f32 = 120.0;
const DAMPING: f32 = 0.85;
const MAX_FORCE: f32 = 10.0;
const GRAVITY_K: f32 = 0.005;
const MAX_LABEL_LEN: usize = 8;
const TEXT_FONT_SIZE: f32 = 10.0;
const TEXT_PADDING: f32 = 8.0;

#[derive(Clone)]
pub struct GraphViewState {
    positions: HashMap<Id, Pos2>,
    velocities: HashMap<Id, Vec2>,
    dragging: Option<Id>,
    drag_offset: Vec2,
    pan_offset: Vec2,
    zoom: f32,
    panning: bool,
    pending_drag: Option<(Id, Pos2)>,
    prev_gid: MutGid,
}

impl GraphViewState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            velocities: HashMap::new(),
            dragging: None,
            drag_offset: Vec2::ZERO,
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            panning: false,
            pending_drag: None,
            prev_gid: MutGid::new(),
        }
    }
}

fn deterministic_pos(id: &Id, index: usize) -> Pos2 {
    let hash = BuildHasherDefault::<DefaultHasher>::default().hash_one(id);
    let x = ((hash & 0xFFFF) as f32 / 65535.0 - 0.5) * 300.0;
    let y = (((hash >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5) * 200.0;
    Pos2::new(x + index as f32 * 5.0, y + index as f32 * 5.0)
}

fn collect_all_ids(doc: &Document) -> std::collections::HashSet<Id> {
    doc.roots.iter().map(|r| r.value.clone())
        .chain(doc.gid.entities().map(|u| Id::Uuid(*u)))
        .collect()
}

fn compute_physics_transfers(
    current: &MutGid,
    prev: &MutGid,
    positions: &HashMap<Id, Pos2>,
    stale: &std::collections::HashSet<&Id>,
) -> HashMap<Id, Id> {
    let mut transfers = HashMap::new();
    for &uuid in current.entities() {
        let entity = Id::Uuid(uuid);
        if let Some(edges) = current.edges(&entity) {
            for (label, new_target) in edges.iter() {
                if !positions.contains_key(new_target) {
                    if let Some(old_target) = prev.get(&entity, label) {
                        if stale.contains(old_target) {
                            transfers.insert(new_target.clone(), old_target.clone());
                        }
                    }
                }
            }
        }
    }
    transfers
}

fn sync_positions(state: &mut GraphViewState, doc: &Document) {
    let all_ids = collect_all_ids(doc);
    let stale_ids: std::collections::HashSet<&Id> = state.positions.keys()
        .filter(|id| !all_ids.contains(*id))
        .collect();

    let transfers = compute_physics_transfers(
        &doc.gid, &state.prev_gid, &state.positions, &stale_ids
    );

    for (new_id, old_id) in &transfers {
        if let Some(&pos) = state.positions.get(old_id) {
            state.positions.insert(new_id.clone(), pos);
        }
        if let Some(&vel) = state.velocities.get(old_id) {
            state.velocities.insert(new_id.clone(), vel);
        }
    }

    for (i, id) in all_ids.iter().enumerate() {
        state.positions.entry(id.clone()).or_insert_with(|| deterministic_pos(id, i));
        state.velocities.entry(id.clone()).or_insert(Vec2::ZERO);
    }

    state.positions.retain(|id, _| all_ids.contains(id));
    state.velocities.retain(|id, _| all_ids.contains(id));
    state.prev_gid = doc.gid.clone();
}

struct Edge {
    source: Id,
    label: Id,
    target: Id,
}

fn collect_edges(doc: &Document) -> Vec<Edge> {
    doc.gid.entities()
        .flat_map(|&uuid| {
            let entity = Id::Uuid(uuid);
            doc.gid.edges(&entity).into_iter().flat_map({
                let entity = entity.clone();
                move |edges| {
                    edges.iter().map({
                        let entity = entity.clone();
                        move |(label, value)| Edge {
                            source: entity.clone(),
                            label: label.clone(),
                            target: value.clone(),
                        }
                    })
                }
            })
        })
        .collect()
}

fn compute_forces(positions: &HashMap<Id, Pos2>, edges: &[Edge]) -> HashMap<Id, Vec2> {
    let ids: Vec<Id> = positions.keys().cloned().collect();
    let mut forces: HashMap<Id, Vec2> = ids.iter().map(|id| (id.clone(), Vec2::ZERO)).collect();

    // Repulsion: each node pair pushes apart
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let delta = positions[&ids[i]] - positions[&ids[j]];
            let dist_sq = delta.length_sq().max(1.0);
            let force = delta.normalized() * (REPULSION_K / dist_sq).min(MAX_FORCE);
            *forces.get_mut(&ids[i]).unwrap() += force;
            *forces.get_mut(&ids[j]).unwrap() -= force;
        }
    }

    // Attraction: connected nodes pull together
    for edge in edges {
        if let (Some(&pa), Some(&pb)) = (positions.get(&edge.source), positions.get(&edge.target)) {
            let delta = pb - pa;
            let dist = delta.length().max(0.1);
            let force = delta.normalized() * (ATTRACTION_K * (dist - REST_LENGTH)).clamp(-MAX_FORCE, MAX_FORCE);
            *forces.get_mut(&edge.source).unwrap() += force;
            *forces.get_mut(&edge.target).unwrap() -= force;
        }
    }

    // Gravity: pull toward center
    for id in &ids {
        *forces.get_mut(id).unwrap() += -positions[id].to_vec2() * GRAVITY_K;
    }

    forces
}

fn apply_forces(state: &mut GraphViewState, forces: &HashMap<Id, Vec2>) {
    for (id, force) in forces {
        if state.dragging.as_ref() != Some(id) {
            let vel = state.velocities.get_mut(id).unwrap();
            *vel = (*vel + *force) * DAMPING;
            let pos = state.positions.get_mut(id).unwrap();
            *pos += *vel;
        }
    }
}

fn simulate(state: &mut GraphViewState, edges: &[Edge]) {
    let forces = compute_forces(&state.positions, edges);
    apply_forces(state, &forces);
}

fn node_half_size(half_sizes: &HashMap<Id, Vec2>, id: &Id) -> Vec2 {
    half_sizes.get(id).copied().unwrap_or(Vec2::splat(IDENTICON_HALF_SIZE))
}

fn compute_half_sizes(painter: &egui::Painter, editor: &Editor, ids: impl Iterator<Item = Id>) -> HashMap<Id, Vec2> {
    let font = egui::FontId::proportional(TEXT_FONT_SIZE);
    ids.map(|id| {
        let size = match &id {
            Id::Uuid(_) => match editor.display_label(&id) {
                Some(label) => {
                    let galley = painter.layout_no_wrap(label, font.clone(), Color32::BLACK);
                    (galley.rect.size() + Vec2::splat(TEXT_PADDING)) / 2.0
                }
                None => Vec2::splat(IDENTICON_HALF_SIZE),
            },
            _ => {
                let text = node_display_text(&id).unwrap_or_default();
                let galley = painter.layout_no_wrap(text, font.clone(), Color32::BLACK);
                (galley.rect.size() + Vec2::splat(TEXT_PADDING)) / 2.0
            }
        };
        (id, size)
    }).collect()
}

fn clip_to_rect(center: Pos2, half: Vec2, target: Pos2) -> Pos2 {
    let dir = target - center;
    if dir.x.abs() < 0.001 && dir.y.abs() < 0.001 {
        center + Vec2::new(half.x, 0.0)
    } else {
        let sx = if dir.x.abs() > 0.001 { half.x / dir.x.abs() } else { f32::MAX };
        let sy = if dir.y.abs() > 0.001 { half.y / dir.y.abs() } else { f32::MAX };
        center + dir * sx.min(sy)
    }
}

fn clip_to_rect_toward(center: Pos2, half: Vec2, control: Pos2, fallback: Pos2) -> Pos2 {
    let dir = control - center;
    if dir.length_sq() > 1.0 {
        clip_to_rect(center, half, control)
    } else {
        clip_to_rect(center, half, fallback)
    }
}

fn draw_arrowhead(painter: &egui::Painter, tip: Pos2, dir: Vec2, stroke: Stroke, zoom: f32) {
    let perp = Vec2::new(-dir.y, dir.x);
    let size = 6.0 * zoom;
    let width = 3.0 * zoom;
    painter.add(egui::Shape::line(
        vec![tip - dir * size + perp * width, tip, tip - dir * size - perp * width],
        stroke,
    ));
}

fn truncate_label(s: &str) -> String {
    if s.chars().count() <= MAX_LABEL_LEN {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(MAX_LABEL_LEN).collect::<String>())
    }
}

fn node_display_text(id: &Id) -> Option<String> {
    match id {
        Id::Uuid(_) => None,
        Id::String(s) => Some(format!("\"{}\"", truncate_label(s))),
        Id::Number(n) => Some(truncate_label(&n.to_string())),
    }
}

fn canonical_pair(a: &Id, b: &Id) -> (Id, Id) {
    if a <= b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) }
}

const EDGE_LABEL_IDENTICON_SIZE: f32 = 18.0;
const EDGE_LABEL_PADDING: f32 = 4.0;
const IDENTICON_HALF_SIZE: f32 = NODE_RADIUS * 0.7 + 2.0;

fn edge_label_text(editor: &Editor, label: &Id) -> Option<String> {
    match label {
        Id::Uuid(_) => editor.name_of(label),
        _ => node_display_text(label),
    }
}

fn edge_label_size(painter: &egui::Painter, editor: &Editor, label: &Id, zoom: f32) -> Vec2 {
    match edge_label_text(editor, label) {
        Some(text) => {
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(TEXT_FONT_SIZE * zoom), Color32::BLACK);
            galley.rect.size() + Vec2::splat(EDGE_LABEL_PADDING * zoom)
        }
        None => Vec2::splat((EDGE_LABEL_IDENTICON_SIZE + EDGE_LABEL_PADDING) * zoom),
    }
}

fn draw_edge_label(painter: &egui::Painter, editor: &Editor, pos: Pos2, label: &Id, bg_color: Color32, zoom: f32) {
    match edge_label_text(editor, label) {
        Some(text) => {
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(TEXT_FONT_SIZE * zoom), Color32::from_gray(80));
            let bg_rect = Rect::from_center_size(pos, galley.rect.size() + Vec2::splat(EDGE_LABEL_PADDING * zoom));
            painter.rect_filled(bg_rect, CornerRadius::ZERO, bg_color);
            painter.galley(bg_rect.min + Vec2::splat(EDGE_LABEL_PADDING * zoom / 2.0), galley, Color32::from_gray(80));
        }
        None => {
            if let Id::Uuid(uuid) = label {
                super::identicon::draw_at(painter, Rect::from_center_size(pos, Vec2::splat(EDGE_LABEL_IDENTICON_SIZE * zoom)), uuid);
            }
        }
    }
}

fn graph_to_screen(pos: Pos2, panel_center: Vec2, pan_offset: Vec2, zoom: f32) -> Pos2 {
    (pos.to_vec2() * zoom).to_pos2() + panel_center + pan_offset
}

fn screen_to_graph(pos: Pos2, panel_center: Vec2, pan_offset: Vec2, zoom: f32) -> Pos2 {
    ((pos.to_vec2() - panel_center - pan_offset) / zoom).to_pos2()
}

pub fn render(ui: &mut egui::Ui, ctx: &egui::Context, editor: &Editor, w: &mut EditorWriter) {
    let mut state = editor.graph_view.clone();
    let panel_rect = ui.max_rect();
    let panel_center = panel_rect.center().to_vec2();

    let pointer_in_panel = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| panel_rect.contains(p));
    let scroll = ui.input(|i| i.smooth_scroll_delta);
    let zoom = ui.input(|i| i.zoom_delta());

    if pointer_in_panel && zoom != 1.0 {
        let new_zoom = (state.zoom * zoom).clamp(0.1, 5.0);
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            let graph_pos = screen_to_graph(cursor, panel_center, state.pan_offset, state.zoom);
            state.zoom = new_zoom;
            let new_screen = graph_to_screen(graph_pos, panel_center, state.pan_offset, state.zoom);
            state.pan_offset += cursor - new_screen;
        } else {
            state.zoom = new_zoom;
        }
    }

    if pointer_in_panel && scroll != Vec2::ZERO {
        state.pan_offset += scroll;
    }

    sync_positions(&mut state, &editor.doc);
    let edges = collect_edges(&editor.doc);
    simulate(&mut state, &edges);

    let graph_selected_edge = editor.selection.as_ref()
        .and_then(|s| match &s.target {
            SelectionTarget::GraphEdge { entity, label } => Some((entity, label)),
            _ => None,
        });

    let painter = ui.painter();
    let bg_color = ui.visuals().panel_fill;
    let half_sizes = compute_half_sizes(painter, editor, state.positions.keys().cloned());
    let response = ui.interact(panel_rect, ui.id().with("graph_bg"), egui::Sense::click_and_drag());

    let pointer = response.interact_pointer_pos();
    let hit = pointer.and_then(|p| state.positions.iter().find(|&(id, pos)| {
        let screen = graph_to_screen(*pos, panel_center, state.pan_offset, state.zoom);
        Rect::from_center_size(screen, node_half_size(&half_sizes, id) * 2.0 * state.zoom).contains(p)
    }).map(|(id, pos)| (id.clone(), *pos)));

    if ui.input(|i| i.pointer.primary_pressed()) && state.dragging.is_none() {
        state.pending_drag = hit.clone();
    }

    if response.drag_started() && state.dragging.is_none() {
        if let Some((ref id, pos)) = state.pending_drag.take() {
            state.dragging = Some(id.clone());
            state.drag_offset = graph_to_screen(pos, panel_center, state.pan_offset, state.zoom) - pointer.unwrap();
        } else {
            state.panning = true;
        }
    }

    if response.dragged() {
        if let Some(id) = state.dragging.clone() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let new_pos = screen_to_graph(pointer + state.drag_offset, panel_center, state.pan_offset, state.zoom);
                state.positions.insert(id.clone(), new_pos);
                state.velocities.insert(id, Vec2::ZERO);
            }
        } else if state.panning {
            state.pan_offset += response.drag_delta();
        }
    }

    if response.drag_stopped() {
        state.dragging = None;
        state.panning = false;
        state.pending_drag = None;
    }

    let to_screen = |pos: Pos2| graph_to_screen(pos, panel_center, state.pan_offset, state.zoom);

    let mut pair_counts: HashMap<(Id, Id), usize> = HashMap::new();
    for edge in &edges {
        *pair_counts.entry(canonical_pair(&edge.source, &edge.target)).or_default() += 1;
    }
    let mut pair_indices: HashMap<(Id, Id), usize> = HashMap::new();

    let arrow_stroke = Stroke::new(1.5 * state.zoom, Color32::from_gray(120));
    let selected_stroke = Stroke::new(2.5 * state.zoom, colors::SELECTION);
    let curve_spacing = 25.0;
    let mut edge_hit_zones: Vec<(Rect, Id, Id)> = Vec::new();

    for edge in &edges {
        let is_selected = graph_selected_edge == Some((&edge.source, &edge.label));
        let stroke = if is_selected { selected_stroke } else { arrow_stroke };
        if let (Some(&sp), Some(&tp)) = (state.positions.get(&edge.source), state.positions.get(&edge.target)) {
            let src_pos = to_screen(sp);
            let tgt_pos = to_screen(tp);
            let src_half = half_sizes.get(&edge.source).copied().unwrap_or(Vec2::splat(NODE_RADIUS)) * state.zoom;
            let tgt_half = half_sizes.get(&edge.target).copied().unwrap_or(Vec2::splat(NODE_RADIUS)) * state.zoom;

            let pair_key = canonical_pair(&edge.source, &edge.target);
            let total = pair_counts[&pair_key];
            let idx = pair_indices.entry(pair_key).or_default();
            let edge_idx = *idx;
            *idx += 1;
            let curve_offset = (edge_idx as f32 - (total - 1) as f32 / 2.0) * curve_spacing * state.zoom;

            if edge.source == edge.target {
                let loop_height = (NODE_RADIUS * 2.5 + edge_idx as f32 * 20.0) * state.zoom;
                let loop_width = (NODE_RADIUS * 1.5 + edge_idx as f32 * 8.0) * state.zoom;
                let cp1 = src_pos + Vec2::new(-loop_width, -loop_height);
                let cp2 = src_pos + Vec2::new(loop_width, -loop_height);
                let start = clip_to_rect(src_pos, src_half, cp1);
                let end = clip_to_rect(src_pos, src_half, cp2);
                let points: Vec<Pos2> = (0..=20)
                    .map(|i| {
                        let t = i as f32 / 20.0;
                        let mt = 1.0 - t;
                        Pos2::new(
                            mt * mt * mt * start.x + 3.0 * mt * mt * t * cp1.x
                                + 3.0 * mt * t * t * cp2.x + t * t * t * end.x,
                            mt * mt * mt * start.y + 3.0 * mt * mt * t * cp1.y
                                + 3.0 * mt * t * t * cp2.y + t * t * t * end.y,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
                draw_arrowhead(painter, end, (end - cp2).normalized(), stroke, state.zoom);
                let label_pos = Pos2::new(
                    0.125 * start.x + 0.375 * cp1.x + 0.375 * cp2.x + 0.125 * end.x,
                    0.125 * start.y + 0.375 * cp1.y + 0.375 * cp2.y + 0.125 * end.y,
                );
                let label_size = edge_label_size(painter, editor, &edge.label, state.zoom);
                edge_hit_zones.push((Rect::from_center_size(label_pos, label_size), edge.source.clone(), edge.label.clone()));
                draw_edge_label(painter, editor, label_pos, &edge.label, bg_color, state.zoom);
            } else {
                let mid = src_pos + (tgt_pos - src_pos) * 0.5;
                let canonical_dir = if edge.source <= edge.target {
                    (tgt_pos - src_pos).normalized()
                } else {
                    (src_pos - tgt_pos).normalized()
                };
                let perp = Vec2::new(-canonical_dir.y, canonical_dir.x);
                let control = mid + perp * curve_offset;
                let end = clip_to_rect_toward(tgt_pos, tgt_half, control, src_pos);

                let points: Vec<Pos2> = (0..=20)
                    .map(|i| {
                        let t = i as f32 / 20.0;
                        let mt = 1.0 - t;
                        Pos2::new(
                            mt * mt * src_pos.x + 2.0 * mt * t * control.x + t * t * end.x,
                            mt * mt * src_pos.y + 2.0 * mt * t * control.y + t * t * end.y,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
                draw_arrowhead(painter, end, ((end - control) * 2.0).normalized(), stroke, state.zoom);

                let label_pos = Pos2::new(
                    0.25 * src_pos.x + 0.5 * control.x + 0.25 * end.x,
                    0.25 * src_pos.y + 0.5 * control.y + 0.25 * end.y,
                );
                let label_size = edge_label_size(painter, editor, &edge.label, state.zoom);
                edge_hit_zones.push((Rect::from_center_size(label_pos, label_size), edge.source.clone(), edge.label.clone()));
                draw_edge_label(painter, editor, label_pos, &edge.label, bg_color, state.zoom);
            }
        }
    }

    let node_fill = Color32::WHITE;
    let text_font = egui::FontId::proportional(TEXT_FONT_SIZE * state.zoom);
    let root_ids: std::collections::HashSet<Id> = editor.doc.roots.iter().map(|r| r.value.clone()).collect();
    let root_stroke = Stroke::new(2.0 * state.zoom, Color32::from_gray(60));
    let selected_node = editor.selected_node_id();

    for (id, &pos) in &state.positions {
        let screen_pos = to_screen(pos);
        let is_root = root_ids.contains(id);
        let is_selected = selected_node.as_ref() == Some(id);
        match id {
            Id::Uuid(uuid) => {
                let stroke = if is_selected { selected_stroke } else if is_root { root_stroke } else { Stroke::new(2.0 * state.zoom, Color32::from_gray(100)) };
                match editor.display_label(id) {
                    Some(label) => {
                        let galley = painter.layout_no_wrap(label.clone(), text_font.clone(), Color32::from_gray(60));
                        let text_rect = Rect::from_center_size(screen_pos, galley.rect.size() + Vec2::splat(TEXT_PADDING * state.zoom));
                        let rounding = CornerRadius::same((4.0 * state.zoom) as u8);
                        painter.rect_filled(text_rect, rounding, Color32::WHITE);
                        painter.rect_stroke(text_rect, rounding, stroke, eframe::epaint::StrokeKind::Outside);
                        painter.galley(text_rect.min + Vec2::splat(TEXT_PADDING * state.zoom / 2.0), galley, Color32::from_gray(60));
                    }
                    None => {
                        let icon_rect = Rect::from_center_size(screen_pos, Vec2::splat(NODE_RADIUS * 1.4 * state.zoom));
                        super::identicon::draw_at(painter, icon_rect, uuid);
                        painter.rect_stroke(icon_rect, CornerRadius::same((2.0 * state.zoom) as u8), stroke, eframe::epaint::StrokeKind::Outside);
                    }
                }
            }
            _ => {
                let half = half_sizes.get(id).copied().unwrap_or(Vec2::splat(NODE_RADIUS));
                let rect = Rect::from_center_size(screen_pos, half * 2.0 * state.zoom);
                let rounding = CornerRadius::same((6.0 * state.zoom) as u8);
                painter.rect_filled(rect, rounding, node_fill);
                let stroke = if is_selected { selected_stroke } else if is_root { root_stroke } else { Stroke::new(1.5 * state.zoom, Color32::from_gray(160)) };
                painter.rect_stroke(rect, rounding, stroke, eframe::epaint::StrokeKind::Middle);
                if let Some(text) = node_display_text(id) {
                    painter.text(screen_pos, egui::Align2::CENTER_CENTER, text, text_font.clone(), Color32::from_gray(60));
                }
            }
        }
    }

    ctx.request_repaint();

    if response.clicked() {
        let edge_hit = pointer.and_then(|p| {
            edge_hit_zones.iter()
                .find(|(rect, _, _)| rect.contains(p))
                .map(|(_, entity, label)| Selection::graph_edge(entity.clone(), label.clone()))
        });
        let node_hit = pointer.and_then(|p| {
            state.positions.iter()
                .find(|(id, pos)| Rect::from_center_size(to_screen(**pos), node_half_size(&half_sizes, id) * 2.0 * state.zoom).contains(p))
                .map(|(id, _)| Selection::graph_node(id.clone()))
        });
        w.select(edge_hit.or(node_hit));
    }

    w.set_graph_view(state);
}
