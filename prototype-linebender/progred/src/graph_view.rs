//! The graph view: the document as spatial nodes and edges, for
//! demos on small graphs. Every value that appears as an entity or at
//! the end of an edge is a node — atoms and lists included, keyed by
//! value equality, so a shared `2` (or two equal `[2, 3]`s) is
//! visibly one node — and every (entity, key, value) is a directed
//! edge with a label pill. Layout is the force simulation carried
//! from the TypeScript/egui/Haskell prototypes (same constants),
//! stepped every frame while the view is open; positions and
//! velocities are explicit model state, seeded deterministically per
//! value. Rendering and hit-testing are one pure pass: build geometry
//! from state, draw it, register handlers over it.

use crate::conventions::Names;
use crate::raw::{Document, Selection, command, short_id};
use crate::sources::Sources;
use parley::style::GenericFamily;
use parley::{Layout, StyleProperty};
use progred_graph::{Atom, Gid, NodeId, Value};
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
    Node(Value),
    Edge { source: NodeId, label: Atom },
}

/// An in-progress drag: a node being moved (with where the pointer
/// grabbed it, as a world offset from the node position), or the
/// background panning the viewport. Either way, an unmoved release is
/// a click, reported through [`Release`].
enum Drag {
    Node {
        node: Value,
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
/// velocities, the viewport (world-space pan, zoom), and any drag in
/// progress. Selection lives in the shell's one slot, not here.
pub struct GraphView {
    positions: HashMap<Value, Point>,
    velocities: HashMap<Value, Vec2>,
    drag: Option<Drag>,
    pan: Vec2,
    zoom: f64,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            positions: HashMap::new(),
            velocities: HashMap::new(),
            drag: None,
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

/// What a released press turned out to be: a drag that ends, or an
/// unmoved click — on a node or the background — for the shell to
/// turn into a selection change.
pub enum Release {
    Drag,
    ClickNode(Value),
    ClickBackground,
}

struct Snapshot {
    nodes: Vec<Value>,
    edges: Vec<(NodeId, Atom, Value)>,
}

/// Every value in the document: entities plus every edge value.
/// Keys appear on edge pills, not as nodes.
fn snapshot(doc: &Document) -> Snapshot {
    let mut nodes: Vec<Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |nodes: &mut Vec<Value>, value: &Value| {
        if seen.insert(value.clone()) {
            nodes.push(value.clone());
        }
    };
    let mut edges = Vec::new();
    let mut entities: Vec<NodeId> = doc.gid.entities().copied().collect();
    entities.sort();
    for entity in entities {
        add(&mut nodes, &Value::from(entity));
        let mut outgoing: Vec<(Atom, Value)> = doc
            .gid
            .edges(entity)
            .map(|edges| edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        outgoing.sort();
        for (key, value) in outgoing {
            add(&mut nodes, &value);
            edges.push((entity, key, value));
        }
    }
    if let Some(root) = &doc.root {
        add(&mut nodes, root);
    }
    Snapshot { nodes, edges }
}

/// FNV-1a over the value's structure, for deterministic seeding.
/// Positions are ignored, like every other reading of a list.
fn id_hash(value: &Value) -> u32 {
    fn eat(hash: &mut u32, byte: u8) {
        *hash = (*hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    fn walk(hash: &mut u32, value: &Value) {
        match value {
            Value::Atom(Atom::Node(node)) => {
                eat(hash, 0);
                for byte in node.as_bytes() {
                    eat(hash, *byte);
                }
            }
            Value::Atom(Atom::String(s)) => {
                eat(hash, 1);
                for byte in s.as_bytes() {
                    eat(hash, *byte);
                }
            }
            Value::Atom(Atom::Number(n)) => {
                eat(hash, 2);
                for byte in n.get().to_le_bytes() {
                    eat(hash, byte);
                }
            }
            Value::List(elements) => {
                eat(hash, 3);
                for element in elements.values() {
                    walk(hash, element);
                }
            }
        }
    }
    let mut hash: u32 = 2_166_136_261;
    walk(&mut hash, value);
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
        let keep: std::collections::HashSet<&Value> = snapshot.nodes.iter().collect();
        self.positions.retain(|id, _| keep.contains(id));
        self.velocities.retain(|id, _| keep.contains(id));
    }

    fn forces(&self, snapshot: &Snapshot) -> HashMap<Value, Vec2> {
        let mut forces: HashMap<Value, Vec2> =
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
            let source = Value::from(*source);
            let delta = self.positions[target] - self.positions[&source];
            let distance = delta.hypot().max(0.1);
            let magnitude =
                (ATTRACTION_K * (distance - REST_LENGTH)).clamp(-MAX_FORCE, MAX_FORCE);
            let force = unit(delta) * magnitude;
            *forces.get_mut(&source).unwrap() += force;
            *forces.get_mut(target).unwrap() -= force;
        }
        for id in &snapshot.nodes {
            *forces.get_mut(id).unwrap() += self.positions[id].to_vec2() * -GRAVITY_K;
        }
        forces
    }

    /// `grab` is the world offset from the node position to the
    /// pointer; `pressed` is in panel pixels, for the click slop.
    pub fn press_node(&mut self, node: Value, grab: Vec2, pressed: Point) {
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

    /// Ends a press, reporting what it was; `None` when no press was
    /// in progress.
    pub fn release(&mut self) -> Option<Release> {
        Some(match self.drag.take()? {
            Drag::Node { node, moved, .. } => {
                if moved {
                    Release::Drag
                } else {
                    Release::ClickNode(node)
                }
            }
            Drag::Pan { moved, .. } => {
                if moved {
                    Release::Drag
                } else {
                    Release::ClickBackground
                }
            }
        })
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

}

/// The value with every occurrence of `target` removed: a match
/// removes the value itself (None), a list drops matching elements
/// and purges the rest recursively — lists are values, so detaching
/// reaches inside them.
fn without(value: &Value, target: &Value) -> Option<Value> {
    if value == target {
        return None;
    }
    match value {
        Value::Atom(_) => Some(value.clone()),
        Value::List(elements) => Some(Value::List(
            elements
                .iter()
                .filter_map(|(position, element)| {
                    without(element, target).map(|element| (position.clone(), element))
                })
                .collect(),
        )),
    }
}

/// Deletes the selection from the graph: an edge is one detachment; a
/// node is fully detached — the root cleared if it is the root, every
/// outgoing edge removed, and every occurrence anywhere as or inside
/// an edge value removed. Unreferenced values simply stop appearing.
pub fn delete_selection(doc: &mut Document, selection: &GraphSelection) -> bool {
    let before = doc.gid.clone();
    let before_root = doc.root.clone();
    match selection {
        GraphSelection::Edge { source, label } => {
            doc.gid.delete(*source, label);
        }
        GraphSelection::Node(id) => {
            doc.root = doc.root.take().and_then(|root| without(&root, id));
            if let Some(entity) = id.as_node() {
                let outgoing: Vec<Atom> = doc
                    .gid
                    .edges(entity)
                    .map(|edges| edges.keys().cloned().collect())
                    .unwrap_or_default();
                for key in outgoing {
                    doc.gid.delete(entity, &key);
                }
            }
            let entities: Vec<NodeId> = doc.gid.entities().copied().collect();
            for entity in entities {
                let touched: Vec<(Atom, Option<Value>)> = doc
                    .gid
                    .edges(entity)
                    .into_iter()
                    .flatten()
                    .filter_map(|(key, value)| {
                        let next = without(value, id);
                        (next.as_ref() != Some(value)).then(|| (key.clone(), next))
                    })
                    .collect();
                for (key, next) in touched {
                    match next {
                        Some(next) => doc.gid.set(entity, key, next),
                        None => doc.gid.delete(entity, &key),
                    }
                }
            }
        }
    }
    !(doc.gid.ptr_eq(&before) && doc.root == before_root)
}

/// Dispatch-time callbacks the shell injects, mirroring `raw::Hooks`:
/// the pane reports what happened in world coordinates; the shell
/// owns the transitions.
pub struct Hooks<C> {
    pub press_node: Rc<dyn Fn(&mut C, Value, Vec2, Point)>,
    pub press_edge: Rc<dyn Fn(&mut C, NodeId, Atom)>,
    pub press_background: Rc<dyn Fn(&mut C, Point)>,
    /// (world point, window point, panel pixels per world unit).
    pub drag_to: Rc<dyn Fn(&mut C, Point, Point, f64) -> bool>,
    pub release: Rc<dyn Fn(&mut C) -> bool>,
    /// Command-click: commit the pointed-at value into the open
    /// pending; false when nothing is pending.
    pub pick: Rc<dyn Fn(&mut C, Value) -> bool>,
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
/// External (library-authority) identities sit on a subtly darker
/// ground, as in the tree.
const EXTERNAL_FILL: [f32; 4] = [0.929, 0.933, 0.945, 1.0];
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
    id: Value,
    rect: Rect,
    content: Layout<Brush>,
    root: bool,
    external: bool,
    /// Lists draw square-cornered; kind is data, worth a silhouette.
    list: bool,
    strength: Strength,
}

#[derive(Clone, Copy, PartialEq)]
enum Strength {
    None,
    Secondary,
    Primary,
}

struct EdgeView {
    source: NodeId,
    label: Atom,
    path: BezPath,
    arrow: BezPath,
    pill: Rect,
    pill_content: Layout<Brush>,
    /// The label is a library-authority identity: darker pill ground.
    external: bool,
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

/// What a value looks like in the graph: named nodes show their name
/// (through the editor's one name policy, derived from the raw bit),
/// unnamed ones their short id, atoms their value, lists their inline
/// literal — the same identity language as the tree view.
fn content(
    sources: &Sources,
    names: &Names,
    raw: bool,
    value: &Value,
    tcx: &mut TextCtx,
    zoom: f64,
) -> Layout<Brush> {
    let size = FONT_SIZE * zoom as f32;
    let ui = GenericFamily::SystemUi;
    let mono = GenericFamily::Monospace;
    match value {
        Value::Atom(Atom::String(s)) => {
            layout_text(tcx, &format!("\"{s}\""), size, STRING_TEXT, ui)
        }
        Value::Atom(Atom::Number(n)) => {
            layout_text(tcx, &n.get().to_string(), size, NUMBER_TEXT, ui)
        }
        Value::Atom(Atom::Node(node)) => {
            match (!raw).then(|| names.of(sources, *node)).flatten() {
                Some(name) => layout_text(tcx, &name.text, size, TEXT, ui),
                None => layout_text(tcx, &short_id(*node), size, DIM_TEXT, mono),
            }
        }
        Value::List(_) => layout_text(tcx, &value.to_string(), size, DIM_TEXT, ui),
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
    if delta.x.abs() > 1e-6 {
        t = t.min((rect.width() / 2.0) / delta.x.abs());
    }
    if delta.y.abs() > 1e-6 {
        t = t.min((rect.height() / 2.0) / delta.y.abs());
    }
    from + delta * t
}

fn quadratic_point(a: Point, control: Point, b: Point, t: f64) -> Point {
    let u = 1.0 - t;
    (a.to_vec2() * (u * u) + control.to_vec2() * (2.0 * u * t) + b.to_vec2() * (t * t))
        .to_point()
}

fn arrowhead(tip: Point, direction: Vec2, px: f64) -> BezPath {
    let length = direction.hypot();
    let dir = if length < 1e-6 {
        Vec2::new(1.0, 0.0)
    } else {
        direction / length
    };
    let normal = Vec2::new(-dir.y, dir.x);
    let base = tip - dir * (ARROW_LENGTH * px);
    let mut path = BezPath::new();
    path.move_to(base + normal * (ARROW_WIDTH * px));
    path.line_to(tip);
    path.line_to(base - normal * (ARROW_WIDTH * px));
    path
}

/// One pure pass: geometry from state, drawing, handlers. The pane
/// reports presses/drags/releases through hooks; the shell owns every
/// transition. `doc_selection` mirrors the document selection in as a
/// secondary mark; `selection` is the graph's own.
#[allow(clippy::too_many_arguments)]
pub fn pane<C: 'static, P: Canvas + HasHandler<C>>(
    sources: &Sources,
    view: &GraphView,
    selection: Option<&GraphSelection>,
    doc_selection: Option<&Selection>,
    names: &Names,
    raw: bool,
    tcx: &mut TextCtx,
    panel: Rect,
    hooks: &Hooks<C>,
) -> Node<P> {
    // Library facts read through for display only; the snapshot —
    // what the graph SHOWS — stays the document's own.
    let doc = sources.doc;
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
        Selection::Edge { path, .. } => sources.resolve(path).cloned(),
        _ => None,
    });
    // The secondary identity, as in the tree: the selected edge's
    // value, or the graph's own selected node — every projection of
    // it marks, label pills included.
    let secondary = doc_value.or_else(|| match selection {
        Some(GraphSelection::Node(id)) => Some(id.clone()),
        _ => None,
    });

    let node_views: Vec<NodeView> = snapshot
        .nodes
        .iter()
        .filter_map(|id| {
            let world = *view.positions.get(id)?;
            let content = content(sources, names, raw, id, tcx, zoom);
            let (w, h) = (f64::from(content.width()), f64::from(content.height()));
            let width = w + 2.0 * NODE_PADDING * px;
            let height = (h + 2.0 * NODE_PADDING * px).max(NODE_MIN_HEIGHT * px);
            let at = to_panel(world);
            let strength = if matches!(selection, Some(GraphSelection::Node(n)) if n == id) {
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
                external: id.as_node().is_some_and(|node| sources.external(node)),
                list: matches!(id, Value::List(_)),
                strength,
            })
        })
        .collect();
    let rects: HashMap<&Value, Rect> = node_views
        .iter()
        .map(|node| (&node.id, node.rect))
        .collect();
    let rect_of = |id: &Value| rects.get(id).copied();

    // Parallel edges between one pair fan out by index; direction
    // folded so a->b and b->a share the count.
    let mut pair_counts: HashMap<(Value, Value), usize> = HashMap::new();
    let pair = |a: &Value, b: &Value| {
        if a <= b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        }
    };
    for (source, _, target) in &snapshot.edges {
        *pair_counts.entry(pair(&Value::from(*source), target)).or_default() += 1;
    }
    let mut pair_seen: HashMap<(Value, Value), usize> = HashMap::new();

    let edge_views: Vec<EdgeView> = snapshot
        .edges
        .iter()
        .filter_map(|(source, label, target)| {
            let source_value = Value::from(*source);
            let source_rect = rect_of(&source_value)?;
            let target_rect = rect_of(target)?;
            let key = pair(&source_value, target);
            let total = pair_counts[&key];
            let index = {
                let seen = pair_seen.entry(key).or_default();
                let index = *seen;
                *seen += 1;
                index
            };
            let offset =
                (index as f64 - (total as f64 - 1.0) / 2.0) * PARALLEL_SPACING * px;
            let strength = if matches!(
                selection,
                Some(GraphSelection::Edge { source: s, label: l }) if s == source && l == label
            ) {
                Strength::Primary
            } else {
                Strength::None
            };
            let (path, mid, tip, tip_direction) = if source_value == *target {
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
            let pill_content =
                content(sources, names, raw, &Value::Atom(label.clone()), tcx, zoom);
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
            } else if secondary.as_ref() == Some(&Value::Atom(label.clone())) {
                Strength::Secondary
            } else {
                Strength::None
            };
            Some(EdgeView {
                source: *source,
                label: label.clone(),
                path,
                arrow: arrowhead(tip, tip_direction, px),
                pill,
                pill_content,
                external: label
                    .as_node()
                    .is_some_and(|node| sources.external(node)),
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
                let pill_bg = if edge.external { EXTERNAL_FILL } else { PILL_BG };
                p.fill(pill, Color::new(pill_bg), Affine::IDENTITY);
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
                let radius = if node.list { 0.0 } else { 5.0 };
                let shape = RoundedRect::from_rect(node.rect, radius * px);
                let fill = if node.root {
                    ROOT_FILL
                } else if node.external {
                    EXTERNAL_FILL
                } else {
                    NODE_FILL
                };
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
        let node_hits: Vec<(Rect, Value)> = node_views
            .iter()
            .map(|node| (node.rect, node.id.clone()))
            .collect();
        let pill_hits: Vec<(Rect, NodeId, Atom)> = edge_views
            .iter()
            .map(|edge| (edge.pill, edge.source, edge.label.clone()))
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
                    if !(picking && pick(ctx, Value::Atom(label.clone()))) {
                        press_edge(ctx, *source, label.clone());
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
        gid.set(a, Atom::from("to"), Value::from(b));
        gid.set(a, Atom::from("x"), Value::from(2.0));
        gid.set(b, Atom::from("x"), Value::from(2.0));
        (
            Document {
                root: Some(Value::from(a)),
                gid,
            },
            a,
            b,
        )
    }

    #[test]
    fn snapshot_shares_value_nodes_lists_included() {
        let (mut doc, a, b) = doc();
        let snapshot = super::snapshot(&doc);
        // a, b, and ONE shared node for the number 2.
        assert_eq!(snapshot.nodes.len(), 3);
        assert_eq!(snapshot.edges.len(), 3);
        assert!(snapshot.nodes.contains(&Value::from(a)));
        assert!(snapshot.nodes.contains(&Value::from(b)));
        assert!(snapshot.nodes.contains(&Value::from(2.0)));

        // Equal list values are one node too — value semantics
        // displayed honestly.
        doc.gid.set(a, Atom::from("p"), Value::list([Value::from(1.0)]));
        doc.gid.set(b, Atom::from("q"), Value::list([Value::from(1.0)]));
        let snapshot = super::snapshot(&doc);
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .filter(|node| matches!(node, Value::List(_)))
                .count(),
            1
        );
    }

    #[test]
    fn simulation_pulls_connected_nodes_toward_rest_length() {
        let (doc, a, b) = doc();
        let mut view = GraphView::default();
        for _ in 0..600 {
            view.step(&doc);
        }
        let distance =
            (view.positions[&Value::from(a)] - view.positions[&Value::from(b)]).hypot();
        assert!(
            distance > REST_LENGTH * 0.3 && distance < REST_LENGTH * 3.0,
            "settled at {distance}"
        );
    }

    #[test]
    fn deleting_a_node_detaches_it_everywhere_lists_included() {
        let (mut doc, a, b) = doc();
        // b also referenced from inside a list value and the root list.
        doc.gid
            .set(a, Atom::from("refs"), Value::list([Value::from(b), Value::from(1.0)]));
        doc.root = Some(Value::list([Value::from(a), Value::from(b)]));
        delete_selection(&mut doc, &GraphSelection::Node(Value::from(b)));
        // b's outgoing edges are gone, a no longer references it, and
        // the list occurrences dropped out with order preserved.
        assert!(doc.gid.edges(b).is_none_or(|edges| edges.is_empty()));
        assert!(
            doc.gid
                .edges(a)
                .is_some_and(|edges| !edges.values().any(|value| *value == Value::from(b)))
        );
        assert_eq!(
            doc.gid.get(a, &Atom::from("refs")),
            Some(&Value::list([Value::from(1.0)]))
        );
        assert_eq!(doc.root, Some(Value::list([Value::from(a)])));

        // Deleting the root also empties the root slot.
        doc.root = Some(Value::from(a));
        delete_selection(&mut doc, &GraphSelection::Node(Value::from(a)));
        assert!(doc.root.is_none());
    }

    #[test]
    fn deleting_an_edge_removes_exactly_one() {
        let (mut doc, a, _) = doc();
        assert!(delete_selection(
            &mut doc,
            &GraphSelection::Edge {
                source: a,
                label: Atom::from("x"),
            },
        ));
        assert!(doc.gid.get(a, &Atom::from("x")).is_none());
        assert!(doc.gid.get(a, &Atom::from("to")).is_some());

        // A stale selection — the edge already gone — is a no-op, not
        // a recorded change.
        assert!(!delete_selection(
            &mut doc,
            &GraphSelection::Edge {
                source: a,
                label: Atom::from("x"),
            },
        ));
    }

    #[test]
    fn release_reports_clicks_and_drags() {
        let mut view = GraphView::default();
        assert!(view.release().is_none());

        view.press_node(Value::from(1.0), Vec2::ZERO, Point::ZERO);
        assert!(matches!(
            view.release(),
            Some(Release::ClickNode(id)) if id == Value::from(1.0)
        ));

        view.press_node(Value::from(1.0), Vec2::ZERO, Point::ZERO);
        view.drag_to(Point::new(50.0, 0.0), Point::new(50.0, 0.0), 1.0);
        assert!(matches!(view.release(), Some(Release::Drag)));

        view.press_background(Point::ZERO);
        assert!(matches!(view.release(), Some(Release::ClickBackground)));
    }
}
