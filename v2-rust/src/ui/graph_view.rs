use crate::graph::{Gid, Id, MutGid};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, Vec2};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault};

const NODE_RADIUS: f32 = 20.0;
const REPULSION_K: f32 = 8000.0;
const ATTRACTION_K: f32 = 0.02;
const REST_LENGTH: f32 = 120.0;
const DAMPING: f32 = 0.85;
const MAX_FORCE: f32 = 10.0;
const MAX_LABEL_LEN: usize = 8;

pub struct GraphViewState {
    positions: HashMap<Id, Pos2>,
    velocities: HashMap<Id, Vec2>,
    dragging: Option<Id>,
    drag_offset: Vec2,
    pan_offset: Vec2,
    panning: bool,
}

impl GraphViewState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            velocities: HashMap::new(),
            dragging: None,
            drag_offset: Vec2::ZERO,
            pan_offset: Vec2::ZERO,
            panning: false,
        }
    }
}

fn deterministic_pos(id: &Id, index: usize) -> Pos2 {
    let hash = BuildHasherDefault::<DefaultHasher>::default().hash_one(id);
    let x = ((hash & 0xFFFF) as f32 / 65535.0 - 0.5) * 300.0;
    let y = (((hash >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5) * 200.0;
    Pos2::new(x + index as f32 * 5.0, y + index as f32 * 5.0)
}

fn sync_positions(state: &mut GraphViewState, gid: &MutGid, roots: &[crate::graph::RootSlot]) {
    let all_ids: Vec<Id> = roots.iter().map(|r| r.node().clone())
        .chain(gid.entities().flat_map(|id| {
            std::iter::once(id.clone()).chain(
                gid.edges(id).into_iter().flat_map(|edges| edges.iter().map(|(_, v)| v.clone()))
            )
        }))
        .collect();

    for (i, id) in all_ids.iter().enumerate() {
        state.positions.entry(id.clone()).or_insert_with(|| deterministic_pos(id, i));
        state.velocities.entry(id.clone()).or_insert(Vec2::ZERO);
    }

    let id_set: std::collections::HashSet<&Id> = all_ids.iter().collect();
    state.positions.retain(|id, _| id_set.contains(id));
    state.velocities.retain(|id, _| id_set.contains(id));
}

struct Edge {
    source: Id,
    label: Id,
    target: Id,
}

fn collect_edges(gid: &MutGid) -> Vec<Edge> {
    gid.entities()
        .flat_map(|entity_id| {
            gid.edges(entity_id).into_iter().flat_map(move |edges| {
                edges.iter().map(move |(label, value)| Edge {
                    source: entity_id.clone(),
                    label: label.clone(),
                    target: value.clone(),
                })
            })
        })
        .collect()
}

fn simulate(state: &mut GraphViewState, edges: &[Edge]) {
    let ids: Vec<Id> = state.positions.keys().cloned().collect();
    let mut forces: HashMap<Id, Vec2> = ids.iter().map(|id| (id.clone(), Vec2::ZERO)).collect();

    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let delta = state.positions[&ids[i]] - state.positions[&ids[j]];
            let dist_sq = delta.length_sq().max(1.0);
            let force = delta.normalized() * (REPULSION_K / dist_sq).min(MAX_FORCE);
            *forces.get_mut(&ids[i]).unwrap() += force;
            *forces.get_mut(&ids[j]).unwrap() -= force;
        }
    }

    for edge in edges {
        if let (Some(&pa), Some(&pb)) = (state.positions.get(&edge.source), state.positions.get(&edge.target)) {
            let delta = pb - pa;
            let dist = delta.length().max(0.1);
            let force = delta.normalized() * (ATTRACTION_K * (dist - REST_LENGTH)).clamp(-MAX_FORCE, MAX_FORCE);
            *forces.get_mut(&edge.source).unwrap() += force;
            *forces.get_mut(&edge.target).unwrap() -= force;
        }
    }

    for id in ids.iter().filter(|id| state.dragging.as_ref() != Some(*id)) {
        let vel = state.velocities.get_mut(id).unwrap();
        *vel = (*vel + forces[id]) * DAMPING;
        let pos = state.positions.get_mut(id).unwrap();
        *pos += *vel;
    }
}

fn node_half_size(id: &Id) -> Vec2 {
    match id {
        Id::Uuid(_) => Vec2::splat(NODE_RADIUS * 0.7 + 2.0),
        _ => {
            let text = node_display_text(id).unwrap_or_default();
            Vec2::new((text.len() as f32 * 3.2 + 8.0).max(14.0), 12.0)
        }
    }
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

fn draw_arrowhead(painter: &egui::Painter, tip: Pos2, dir: Vec2, stroke: Stroke) {
    let perp = Vec2::new(-dir.y, dir.x);
    painter.add(egui::Shape::line(
        vec![tip - dir * 6.0 + perp * 3.0, tip, tip - dir * 6.0 - perp * 3.0],
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

fn draw_edge_label(painter: &egui::Painter, pos: Pos2, label: &Id) {
    match label {
        Id::Uuid(uuid) => {
            super::identicon::draw_at(painter, Rect::from_center_size(pos, Vec2::splat(18.0)), uuid);
        }
        _ => {
            if let Some(text) = node_display_text(label) {
                painter.text(
                    pos, egui::Align2::CENTER_CENTER, text,
                    egui::FontId::proportional(10.0), Color32::from_gray(100),
                );
            }
        }
    }
}

pub fn render(ui: &mut egui::Ui, ctx: &egui::Context, gid: &MutGid, roots: &[crate::graph::RootSlot], state: &mut GraphViewState) {
    let panel_rect = ui.max_rect();
    let view_offset = panel_rect.center().to_vec2() + state.pan_offset;

    sync_positions(state, gid, roots);
    let edges = collect_edges(gid);
    simulate(state, &edges);

    let painter = ui.painter();
    let response = ui.interact(panel_rect, ui.id().with("graph_bg"), egui::Sense::click_and_drag());

    if response.drag_started() && state.dragging.is_none() {
        if let Some(pointer) = response.interact_pointer_pos() {
            let hit = state.positions.iter().find(|&(_, pos)| {
                (pointer - (*pos + view_offset)).length() <= NODE_RADIUS
            });
            if let Some((id, pos)) = hit {
                state.dragging = Some(id.clone());
                state.drag_offset = (*pos + view_offset) - pointer;
            } else {
                state.panning = true;
            }
        }
    }

    if response.dragged() {
        if let Some(id) = state.dragging.clone() {
            if let Some(pointer) = response.interact_pointer_pos() {
                state.positions.insert(id.clone(), pointer + state.drag_offset - view_offset);
                state.velocities.insert(id, Vec2::ZERO);
            }
        } else if state.panning {
            state.pan_offset += response.drag_delta();
        }
    }

    if response.drag_stopped() {
        state.dragging = None;
        state.panning = false;
    }

    let half_sizes: HashMap<&Id, Vec2> = state.positions.keys().map(|id| (id, node_half_size(id))).collect();

    let mut pair_counts: HashMap<(Id, Id), usize> = HashMap::new();
    for edge in &edges {
        *pair_counts.entry(canonical_pair(&edge.source, &edge.target)).or_default() += 1;
    }
    let mut pair_indices: HashMap<(Id, Id), usize> = HashMap::new();

    let arrow_stroke = Stroke::new(1.5, Color32::from_gray(120));
    let curve_spacing = 25.0;

    for edge in &edges {
        if let (Some(&sp), Some(&tp)) = (state.positions.get(&edge.source), state.positions.get(&edge.target)) {
            let src_pos = sp + view_offset;
            let tgt_pos = tp + view_offset;
            let src_half = half_sizes.get(&edge.source).copied().unwrap_or(Vec2::splat(NODE_RADIUS));
            let tgt_half = half_sizes.get(&edge.target).copied().unwrap_or(Vec2::splat(NODE_RADIUS));

            let pair_key = canonical_pair(&edge.source, &edge.target);
            let total = pair_counts[&pair_key];
            let idx = pair_indices.entry(pair_key).or_default();
            let edge_idx = *idx;
            *idx += 1;
            let curve_offset = (edge_idx as f32 - (total - 1) as f32 / 2.0) * curve_spacing;

            if edge.source == edge.target {
                let loop_height = NODE_RADIUS * 2.5 + edge_idx as f32 * 20.0;
                let loop_width = NODE_RADIUS * 1.5 + edge_idx as f32 * 8.0;
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
                painter.add(egui::Shape::line(points, arrow_stroke));
                draw_arrowhead(painter, end, (end - cp2).normalized(), arrow_stroke);
                draw_edge_label(painter, cp1 + (cp2 - cp1) * 0.5 - Vec2::new(0.0, 6.0), &edge.label);
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
                painter.add(egui::Shape::line(points, arrow_stroke));
                draw_arrowhead(painter, end, ((end - control) * 2.0).normalized(), arrow_stroke);

                let label_pos = Pos2::new(
                    0.25 * src_pos.x + 0.5 * control.x + 0.25 * end.x,
                    0.25 * src_pos.y + 0.5 * control.y + 0.25 * end.y,
                );
                draw_edge_label(painter, label_pos, &edge.label);
            }
        }
    }

    let node_fill = Color32::WHITE;
    let node_stroke = Stroke::new(1.5, Color32::from_gray(160));
    let text_font = egui::FontId::proportional(10.0);

    for (id, &pos) in &state.positions {
        let screen_pos = pos + view_offset;
        match id {
            Id::Uuid(uuid) => {
                let icon_rect = Rect::from_center_size(screen_pos, Vec2::splat(NODE_RADIUS * 1.4));
                super::identicon::draw_at(painter, icon_rect, uuid);
                painter.rect_stroke(
                    icon_rect, CornerRadius::same(2),
                    Stroke::new(2.0, Color32::from_gray(100)),
                    eframe::epaint::StrokeKind::Outside,
                );
            }
            _ => {
                let half = half_sizes.get(id).copied().unwrap_or(Vec2::splat(NODE_RADIUS));
                let rect = Rect::from_center_size(screen_pos, half * 2.0);
                let rounding = CornerRadius::same(6);
                painter.rect_filled(rect, rounding, node_fill);
                painter.rect_stroke(rect, rounding, node_stroke, eframe::epaint::StrokeKind::Middle);
                if let Some(text) = node_display_text(id) {
                    painter.text(screen_pos, egui::Align2::CENTER_CENTER, text, text_font.clone(), Color32::from_gray(60));
                }
            }
        }
    }

    ctx.request_repaint();
}
