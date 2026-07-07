//! The graph view: the document as spatial nodes and edges, for
//! demos on small graphs. Every identity that appears as an entity or
//! a value is a node — atoms included, so a shared `2` is visibly one
//! node — and every (entity, label, value) is a directed edge with a
//! label pill. Layout is the force simulation carried from the
//! TypeScript/egui/Haskell prototypes (same constants), stepped every
//! frame while the view is open; positions and velocities are
//! explicit model state, seeded deterministically per id. Rendering
//! and hit-testing are one pure pass: build geometry from state, draw
//! it, register handlers over it.

use crate::raw::{Document, Selection, command, hex, resolve, short_id};
use parley::style::GenericFamily;
use parley::{Layout, StyleProperty};
use progred_graph::{Gid, Id, NodeId, position};
use puri::draw::Canvas;
use puri::handler::HasHandler;
use puri::layout::{Extent, Node, leaf};
use puri::text::{TextCtx, draw_layout};
use std::collections::HashMap;
use std::rc::Rc;
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, BezPath, Point, Rect, RoundedRect, Stroke, Vec2};
use vello::peniko::{Brush, Color};

const REPULSION_K: f64 = 8000.0;
const ATTRACTION_K: f64 = 0.02;
const REST_LENGTH: f64 = 120.0;
const DAMPING: f64 = 0.85;
const MAX_FORCE: f64 = 10.0;
const GRAVITY_K: f64 = 0.005;
const PARALLEL_SPACING: f64 = 50.0;
const CLICK_SLOP: f64 = 2.0;

/// The pane's window rectangle: the right 40% of the viewport.
pub fn panel(width: f64, height: f64) -> Rect {
    Rect::new((width * 0.6).round(), 0.0, width, height)
}

#[derive(Clone, PartialEq, Eq)]
pub enum GraphSelection {
    Node(Id),
    Edge { source: Id, label: Id },
}

/// An in-progress drag: a node being moved (with where the pointer
/// grabbed it, as a world offset from the node position), or the
/// background panning the viewport. Either way, an unmoved release is
/// a click — selecting the node, or clearing the selection.
enum Drag {
    Node {
        node: Id,
        grab: Vec2,
        pressed: Point,
        moved: bool,
    },
    Pan {
        last: Point,
        pressed: Point,
        moved: bool,
    },
}

/// The graph view's explicit state: per-node world positions and
/// velocities, the viewport (world-space pan, zoom), the graph's own
/// selection, and any drag in progress.
pub struct GraphView {
    positions: HashMap<Id, Point>,
    velocities: HashMap<Id, Vec2>,
    pub selection: Option<GraphSelection>,
    drag: Option<Drag>,
    pan: Vec2,
    zoom: f64,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            positions: HashMap::new(),
            velocities: HashMap::new(),
            selection: None,
            drag: None,
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

struct Snapshot {
    nodes: Vec<Id>,
    edges: Vec<(Id, Id, Id)>,
}

/// Every identity in the document: entities plus every edge value.
/// Labels appear on edge pills, not as nodes.
fn snapshot(doc: &Document) -> Snapshot {
    let mut nodes: Vec<Id> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |nodes: &mut Vec<Id>, id: &Id| {
        if seen.insert(id.clone()) {
            nodes.push(id.clone());
        }
    };
    let mut edges = Vec::new();
    let mut entities: Vec<NodeId> = doc.gid.entities().copied().collect();
    entities.sort();
    for entity in entities {
        let source = Id::from(entity);
        add(&mut nodes, &source);
        let mut outgoing: Vec<(Id, Id)> = doc
            .gid
            .edges(&source)
            .map(|edges| edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        outgoing.sort();
        for (label, value) in outgoing {
            add(&mut nodes, &value);
            edges.push((source.clone(), label, value));
        }
    }
    if let Some(root) = &doc.root {
        add(&mut nodes, root);
    }
    Snapshot { nodes, edges }
}

/// FNV-1a over the id's stable bytes, for deterministic seeding.
fn id_hash(id: &Id) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    let mut eat = |byte: u8| hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    for byte in id.space().as_bytes() {
        eat(*byte);
    }
    for byte in id.payload() {
        eat(*byte);
    }
    hash
}

impl GraphView {
    /// Sync positions with the document (seed new nodes, drop stale
    /// ones) and advance the simulation one step. Runs every frame
    /// while the view is open; a dragged node is pinned.
    pub fn step(&mut self, doc: &Document) {
        let snapshot = snapshot(doc);
        self.sync(&snapshot);
        let forces = self.forces(&snapshot);
        let dragged = match &self.drag {
            Some(Drag::Node { node, .. }) => Some(node.clone()),
            _ => None,
        };
        for id in &snapshot.nodes {
            if dragged.as_ref() == Some(id) {
                continue;
            }
            let force = forces.get(id).copied().unwrap_or_default();
            let velocity = self.velocities.entry(id.clone()).or_default();
            *velocity = (*velocity + force) * DAMPING;
            let position = self.positions.entry(id.clone()).or_default();
            *position += *velocity;
        }
    }

    fn sync(&mut self, snapshot: &Snapshot) {
        for (index, id) in snapshot.nodes.iter().enumerate() {
            if !self.positions.contains_key(id) {
                let position = if self.positions.is_empty() {
                    Point::ZERO
                } else {
                    let hash = id_hash(id);
                    let x = (f64::from(hash & 0xFFFF) / 65535.0 - 0.5) * 300.0;
                    let y = (f64::from((hash >> 16) & 0xFFFF) / 65535.0 - 0.5) * 200.0;
                    Point::new(x + index as f64 * 5.0, y + index as f64 * 5.0)
                };
                self.positions.insert(id.clone(), position);
                self.velocities.insert(id.clone(), Vec2::ZERO);
            }
        }
        self.positions
            .retain(|id, _| snapshot.nodes.contains(id));
        self.velocities
            .retain(|id, _| snapshot.nodes.contains(id));
    }

    fn forces(&self, snapshot: &Snapshot) -> HashMap<Id, Vec2> {
        let mut forces: HashMap<Id, Vec2> =
            snapshot.nodes.iter().map(|id| (id.clone(), Vec2::ZERO)).collect();
        let unit = |delta: Vec2| {
            let length = delta.hypot();
            if length < 1e-6 {
                Vec2::new(1.0, 0.0)
            } else {
                delta / length
            }
        };
        for i in 0..snapshot.nodes.len() {
            for j in (i + 1)..snapshot.nodes.len() {
                let (a, b) = (&snapshot.nodes[i], &snapshot.nodes[j]);
                let delta = self.positions[a] - self.positions[b];
                let force =
                    unit(delta) * (REPULSION_K / delta.hypot2().max(1.0)).min(MAX_FORCE);
                *forces.get_mut(a).unwrap() += force;
                *forces.get_mut(b).unwrap() -= force;
            }
        }
        for (source, _, target) in &snapshot.edges {
            let delta = self.positions[target] - self.positions[source];
            let distance = delta.hypot().max(0.1);
            let magnitude =
                (ATTRACTION_K * (distance - REST_LENGTH)).clamp(-MAX_FORCE, MAX_FORCE);
            let force = unit(delta) * magnitude;
            *forces.get_mut(source).unwrap() += force;
            *forces.get_mut(target).unwrap() -= force;
        }
        for id in &snapshot.nodes {
            *forces.get_mut(id).unwrap() += self.positions[id].to_vec2() * -GRAVITY_K;
        }
        forces
    }

    /// `grab` is the world offset from the node position to the
    /// pointer; `pressed` is in panel pixels, for the click slop.
    pub fn press_node(&mut self, node: Id, grab: Vec2, pressed: Point) {
        self.drag = Some(Drag::Node {
            node,
            grab,
            pressed,
            moved: false,
        });
    }

    /// A background press begins a pan; `panel` in panel pixels.
    pub fn press_background(&mut self, panel: Point) {
        self.drag = Some(Drag::Pan {
            last: panel,
            pressed: panel,
            moved: false,
        });
    }

    /// Drag the press: a node moves in world space, a pan shifts the
    /// viewport by panel pixels. Returns whether a drag was active.
    pub fn drag_to(&mut self, world: Point, panel: Point, px_per_world: f64) -> bool {
        match &mut self.drag {
            Some(Drag::Node {
                node,
                grab,
                pressed,
                moved,
            }) => {
                if (panel - *pressed).hypot() > CLICK_SLOP {
                    *moved = true;
                }
                if *moved {
                    let target = world - *grab;
                    self.positions.insert(node.clone(), target);
                    self.velocities.insert(node.clone(), Vec2::ZERO);
                }
                true
            }
            Some(Drag::Pan {
                last,
                pressed,
                moved,
            }) => {
                if (panel - *pressed).hypot() > CLICK_SLOP {
                    *moved = true;
                }
                self.pan += (panel - *last) / px_per_world;
                *last = panel;
                true
            }
            None => false,
        }
    }

    /// Ends a press; an unmoved one is a click — selecting the node,
    /// or clearing the selection for the background.
    pub fn release(&mut self) -> bool {
        match self.drag.take() {
            Some(Drag::Node { node, moved, .. }) => {
                if !moved {
                    self.selection = Some(GraphSelection::Node(node));
                }
                true
            }
            Some(Drag::Pan { moved, .. }) => {
                if !moved {
                    self.selection = None;
                }
                true
            }
            None => false,
        }
    }

    /// Whether the simulation is visibly moving (or held by a drag);
    /// when false the shell lets the continuous redraw sleep.
    pub fn hot(&self) -> bool {
        self.drag.is_some() || self.velocities.values().any(|v| v.hypot() > 0.02)
    }

    /// Zooms by `factor` keeping the world point under `anchor` (panel
    /// pixels from the panel center) fixed.
    pub fn zoom_at(&mut self, factor: f64, anchor: Vec2, scale: f64) {
        let anchor_world = anchor / (scale * self.zoom) - self.pan;
        let old = self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.1, 5.0);
        self.pan = (anchor_world + self.pan) * (old / self.zoom) - anchor_world;
    }

    /// Interprets a scroll over the panel: trackpad pixels pan, wheel
    /// lines and pages zoom toward the cursor. `cursor` in panel
    /// pixels from the panel center.
    pub fn scroll(
        &mut self,
        delta: &ui_events::ScrollDelta,
        cursor: Vec2,
        scale: f64,
    ) {
        match delta {
            ui_events::ScrollDelta::PixelDelta(pixels) => {
                self.pan += Vec2::new(pixels.x, pixels.y) / (scale * self.zoom);
            }
            ui_events::ScrollDelta::LineDelta(_, y)
            | ui_events::ScrollDelta::PageDelta(_, y) => {
                self.zoom_at(1.1_f64.powf(f64::from(*y)), cursor, scale);
            }
        }
    }

    /// The graph-selected node, for the projection's secondary marks.
    pub fn selected_node(&self) -> Option<&Id> {
        match &self.selection {
            Some(GraphSelection::Node(id)) => Some(id),
            _ => None,
        }
    }
}

/// Deletes the selection from the graph: an edge is one detachment; a
/// node is fully detached — the root cleared if it is the root, every
/// outgoing edge removed, and every edge anywhere targeting it
/// removed. Unreferenced values simply stop appearing.
pub fn delete_selection(doc: &mut Document, selection: &GraphSelection) -> bool {
    let before = doc.gid.clone();
    let before_root = doc.root.clone();
    match selection {
        GraphSelection::Edge { source, label } => {
            if let Some(entity) = source.as_node_id() {
                doc.gid.delete(&entity, label);
            }
        }
        GraphSelection::Node(id) => {
            if doc.root.as_ref() == Some(id) {
                doc.root = None;
            }
            if let Some(entity) = id.as_node_id() {
                let outgoing: Vec<Id> = doc
                    .gid
                    .edges(id)
                    .map(|edges| edges.keys().cloned().collect())
                    .unwrap_or_default();
                for label in outgoing {
                    doc.gid.delete(&entity, &label);
                }
            }
            let incoming: Vec<(NodeId, Id)> = doc
                .gid
                .entities()
                .flat_map(|entity| {
                    let source = Id::from(*entity);
                    doc.gid
                        .edges(&source)
                        .into_iter()
                        .flatten()
                        .filter(|(_, value)| *value == id)
                        .map(|(label, _)| (*entity, label.clone()))
                        .collect::<Vec<_>>()
                })
                .collect();
            for (source, label) in incoming {
                doc.gid.delete(&source, &label);
            }
        }
    }
    !(doc.gid.ptr_eq(&before) && doc.root == before_root)
}

/// Dispatch-time callbacks the shell injects, mirroring `raw::Hooks`:
/// the pane reports what happened in world coordinates; the shell
/// owns the transitions.
pub struct Hooks<C> {
    pub press_node: Rc<dyn Fn(&mut C, Id, Vec2, Point)>,
    pub press_edge: Rc<dyn Fn(&mut C, Id, Id)>,
    pub press_background: Rc<dyn Fn(&mut C, Point)>,
    /// (world point, window point, panel pixels per world unit).
    pub drag_to: Rc<dyn Fn(&mut C, Point, Point, f64) -> bool>,
    pub release: Rc<dyn Fn(&mut C) -> bool>,
    /// Command-click: commit the pointed-at identity into the open
    /// pending; false when nothing is pending.
    pub pick: Rc<dyn Fn(&mut C, Id) -> bool>,
}

const FONT_SIZE: f32 = 10.0;
const NODE_PADDING: f64 = 7.0;
const NODE_MIN_HEIGHT: f64 = 24.0;
const PILL_PADDING: f64 = 4.0;
const ARROW_LENGTH: f64 = 7.0;
const ARROW_WIDTH: f64 = 3.5;

const PANEL_BG: [f32; 4] = [0.968, 0.972, 0.984, 1.0];
const SEPARATOR: [f32; 4] = [0.851, 0.867, 0.890, 1.0];
const NODE_FILL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const ROOT_FILL: [f32; 4] = [0.922, 0.941, 0.980, 1.0];
const BORDER: [f32; 4] = [0.467, 0.467, 0.467, 1.0];
const EDGE: [f32; 4] = [0.561, 0.588, 0.631, 1.0];
/// Pane-local primary: a full-strength stroke in the selection blue.
const PRIMARY: [f32; 4] = [0.0, 0.48, 1.0, 1.0];
/// The secondary mark, shared with the tree: a translucent blue wash
/// plus a thin translucent outline — clearly related to the primary,
/// clearly not it.
const WASH: [f32; 4] = [0.0, 0.48, 1.0, 0.10];
const SECONDARY_OUTLINE: [f32; 4] = [0.0, 0.48, 1.0, 0.55];
const PILL_BG: [f32; 4] = [0.984, 0.984, 0.980, 1.0];
const TEXT: [f32; 4] = [0.13, 0.14, 0.16, 1.0];
const STRING_TEXT: [f32; 4] = [0.55, 0.33, 0.28, 1.0];
const NUMBER_TEXT: [f32; 4] = [0.16, 0.40, 0.62, 1.0];
const DIM_TEXT: [f32; 4] = [0.55, 0.58, 0.64, 1.0];

struct NodeView {
    id: Id,
    rect: Rect,
    content: Layout<Brush>,
    root: bool,
    strength: Strength,
}

#[derive(Clone, Copy, PartialEq)]
enum Strength {
    None,
    Secondary,
    Primary,
}

struct EdgeView {
    source: Id,
    label: Id,
    path: BezPath,
    arrow: BezPath,
    pill: Rect,
    pill_content: Layout<Brush>,
    strength: Strength,
    /// The pill can outrank the curve: a label that is the secondary
    /// identity marks the pill alone, as the tree marks label text.
    pill_strength: Strength,
}

fn layout_text(
    tcx: &mut TextCtx,
    s: &str,
    size: f32,
    color: [f32; 4],
    family: GenericFamily,
) -> Layout<Brush> {
    let mut builder = tcx.layouts.ranged_builder(tcx.fonts, s, tcx.scale, true);
    builder.push_default(StyleProperty::Brush(Brush::from(Color::new(color))));
    builder.push_default(family);
    builder.push_default(StyleProperty::FontSize(size));
    let mut layout: Layout<Brush> = builder.build(s);
    layout.break_all_lines(None);
    layout
}

/// What an identity looks like in the graph: named nodes show their
/// name, unnamed ones their short id, atoms their value — the same
/// identity language as the tree view.
fn content(doc: &Document, id: &Id, tcx: &mut TextCtx, zoom: f64) -> Layout<Brush> {
    let size = FONT_SIZE * zoom as f32;
    let ui = GenericFamily::SystemUi;
    let mono = GenericFamily::Monospace;
    if let Some(s) = id.as_str() {
        layout_text(tcx, &format!("\"{s}\""), size, STRING_TEXT, ui)
    } else if let Some(n) = id.as_number() {
        layout_text(tcx, &n.to_string(), size, NUMBER_TEXT, ui)
    } else if let Some(node) = id.as_node_id() {
        match doc
            .gid
            .get(id, &Id::from(crate::conventions::NAME))
            .and_then(Id::as_str)
        {
            Some(name) => layout_text(tcx, name, size, TEXT, ui),
            None => layout_text(tcx, &short_id(node), size, DIM_TEXT, mono),
        }
    } else if let Some(bytes) = position::as_position(id) {
        layout_text(tcx, &hex(bytes), size, DIM_TEXT, mono)
    } else {
        layout_text(tcx, &hex(id.payload()), size, DIM_TEXT, mono)
    }
}

fn draw_content<P: Canvas>(p: &mut P, layout: &Layout<Brush>, rect: Rect) {
    let at = Point::new(
        rect.center().x - f64::from(layout.width()) / 2.0,
        rect.center().y - f64::from(layout.height()) / 2.0,
    );
    draw_layout(p, layout, Affine::translate(at.to_vec2()));
}

/// Where the segment from `from` toward `to` crosses the boundary of
/// `rect` (centered on `from`), so edges stop at node borders.
fn clip_to_rect(from: Point, to: Point, rect: Rect) -> Point {
    let delta = to - from;
    let mut t: f64 = 1.0;
    if delta.x.abs() > 1e-9 {
        t = t.min((rect.width() / 2.0) / delta.x.abs());
    }
    if delta.y.abs() > 1e-9 {
        t = t.min((rect.height() / 2.0) / delta.y.abs());
    }
    from + delta * t.min(1.0)
}

fn quadratic_point(p0: Point, c: Point, p1: Point, t: f64) -> Point {
    let u = 1.0 - t;
    Point::new(
        u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x,
        u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y,
    )
}

fn arrowhead(tip: Point, direction: Vec2, scale: f64) -> BezPath {
    let unit = if direction.hypot() < 1e-6 {
        Vec2::new(1.0, 0.0)
    } else {
        direction / direction.hypot()
    };
    let normal = Vec2::new(-unit.y, unit.x);
    let back = tip - unit * (ARROW_LENGTH * scale);
    let mut path = BezPath::new();
    path.move_to(back + normal * (ARROW_WIDTH * scale));
    path.line_to(tip);
    path.line_to(back - normal * (ARROW_WIDTH * scale));
    path
}

/// The graph pane: geometry built from explicit state, drawn and
/// hit-tested in one pass. `panel` is the pane's window rectangle;
/// world origin maps to its center.
pub fn pane<C: 'static, P: Canvas + HasHandler<C>>(
    doc: &Document,
    view: &GraphView,
    doc_selection: Option<&Selection>,
    tcx: &mut TextCtx,
    panel: Rect,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = f64::from(tcx.scale);
    let zoom = view.zoom;
    // Panel pixels per world unit; world -> panel goes through the
    // viewport's pan and zoom.
    let px = scale * zoom;
    let pan = view.pan;
    let center = panel.center().to_vec2();
    let to_panel = |world: Point| ((world.to_vec2() + pan) * px).to_point() + center;
    let snapshot = snapshot(doc);

    // The document selection projects into the graph through its
    // VALUE only — the node it points at washes; mirroring the edge
    // itself as a pill wash read too much like a graph-side edge
    // selection.
    let doc_value = doc_selection.and_then(|selection| match selection {
        Selection::Edge { path, .. } => resolve(doc, path).cloned(),
        _ => None,
    });
    // The secondary identity, as in the tree: the selected edge's
    // value, or the graph's own selected node — every projection of
    // it marks, label pills included.
    let secondary = doc_value.or_else(|| match &view.selection {
        Some(GraphSelection::Node(id)) => Some(id.clone()),
        _ => None,
    });

    let node_views: Vec<NodeView> = snapshot
        .nodes
        .iter()
        .filter_map(|id| {
            let world = *view.positions.get(id)?;
            let content = content(doc, id, tcx, zoom);
            let (w, h) = (f64::from(content.width()), f64::from(content.height()));
            let width = w + 2.0 * NODE_PADDING * px;
            let height = (h + 2.0 * NODE_PADDING * px).max(NODE_MIN_HEIGHT * px);
            let at = to_panel(world);
            let strength = if view.selection == Some(GraphSelection::Node(id.clone())) {
                Strength::Primary
            } else if secondary.as_ref() == Some(id) {
                Strength::Secondary
            } else {
                Strength::None
            };
            Some(NodeView {
                id: id.clone(),
                rect: Rect::from_center_size(at, (width, height)),
                content,
                root: doc.root.as_ref() == Some(id),
                strength,
            })
        })
        .collect();
    let rect_of = |id: &Id| {
        node_views
            .iter()
            .find(|node| node.id == *id)
            .map(|node| node.rect)
    };

    // Parallel edges between the same unordered pair fan out; an
    // exact bidirectional pair splits evenly.
    let mut pair_counts: HashMap<(Id, Id), usize> = HashMap::new();
    let pair = |a: &Id, b: &Id| {
        if a <= b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        }
    };
    for (source, _, target) in &snapshot.edges {
        *pair_counts.entry(pair(source, target)).or_default() += 1;
    }
    let mut pair_seen: HashMap<(Id, Id), usize> = HashMap::new();

    let edge_views: Vec<EdgeView> = snapshot
        .edges
        .iter()
        .filter_map(|(source, label, target)| {
            let source_rect = rect_of(source)?;
            let target_rect = rect_of(target)?;
            let key = pair(source, target);
            let total = pair_counts[&key];
            let index = {
                let seen = pair_seen.entry(key).or_default();
                let index = *seen;
                *seen += 1;
                index
            };
            let offset =
                (index as f64 - (total as f64 - 1.0) / 2.0) * PARALLEL_SPACING * px;
            let strength = if view.selection
                == Some(GraphSelection::Edge {
                    source: source.clone(),
                    label: label.clone(),
                }) {
                Strength::Primary
            } else {
                Strength::None
            };
            let (path, mid, tip, tip_direction) = if source == target {
                // Self-loop: a cubic arch above the node, growing
                // with its index so stacked loops separate.
                let top = Point::new(source_rect.center().x, source_rect.y0);
                let rise = (40.0 + index as f64 * 24.0) * px;
                let spread = (28.0 + index as f64 * 10.0) * px;
                let c1 = Point::new(top.x - spread, top.y - rise);
                let c2 = Point::new(top.x + spread, top.y - rise);
                let start = Point::new(top.x - 8.0 * px, top.y);
                let end = Point::new(top.x + 8.0 * px, top.y);
                let mut path = BezPath::new();
                path.move_to(start);
                path.curve_to(c1, c2, end);
                let mid = Point::new(top.x, top.y - rise * 0.75);
                (path, mid, end, end - c2)
            } else {
                let a = source_rect.center();
                let b = target_rect.center();
                let axis = b - a;
                let normal = if axis.hypot() < 1e-6 {
                    Vec2::new(0.0, 1.0)
                } else {
                    Vec2::new(-axis.y, axis.x) / axis.hypot()
                };
                let control = a.midpoint(b) + normal * offset;
                let start = clip_to_rect(a, control, source_rect);
                let end = clip_to_rect(b, control, target_rect);
                let mut path = BezPath::new();
                path.move_to(start);
                path.quad_to(control, end);
                let mid = quadratic_point(start, control, end, 0.5);
                (path, mid, end, end - control)
            };
            let pill_content = content(doc, label, tcx, zoom);
            let (w, h) = (
                f64::from(pill_content.width()),
                f64::from(pill_content.height()),
            );
            let pill = Rect::from_center_size(
                mid,
                (
                    w + 2.0 * PILL_PADDING * px,
                    h + 2.0 * PILL_PADDING * px,
                ),
            );
            // Pills wash only as identity occurrences — a label
            // that IS the secondary identity (including the selected
            // edge's own value used as a label elsewhere).
            let pill_strength = if strength == Strength::Primary {
                Strength::Primary
            } else if secondary.as_ref() == Some(label) {
                Strength::Secondary
            } else {
                Strength::None
            };
            Some(EdgeView {
                source: source.clone(),
                label: label.clone(),
                path,
                arrow: arrowhead(tip, tip_direction, px),
                pill,
                pill_content,
                strength,
                pill_strength,
            })
        })
        .collect();

    let press_node = hooks.press_node.clone();
    let press_edge = hooks.press_edge.clone();
    let press_background = hooks.press_background.clone();
    let drag_to = hooks.drag_to.clone();
    let release = hooks.release.clone();
    let pick = hooks.pick.clone();
    let extent = Extent {
        width: panel.width(),
        ascent: 0.0,
        descent: panel.height(),
    };
    leaf(extent, move |p: &mut P, at: Point| {
        let panel = Rect::new(at.x, at.y, at.x + extent.width, at.y + extent.descent);
        // Everything the viewport shows stays inside the panel.
        p.clip(panel, Affine::IDENTITY, |p| {
            p.fill(panel, Color::new(PANEL_BG), Affine::IDENTITY);
            for edge in &edge_views {
                let (color, width) = match edge.strength {
                    Strength::Primary => (PRIMARY, 2.5),
                    _ => (EDGE, 1.2),
                };
                p.stroke(
                    edge.path.clone(),
                    Stroke::new(width * px),
                    Color::new(color),
                    Affine::IDENTITY,
                );
                p.stroke(
                    edge.arrow.clone(),
                    Stroke::new(width * px),
                    Color::new(color),
                    Affine::IDENTITY,
                );
                let pill = RoundedRect::from_rect(edge.pill, 4.0 * px);
                let (pill_color, pill_width) = match edge.pill_strength {
                    Strength::Primary => (PRIMARY, 2.5),
                    Strength::Secondary => (SECONDARY_OUTLINE, 1.5),
                    Strength::None => (color, 1.0),
                };
                p.fill(pill, Color::new(PILL_BG), Affine::IDENTITY);
                if edge.pill_strength == Strength::Secondary {
                    p.fill(pill, Color::new(WASH), Affine::IDENTITY);
                }
                p.stroke(
                    pill,
                    Stroke::new(pill_width * px),
                    Color::new(pill_color),
                    Affine::IDENTITY,
                );
                draw_content(p, &edge.pill_content, edge.pill);
            }
            for node in &node_views {
                let shape = RoundedRect::from_rect(node.rect, 5.0 * px);
                let fill = if node.root { ROOT_FILL } else { NODE_FILL };
                p.fill(shape, Color::new(fill), Affine::IDENTITY);
                if node.strength == Strength::Secondary {
                    p.fill(shape, Color::new(WASH), Affine::IDENTITY);
                }
                let (color, width) = match node.strength {
                    Strength::Primary => (PRIMARY, 2.5),
                    Strength::Secondary => (SECONDARY_OUTLINE, 1.5),
                    Strength::None => (BORDER, 1.2),
                };
                p.stroke(
                    shape,
                    Stroke::new(width * px),
                    Color::new(color),
                    Affine::IDENTITY,
                );
                draw_content(p, &node.content, node.rect);
            }
        });
        p.stroke(
            vello::kurbo::Line::new((panel.x0, panel.y0), (panel.x0, panel.y1)),
            Stroke::new(1.0 * scale),
            Color::new(SEPARATOR),
            Affine::IDENTITY,
        );

        // Hit-testing mirrors draw order back-to-front: pills over
        // nodes, nodes over background; the pane swallows everything
        // inside the panel so nothing lands on the document beneath.
        let node_hits: Vec<(Rect, Id)> = node_views
            .iter()
            .map(|node| (node.rect, node.id.clone()))
            .collect();
        let pill_hits: Vec<(Rect, Id, Id)> = edge_views
            .iter()
            .map(|edge| (edge.pill, edge.source.clone(), edge.label.clone()))
            .collect();
        let from_panel = move |window: Point| {
            (((window - panel.center()) / px) - pan).to_point()
        };
        let press_node = press_node.clone();
        let press_edge = press_edge.clone();
        let press_background = press_background.clone();
        let pick = pick.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            event.button == Some(PointerButton::Primary) && panel.contains(point) && {
                let picking = command(&event.state.modifiers);
                if let Some((_, source, label)) =
                    pill_hits.iter().find(|(rect, _, _)| rect.contains(point))
                {
                    if !(picking && pick(ctx, label.clone())) {
                        press_edge(ctx, source.clone(), label.clone());
                    }
                } else if let Some((rect, id)) =
                    node_hits.iter().find(|(rect, _)| rect.contains(point))
                {
                    if !(picking && pick(ctx, id.clone())) {
                        let world = from_panel(point);
                        let node_world = from_panel(rect.center());
                        press_node(ctx, id.clone(), world - node_world, point);
                    }
                } else {
                    press_background(ctx, point);
                }
                true
            }
        });
        let drag_to = drag_to.clone();
        p.handler().on_pointer_move(move |ctx, update| {
            let point = Point::new(update.current.position.x, update.current.position.y);
            update.current.buttons.contains(PointerButton::Primary)
                && drag_to(ctx, from_panel(point), point, px)
        });
        let release = release.clone();
        p.handler().on_pointer_up(move |ctx, _| release(ctx));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::{MutGid, new_node_id};

    fn doc() -> (Document, NodeId, NodeId) {
        let mut gid = MutGid::new();
        let a = new_node_id();
        let b = new_node_id();
        gid.set(a, Id::from("to"), Id::from(b));
        gid.set(a, Id::from("x"), Id::from(2.0));
        gid.set(b, Id::from("x"), Id::from(2.0));
        (
            Document {
                root: Some(Id::from(a)),
                gid,
            },
            a,
            b,
        )
    }

    #[test]
    fn snapshot_shares_atom_nodes() {
        let (doc, a, b) = doc();
        let snapshot = snapshot(&doc);
        // a, b, and ONE shared node for the number 2.
        assert_eq!(snapshot.nodes.len(), 3);
        assert_eq!(snapshot.edges.len(), 3);
        assert!(snapshot.nodes.contains(&Id::from(a)));
        assert!(snapshot.nodes.contains(&Id::from(b)));
        assert!(snapshot.nodes.contains(&Id::from(2.0)));
    }

    #[test]
    fn simulation_pulls_connected_nodes_toward_rest_length() {
        let (doc, a, b) = doc();
        let mut view = GraphView::default();
        for _ in 0..600 {
            view.step(&doc);
        }
        let distance =
            (view.positions[&Id::from(a)] - view.positions[&Id::from(b)]).hypot();
        assert!(
            distance > REST_LENGTH * 0.3 && distance < REST_LENGTH * 3.0,
            "settled at {distance}"
        );
    }

    #[test]
    fn deleting_a_node_detaches_it_everywhere() {
        let (mut doc, a, b) = doc();
        delete_selection(&mut doc, &GraphSelection::Node(Id::from(b)));
        // b's outgoing edge is gone and a no longer references it.
        assert!(doc.gid.edges(&Id::from(b)).is_none_or(|edges| edges.is_empty()));
        assert!(
            doc.gid
                .edges(&Id::from(a))
                .is_some_and(|edges| !edges.values().any(|value| *value == Id::from(b)))
        );
        // Deleting the root also empties the root slot.
        delete_selection(&mut doc, &GraphSelection::Node(Id::from(a)));
        assert!(doc.root.is_none());

        // Edge deletion removes exactly one edge.
        let (mut doc, a, _) = doc2();
        delete_selection(
            &mut doc,
            &GraphSelection::Edge {
                source: Id::from(a),
                label: Id::from("x"),
            },
        );
        assert!(doc.gid.get(&Id::from(a), &Id::from("x")).is_none());
        assert!(doc.gid.get(&Id::from(a), &Id::from("to")).is_some());
    }

    fn doc2() -> (Document, NodeId, NodeId) {
        doc()
    }
}
