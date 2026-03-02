use crate::document::Document;
use crate::graph::{Gid, Id, MutGid};
use crate::math::{Pos2, Vec2};
use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault};
use std::collections::hash_map::DefaultHasher;

const REPULSION_K: f32 = 8000.0;
const ATTRACTION_K: f32 = 0.02;
const REST_LENGTH: f32 = 120.0;
const DAMPING: f32 = 0.85;
const MAX_FORCE: f32 = 10.0;
const GRAVITY_K: f32 = 0.005;

#[derive(Clone)]
pub struct GraphViewState {
    pub positions: HashMap<Id, Pos2>,
    pub velocities: HashMap<Id, Vec2>,
    pub prev_gid: MutGid,
}

impl GraphViewState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            velocities: HashMap::new(),
            prev_gid: MutGid::new(),
        }
    }
}

pub struct Edge {
    pub source: Id,
    pub label: Id,
    pub target: Id,
}

fn deterministic_pos(id: &Id, index: usize) -> Pos2 {
    let hash = BuildHasherDefault::<DefaultHasher>::default().hash_one(id);
    let x = ((hash & 0xFFFF) as f32 / 65535.0 - 0.5) * 300.0;
    let y = (((hash >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5) * 200.0;
    Pos2::new(x + index as f32 * 5.0, y + index as f32 * 5.0)
}

fn collect_all_ids(doc: &Document) -> std::collections::HashSet<Id> {
    doc.gid.all_ids().into_iter()
        .chain(doc.roots.iter().map(|r| r.value.clone()))
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

pub fn collect_edges(doc: &Document) -> Vec<Edge> {
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

    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let delta = positions[&ids[i]] - positions[&ids[j]];
            let dist_sq = delta.length_sq().max(1.0);
            let force = delta.normalized() * (REPULSION_K / dist_sq).min(MAX_FORCE);
            *forces.get_mut(&ids[i]).unwrap() += force;
            *forces.get_mut(&ids[j]).unwrap() -= force;
        }
    }

    for edge in edges {
        if let (Some(&pa), Some(&pb)) = (positions.get(&edge.source), positions.get(&edge.target)) {
            let delta = pb - pa;
            let dist = delta.length().max(0.1);
            let force = delta.normalized() * (ATTRACTION_K * (dist - REST_LENGTH)).clamp(-MAX_FORCE, MAX_FORCE);
            *forces.get_mut(&edge.source).unwrap() += force;
            *forces.get_mut(&edge.target).unwrap() -= force;
        }
    }

    for id in &ids {
        *forces.get_mut(id).unwrap() += -positions[id].to_vec2() * GRAVITY_K;
    }

    forces
}

fn apply_forces(state: &mut GraphViewState, forces: &HashMap<Id, Vec2>, dragging: Option<&Id>) {
    for (id, force) in forces {
        if dragging == Some(id) { continue; }
        let vel = state.velocities.get_mut(id).unwrap();
        *vel = (*vel + *force) * DAMPING;
        let pos = state.positions.get_mut(id).unwrap();
        *pos += *vel;
    }
}

fn simulate(state: &mut GraphViewState, edges: &[Edge], dragging: Option<&Id>) {
    let forces = compute_forces(&state.positions, edges);
    apply_forces(state, &forces, dragging);
}

pub fn step_physics(state: &mut GraphViewState, doc: &Document, dragging: Option<&Id>) {
    sync_positions(state, doc);
    let edges = collect_edges(doc);
    simulate(state, &edges, dragging);
}
