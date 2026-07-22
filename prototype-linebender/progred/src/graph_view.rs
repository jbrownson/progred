//! The graph view: the document's REFERENCE TOPOLOGY, for demos on
//! small graphs. Nodes are cells (plus one synthetic node for a root
//! value that is not a link); an arrow means "this node's value
//! mentions that cell", deduplicated — which field or element holds
//! the link is the tree's business, so edges carry no labels and
//! cannot be selected. What the picture shows is what the tree
//! hides: cycles, sharing, floaters, and red links (valueless cells,
//! dashed). Layout is the force simulation carried from the
//! TypeScript/egui/Haskell prototypes (same constants), stepped
//! every frame while the view is open; positions and velocities are
//! explicit model state, seeded deterministically per node.
//! Rendering and hit-testing are one pure pass: build geometry from
//! state, draw it, register handlers over it.

use crate::conventions::Names;
use crate::raw::{Document, Selection, command, short_id};
use crate::sources::Sources;
use parley::style::GenericFamily;
use parley::{Layout, StyleProperty};
use progred_graph::{Atom, CellId, Value};
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

/// A drawn node: a cell, or the document's root value when the root
/// is not a link (a link root tints its cell instead).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum GraphNode {
    Root,
    Cell(CellId),
}

/// The graph's selection: nodes only — edges are anonymous mentions,
/// deduplicated, with nothing per-edge to select or delete.
#[derive(Clone, Copy, PartialEq)]
pub enum GraphSelection {
    Node(GraphNode),
}

/// An in-progress drag: a node being moved (with where the pointer
/// grabbed it, as a world offset from the node position), or the
/// background panning the viewport. Either way, an unmoved release is
/// a click, reported through [`Release`].
enum Drag {
    Node {
        node: GraphNode,
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
    positions: HashMap<GraphNode, Point>,
    velocities: HashMap<GraphNode, Vec2>,
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
    ClickNode(GraphNode),
    ClickBackground,
}

struct Snapshot {
    nodes: Vec<GraphNode>,
    /// Mentions, deduplicated per (from, to) pair.
    edges: Vec<(GraphNode, CellId)>,
}

/// Every cell a value links, labels included — a label is a mention
/// too.
fn links(value: &Value, out: &mut Vec<CellId>) {
    match value {
        Value::Atom(atom) => out.extend(atom.as_cell()),
        Value::List(elements) => {
            for element in elements.values() {
                links(element, out);
            }
        }
        Value::Record(fields) => {
            for (label, field) in fields {
                out.extend(label.as_cell());
                links(field, out);
            }
        }
    }
}

/// The document's reference topology: every cell its table or links
/// mention (valueless cells included — a link is a mention), each
/// value's mentions as deduplicated edges. Library facts enrich
/// display only; the snapshot — what the graph SHOWS — stays the
/// document's own.
fn snapshot(doc: &Document) -> Snapshot {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<(GraphNode, CellId)> = Vec::new();
    let mention = |from: GraphNode, value: &Value, edges: &mut Vec<(GraphNode, CellId)>| {
        let mut found = Vec::new();
        links(value, &mut found);
        found.sort();
        found.dedup();
        edges.extend(found.into_iter().map(|to| (from, to)));
    };
    let mut cells: Vec<CellId> = doc.cells.cells().copied().collect();
    cells.sort();
    for cell in &cells {
        nodes.push(GraphNode::Cell(*cell));
        if let Some(value) = doc.cells.value(*cell) {
            mention(GraphNode::Cell(*cell), value, &mut edges);
        }
    }
    match &doc.root {
        // A link root is no extra node: it tints its cell.
        Some(root) if root.as_cell().is_none() => {
            nodes.push(GraphNode::Root);
            mention(GraphNode::Root, root, &mut edges);
        }
        _ => {}
    }
    // Mentioned cells without a table entry: fully bare, drawn too.
    nodes.extend(
        edges
            .iter()
            .map(|(_, to)| *to)
            .chain(doc.root.as_ref().and_then(Value::as_cell))
            .map(GraphNode::Cell),
    );
    nodes.sort();
    nodes.dedup();
    Snapshot { nodes, edges }
}

/// FNV-1a for deterministic seeding.
fn id_hash(node: &GraphNode) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    let mut eat = |byte: u8| {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    };
    match node {
        GraphNode::Root => eat(0),
        GraphNode::Cell(cell) => {
            eat(1);
            for byte in cell.as_bytes() {
                eat(*byte);
            }
        }
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
            Some(Drag::Node { node, .. }) => Some(*node),
            _ => None,
        };
        for id in &snapshot.nodes {
            if dragged.as_ref() == Some(id) {
                continue;
            }
            let force = forces.get(id).copied().unwrap_or_default();
            let velocity = self.velocities.entry(*id).or_default();
            *velocity = (*velocity + force) * DAMPING;
            let position = self.positions.entry(*id).or_default();
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
                self.positions.insert(*id, position);
                self.velocities.insert(*id, Vec2::ZERO);
            }
        }
        let keep: std::collections::HashSet<&GraphNode> = snapshot.nodes.iter().collect();
        self.positions.retain(|id, _| keep.contains(id));
        self.velocities.retain(|id, _| keep.contains(id));
    }

    fn forces(&self, snapshot: &Snapshot) -> HashMap<GraphNode, Vec2> {
        let mut forces: HashMap<GraphNode, Vec2> =
            snapshot.nodes.iter().map(|id| (*id, Vec2::ZERO)).collect();
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
        for (source, to) in &snapshot.edges {
            let target = &GraphNode::Cell(*to);
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
    pub fn press_node(&mut self, node: GraphNode, grab: Vec2, pressed: Point) {
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
                    self.positions.insert(*node, target);
                    self.velocities.insert(*node, Vec2::ZERO);
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

/// The value with every link to `cell` removed: the field or element
/// holding the link drops, containers purge recursively, and a value
/// that IS the link strips to nothing. Labels keep referencing — a
/// dangling label is the tolerated class, like any stale mention.
fn strip(value: &Value, cell: CellId) -> Option<Value> {
    match value {
        Value::Atom(Atom::Cell(linked)) if *linked == cell => None,
        Value::Atom(_) => Some(value.clone()),
        Value::List(elements) => Some(Value::List(
            elements
                .iter()
                .filter_map(|(position, element)| {
                    strip(element, cell).map(|element| (position.clone(), element))
                })
                .collect(),
        )),
        Value::Record(fields) => Some(Value::Record(
            fields
                .iter()
                .filter_map(|(label, field)| {
                    strip(field, cell).map(|field| (label.clone(), field))
                })
                .collect(),
        )),
    }
}

/// The value a node stands for: a cell's link, or the root value.
pub fn node_value(doc: &Document, node: &GraphNode) -> Option<Value> {
    match node {
        GraphNode::Cell(cell) => Some(Value::from(*cell)),
        GraphNode::Root => doc.root.clone(),
    }
}

/// Deletes the selected node from the graph: a cell is fully
/// detached — its table entry (name and value) removed, the root
/// cleared if it is the root link, and every link to it anywhere
/// unlinked; a cell whose whole value was such a link keeps its name
/// and goes valueless. The root node empties the root. Unreferenced
/// cells simply stop appearing.
pub fn delete_selection(doc: &mut Document, selection: &GraphSelection) -> bool {
    let before = doc.cells.clone();
    let before_root = doc.root.clone();
    match selection {
        GraphSelection::Node(GraphNode::Root) => {
            doc.root = None;
        }
        GraphSelection::Node(GraphNode::Cell(cell)) => {
            doc.root = doc.root.take().and_then(|root| strip(&root, *cell));
            doc.cells.remove(*cell);
            let others: Vec<CellId> = doc.cells.cells().copied().collect();
            for other in others {
                let stripped = doc.cells.value(other).and_then(|value| {
                    let next = strip(value, *cell);
                    (next.as_ref() != Some(value)).then_some(next)
                });
                match stripped {
                    Some(Some(next)) => doc.cells.set_value(other, next),
                    Some(None) => doc.cells.clear_value(other),
                    None => {}
                }
            }
        }
    }
    !(doc.cells.ptr_eq(&before) && doc.root == before_root)
}

/// Dispatch-time callbacks the shell injects, mirroring `raw::Hooks`:
/// the pane reports what happened in world coordinates; the shell
/// owns the transitions.
pub struct Hooks<C> {
    pub press_node: Rc<dyn Fn(&mut C, GraphNode, Vec2, Point)>,
    pub press_background: Rc<dyn Fn(&mut C, Point)>,
    /// (world point, window point, panel pixels per world unit).
    pub drag_to: Rc<dyn Fn(&mut C, Point, Point, f64) -> bool>,
    pub release: Rc<dyn Fn(&mut C) -> bool>,
    /// Command-click: commit the pointed-at cell into the open
    /// pending; false when nothing is pending.
    pub pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    /// The pointer's resting claim inside the panel: the node under
    /// it, or `None` for the pane's own ground — either way the pane
    /// takes the pointer, so the tree beneath never lights.
    pub hover: Rc<dyn Fn(&mut C, Option<GraphNode>)>,
}

const FONT_SIZE: f32 = 10.0;
const NODE_PADDING: f64 = 7.0;
const NODE_MIN_HEIGHT: f64 = 24.0;
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
/// The hover tier's wash: the secondary at half voice.
const HOVER_WASH: [f32; 4] = [0.0, 0.48, 1.0, 0.05];
const TEXT: [f32; 4] = [0.13, 0.14, 0.16, 1.0];
const STRING_TEXT: [f32; 4] = [0.55, 0.33, 0.28, 1.0];
const DIM_TEXT: [f32; 4] = [0.55, 0.58, 0.64, 1.0];

struct NodeView {
    id: GraphNode,
    rect: Rect,
    content: Layout<Brush>,
    root: bool,
    external: bool,
    /// A cell holding no value: dashed border, the red-link look.
    bare: bool,
    strength: Strength,
}

#[derive(Clone, Copy, PartialEq)]
enum Strength {
    None,
    /// The pointer's claim, here or projected from the other pane —
    /// the secondary mark at half voice.
    Hover,
    Secondary,
    Primary,
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

/// What a node shows: cells speak the tree's paren syntax — `(name)`
/// or `(…id)`, the name through the editor's one display read — so
/// the unparenthesized root value node reads as the value it is.
fn node_content(
    sources: &Sources,
    names: &Names,
    raw: bool,
    doc: &Document,
    node: &GraphNode,
    tcx: &mut TextCtx,
    zoom: f64,
) -> Layout<Brush> {
    let size = FONT_SIZE * zoom as f32;
    let ui = GenericFamily::SystemUi;
    let mono = GenericFamily::Monospace;
    match node {
        GraphNode::Cell(cell) => {
            match crate::conventions::display_name(sources, names, raw, *cell) {
                Some(name) => layout_text(tcx, &format!("({name})"), size, TEXT, ui),
                None => {
                    layout_text(tcx, &format!("({})", short_id(*cell)), size, DIM_TEXT, mono)
                }
            }
        }
        GraphNode::Root => {
            let mark = match &doc.root {
                Some(Value::Record(_)) => "{…}".to_string(),
                Some(Value::List(elements)) if elements.is_empty() => "[ ]".to_string(),
                Some(Value::List(_)) => "[…]".to_string(),
                Some(Value::Atom(Atom::String(s))) => format!("\"{s}\""),
                Some(other) => other.to_string(),
                None => String::new(),
            };
            match &doc.root {
                Some(Value::Atom(Atom::String(_))) => {
                    layout_text(tcx, &mark, size, STRING_TEXT, ui)
                }
                _ => layout_text(tcx, &mark, size, DIM_TEXT, ui),
            }
        }
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
    hover: Option<&GraphNode>,
    doc_hover: Option<&crate::raw::Hover>,
    names: &Names,
    raw: bool,
    tcx: &mut TextCtx,
    panel: Rect,
    hooks: &Hooks<C>,
) -> Node<P> {
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
    // VALUE: the cell it links washes as a secondary.
    let doc_value = doc_selection.and_then(|selection| match selection {
        Selection::Edge { path, .. } => sources.resolve(path).cloned(),
        _ => None,
    });
    let secondary_cell = doc_value
        .or_else(|| match selection {
            Some(GraphSelection::Node(node)) => node_value(doc, node),
            _ => None,
        })
        .and_then(|value| value.as_cell());
    // The document's hover projects in the same way, at half voice.
    let hover_cell = doc_hover
        .and_then(|hover| crate::raw::hover_value(sources, hover))
        .and_then(|value| value.as_cell());

    let root_link = doc.root.as_ref().and_then(Value::as_cell);
    let node_views: Vec<NodeView> = snapshot
        .nodes
        .iter()
        .filter_map(|id| {
            let world = *view.positions.get(id)?;
            let content = node_content(sources, names, raw, doc, id, tcx, zoom);
            let (w, h) = (f64::from(content.width()), f64::from(content.height()));
            let width = w + 2.0 * NODE_PADDING * px;
            let height = (h + 2.0 * NODE_PADDING * px).max(NODE_MIN_HEIGHT * px);
            let at = to_panel(world);
            let strength = if matches!(selection, Some(GraphSelection::Node(n)) if n == id) {
                Strength::Primary
            } else if matches!(id, GraphNode::Cell(cell) if secondary_cell == Some(*cell)) {
                Strength::Secondary
            } else if hover == Some(id)
                || matches!(id, GraphNode::Cell(cell) if hover_cell == Some(*cell))
            {
                Strength::Hover
            } else {
                Strength::None
            };
            let (external, bare) = match id {
                GraphNode::Cell(cell) => (
                    sources.external(*cell),
                    sources.value(*cell).is_none(),
                ),
                GraphNode::Root => (false, false),
            };
            Some(NodeView {
                id: *id,
                rect: Rect::from_center_size(at, (width, height)),
                content,
                root: matches!(id, GraphNode::Root)
                    || matches!(id, GraphNode::Cell(cell) if root_link == Some(*cell)),
                external,
                bare,
                strength,
            })
        })
        .collect();
    let rects: HashMap<&GraphNode, Rect> = node_views
        .iter()
        .map(|node| (&node.id, node.rect))
        .collect();

    // The two directions between one pair arc to opposite sides;
    // offsets live in the canonical pair's frame because the normal
    // below flips with edge direction.
    let mut pair_counts: HashMap<(GraphNode, GraphNode), usize> = HashMap::new();
    let pair = |a: &GraphNode, b: &GraphNode| {
        if a <= b { (*a, *b) } else { (*b, *a) }
    };
    for (from, to) in &snapshot.edges {
        *pair_counts.entry(pair(from, &GraphNode::Cell(*to))).or_default() += 1;
    }
    let mut pair_seen: HashMap<(GraphNode, GraphNode), usize> = HashMap::new();

    let edge_views: Vec<(BezPath, BezPath)> = snapshot
        .edges
        .iter()
        .filter_map(|(from, to)| {
            let source_rect = *rects.get(from)?;
            let target = GraphNode::Cell(*to);
            let target_rect = *rects.get(&target)?;
            let key = pair(from, &target);
            let total = pair_counts[&key];
            let index = {
                let seen = pair_seen.entry(key).or_default();
                let index = *seen;
                *seen += 1;
                index
            };
            let aligned = (*from, target) == key;
            let offset = (index as f64 - (total as f64 - 1.0) / 2.0)
                * PARALLEL_SPACING
                * px
                * if aligned { 1.0 } else { -1.0 };
            let (path, tip, tip_direction) = if *from == target {
                // Self-loop: a cubic arch above the node.
                let top = Point::new(source_rect.center().x, source_rect.y0);
                let rise = 40.0 * px;
                let spread = 28.0 * px;
                let c1 = Point::new(top.x - spread, top.y - rise);
                let c2 = Point::new(top.x + spread, top.y - rise);
                let start = Point::new(top.x - 8.0 * px, top.y);
                let end = Point::new(top.x + 8.0 * px, top.y);
                let mut path = BezPath::new();
                path.move_to(start);
                path.curve_to(c1, c2, end);
                (path, end, end - c2)
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
                (path, end, end - control)
            };
            Some((path, arrowhead(tip, tip_direction, px)))
        })
        .collect();

    let press_node = hooks.press_node.clone();
    let press_background = hooks.press_background.clone();
    let drag_to = hooks.drag_to.clone();
    let release = hooks.release.clone();
    let pick = hooks.pick.clone();
    let hover_hook = hooks.hover.clone();
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
            for (path, arrow) in &edge_views {
                p.stroke(
                    path.clone(),
                    Stroke::new(1.2 * px),
                    Color::new(EDGE),
                    Affine::IDENTITY,
                );
                p.stroke(
                    arrow.clone(),
                    Stroke::new(1.2 * px),
                    Color::new(EDGE),
                    Affine::IDENTITY,
                );
            }
            for node in &node_views {
                let shape = RoundedRect::from_rect(node.rect, 5.0 * px);
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
                if node.strength == Strength::Hover {
                    p.fill(shape, Color::new(HOVER_WASH), Affine::IDENTITY);
                }
                let (color, width) = match node.strength {
                    Strength::Primary => (PRIMARY, 2.5),
                    Strength::Secondary => (SECONDARY_OUTLINE, 1.5),
                    Strength::Hover | Strength::None => (BORDER, 1.2),
                };
                let stroke = if node.bare {
                    Stroke::new(width * px).with_dashes(0.0, [4.0 * px, 3.0 * px])
                } else {
                    Stroke::new(width * px)
                };
                p.stroke(shape, stroke, Color::new(color), Affine::IDENTITY);
                draw_content(p, &node.content, node.rect);
            }
        });
        p.stroke(
            vello::kurbo::Line::new((panel.x0, panel.y0), (panel.x0, panel.y1)),
            Stroke::new(1.0 * scale),
            Color::new(SEPARATOR),
            Affine::IDENTITY,
        );

        // Hit-testing mirrors draw order back-to-front: nodes over
        // background; the pane swallows everything inside the panel
        // so nothing lands on the document beneath.
        let node_hits: Vec<(Rect, GraphNode)> = node_views
            .iter()
            .map(|node| (node.rect, node.id))
            .collect();
        let from_panel = move |window: Point| {
            (((window - panel.center()) / px) - pan).to_point()
        };
        // Moves inside the panel report the node under the pointer —
        // or the pane's own ground — and never consume the event, so
        // the drag handler registered after still sees every move.
        let hover = hover_hook.clone();
        let hover_hits = node_hits.clone();
        p.handler().on_pointer_move(move |ctx, update| {
            let point = Point::new(update.current.position.x, update.current.position.y);
            if panel.contains(point) {
                hover(
                    ctx,
                    hover_hits
                        .iter()
                        .find(|(rect, _)| rect.contains(point))
                        .map(|(_, id)| *id),
                );
            }
            false
        });
        let press_node = press_node.clone();
        let press_background = press_background.clone();
        let pick = pick.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            event.button == Some(PointerButton::Primary) && panel.contains(point) && {
                if let Some((rect, id)) =
                    node_hits.iter().find(|(rect, _)| rect.contains(point))
                {
                    let picked = command(&event.state.modifiers)
                        && match id {
                            GraphNode::Cell(cell) => pick(ctx, Value::from(*cell)),
                            GraphNode::Root => false,
                        };
                    if !picked {
                        let world = from_panel(point);
                        let node_world = from_panel(rect.center());
                        press_node(ctx, *id, world - node_world, point);
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
    use progred_graph::{Cells, Label, new_cell_id};

    fn doc() -> (Document, CellId, CellId) {
        let mut cells = Cells::new();
        let a = new_cell_id();
        let b = new_cell_id();
        cells.set_value(
            a,
            Value::record([
                (Label::from("to"), Value::from(b)),
                (Label::from("x"), Value::from("2")),
            ]),
        );
        cells.set_value(b, Value::record([(Label::from("x"), Value::from("2"))]));
        (
            Document {
                root: Some(Value::from(a)),
                cells,
            },
            a,
            b,
        )
    }

    #[test]
    fn snapshot_draws_cells_and_deduped_mentions() {
        let (mut doc, a, b) = doc();
        let snapshot = super::snapshot(&doc);
        // Cells only: a and b, one mention, atoms as content.
        assert_eq!(snapshot.nodes.len(), 2);
        assert!(snapshot.nodes.contains(&GraphNode::Cell(a)));
        assert!(snapshot.nodes.contains(&GraphNode::Cell(b)));
        assert_eq!(snapshot.edges, vec![(GraphNode::Cell(a), b)]);
        // A link root adds no synthetic node.
        assert!(!snapshot.nodes.contains(&GraphNode::Root));

        // Many links to one cell are ONE arrow; links inside lists
        // and inline records count; a valueless mention appears as a
        // node; a record root becomes the synthetic root node.
        let bare = new_cell_id();
        doc.cells.set_value(
            a,
            Value::record([
                (Label::from("to"), Value::from(b)),
                (
                    Label::from("points"),
                    Value::list([Value::from(b), Value::from(bare)]),
                ),
                (
                    Label::from("at"),
                    Value::record([(Label::from("of"), Value::from(b))]),
                ),
            ]),
        );
        doc.root = Some(Value::record([(Label::from("shape"), Value::from(a))]));
        let snapshot = super::snapshot(&doc);
        assert!(snapshot.nodes.contains(&GraphNode::Root));
        assert!(snapshot.nodes.contains(&GraphNode::Cell(bare)));
        assert_eq!(snapshot.nodes.len(), 4);
        assert_eq!(
            snapshot
                .edges
                .iter()
                .filter(|(from, to)| *from == GraphNode::Cell(a) && *to == b)
                .count(),
            1
        );
        assert!(snapshot.edges.contains(&(GraphNode::Cell(a), bare)));
        assert!(snapshot.edges.contains(&(GraphNode::Root, a)));
    }

    #[test]
    fn simulation_pulls_connected_nodes_toward_rest_length() {
        let (doc, a, b) = doc();
        let mut view = GraphView::default();
        for _ in 0..600 {
            view.step(&doc);
        }
        let distance = (view.positions[&GraphNode::Cell(a)]
            - view.positions[&GraphNode::Cell(b)])
            .hypot();
        assert!(
            distance > REST_LENGTH * 0.3 && distance < REST_LENGTH * 3.0,
            "settled at {distance}"
        );
    }

    #[test]
    fn deleting_a_cell_unlinks_it_everywhere() {
        let (mut doc, a, b) = doc();
        // b also linked from inside a list value and the root list.
        doc.cells.set_value(
            a,
            Value::record([
                (Label::from("to"), Value::from(b)),
                (
                    Label::from("refs"),
                    Value::list([Value::from(b), Value::from("1")]),
                ),
            ]),
        );
        doc.root = Some(Value::list([Value::from(a), Value::from(b)]));
        assert!(delete_selection(
            &mut doc,
            &GraphSelection::Node(GraphNode::Cell(b))
        ));
        // b's table entry is gone, a no longer links it, the list
        // occurrences dropped out with order preserved.
        assert!(doc.cells.entry(b).is_none());
        assert_eq!(
            doc.cells.value(a),
            Some(&Value::record([(
                Label::from("refs"),
                Value::list([Value::from("1")])
            )]))
        );
        assert_eq!(doc.root, Some(Value::list([Value::from(a)])));

        // A cell whose whole value was the link keeps its name and
        // goes valueless.
        let c = new_cell_id();
        doc.cells.set_name(c, "keeper");
        doc.cells.set_value(c, Value::from(a));
        doc.root = Some(Value::from(a));
        assert!(delete_selection(
            &mut doc,
            &GraphSelection::Node(GraphNode::Cell(a))
        ));
        assert!(doc.root.is_none());
        assert_eq!(doc.cells.name(c), Some("keeper"));
        assert!(doc.cells.value(c).is_none());

        // The root node empties the root.
        doc.root = Some(Value::record([]));
        assert!(delete_selection(
            &mut doc,
            &GraphSelection::Node(GraphNode::Root)
        ));
        assert!(doc.root.is_none());
    }

    #[test]
    fn release_reports_clicks_and_drags() {
        let mut view = GraphView::default();
        assert!(view.release().is_none());

        let node = GraphNode::Cell(new_cell_id());
        view.press_node(node, Vec2::ZERO, Point::ZERO);
        assert!(matches!(
            view.release(),
            Some(Release::ClickNode(n)) if n == node
        ));

        view.press_node(node, Vec2::ZERO, Point::ZERO);
        view.drag_to(Point::new(50.0, 0.0), Point::new(50.0, 0.0), 1.0);
        assert!(matches!(view.release(), Some(Release::Drag)));

        view.press_background(Point::ZERO);
        assert!(matches!(view.release(), Some(Release::ClickBackground)));
    }
}
