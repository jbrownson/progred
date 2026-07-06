//! The raw projection: any gid document rendered as entity blocks of
//! edge rows, with no schema and no interpretation of semantic
//! conventions — every node is just its identicon, every edge a row,
//! including `name`. Only the structural cons-list shape is sugared
//! (`[a, b, c]`); SID labels render plain, GUID labels and values get
//! identicons, unparsable ids render as what they are.

use crate::conventions::{EMPTY, HEAD, NAME, TAIL};
use crate::identicon::{label_identicon, node_identicon};
use progred_graph::{Gid, Id, MutGid, NodeId, new_node_id};
use puri::draw::Canvas;
use puri::handler::HasHandler;
use puri::layout::{Extent, HAlign, Node, col, decorate, leaf, pad, row};
use puri::text::{TextCtx, TextStyle, text};
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, BezPath, Insets, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Brush, Color};

pub struct RawStyles {
    pub label: TextStyle,
    pub string: TextStyle,
    pub number: TextStyle,
    pub dim: TextStyle,
    pub scale: f64,
}

impl RawStyles {
    pub fn new(scale: f64) -> Self {
        let style = |size: f32, color: [f32; 4], weight: Option<f32>| TextStyle {
            size,
            brush: Brush::from(Color::new(color)),
            weight,
        };
        // A light, native-feeling palette: near-black primary labels,
        // gray secondary labels, restrained literal accents.
        Self {
            label: style(14.0, [0.46, 0.49, 0.55, 1.0], None),
            string: style(14.0, [0.55, 0.33, 0.28, 1.0], None),
            number: style(14.0, [0.16, 0.40, 0.62, 1.0], None),
            dim: style(13.0, [0.55, 0.58, 0.64, 1.0], None),
            scale,
        }
    }
}

/// A rooted graph: the document is its `root` id plus the `gid` that
/// stores its edges. Every projection path starts at `root`; several
/// top-level items are just a root that is an in-graph list.
pub struct Document {
    pub root: Id,
    pub gid: MutGid,
}

/// Builds a cons list from `items`, returning its head (or `EMPTY`).
fn cons_list(gid: &mut MutGid, items: Vec<Id>) -> Id {
    items.into_iter().rev().fold(Id::from(EMPTY), |tail, item| {
        let cell = new_node_id();
        gid.set(cell, Id::from(HEAD), item);
        gid.set(cell, Id::from(TAIL), tail);
        Id::from(cell)
    })
}

/// A small document exercising the model's range: named nodes, SID and
/// GUID labels, cons lists, an unnamed scratch node, and a value from
/// an unknown space. Its root is a list of the top-level entities.
pub fn sample_document() -> Document {
    let mut gid = MutGid::new();
    let name = Id::from(NAME);

    let origin = new_node_id();
    gid.set(origin, name.clone(), Id::from("origin"));
    gid.set(origin, Id::from("x"), Id::from(0.0));
    gid.set(origin, Id::from("y"), Id::from(0.0));

    let corner = new_node_id();
    gid.set(corner, name.clone(), Id::from("corner"));
    gid.set(corner, Id::from("x"), Id::from(4.0));
    gid.set(corner, Id::from("y"), Id::from(2.5));

    let cell2 = new_node_id();
    gid.set(cell2, Id::from(HEAD), Id::from(corner));
    gid.set(cell2, Id::from(TAIL), Id::from(EMPTY));
    let cell1 = new_node_id();
    gid.set(cell1, Id::from(HEAD), Id::from(origin));
    gid.set(cell1, Id::from(TAIL), Id::from(cell2));

    let stroke_width = new_node_id();
    gid.set(stroke_width, name.clone(), Id::from("stroke-width"));

    let polygon = new_node_id();
    gid.set(polygon, name.clone(), Id::from("polygon"));
    gid.set(polygon, Id::from("points"), Id::from(cell1));
    gid.set(polygon, Id::from(stroke_width), Id::from(1.5));
    let dash = cons_list(&mut gid, vec![Id::from(2.0), Id::from(3.0)]);
    gid.set(polygon, Id::from("dash"), dash);

    let scratch = new_node_id();
    gid.set(scratch, Id::from("color"), Id::from("rebeccapurple"));
    gid.set(
        scratch,
        Id::from("mystery"),
        Id::in_space(new_node_id(), vec![0xde, 0xad, 0xbe, 0xef]),
    );
    // A self-reference: exercises cycle-collapse, which renders the
    // back-edge as a collapsed header rather than recursing forever.
    gid.set(scratch, Id::from("self"), Id::from(scratch));

    let root = cons_list(
        &mut gid,
        vec![
            Id::from(polygon),
            Id::from(origin),
            Id::from(corner),
            Id::from(stroke_width),
            Id::from(scratch),
        ],
    );
    Document { root, gid }
}

/// Per-path collapse overrides. An absent entry means "use the
/// default", which is collapsed inside a cycle and expanded otherwise;
/// a present entry forces it the other way. Sparse: only overrides are
/// stored.
#[derive(Default)]
pub struct Collapse {
    overrides: std::collections::HashMap<Vec<Id>, bool>,
}

impl Collapse {
    fn collapsed(&self, path: &[Id], in_cycle: bool) -> bool {
        self.overrides.get(path).copied().unwrap_or(in_cycle)
    }
}

/// Read-only projection context threaded through every view.
struct Cx<'a> {
    gid: &'a MutGid,
    collapse: &'a Collapse,
    styles: &'a RawStyles,
    selection: Option<&'a Selection>,
}

impl Cx<'_> {
    fn selected(&self, path: &[Id]) -> bool {
        self.selection.map(Selection::path) == Some(path)
    }
}

/// A location in the projected spanning tree: the sequence of edge
/// labels from the root. The same node or edge can be projected at
/// several paths, so the path — not the id — is the stable identity a
/// selection names.
pub type Path = Vec<Id>;

/// What is selected. An edge (the value at a path) for now; splice
/// selections (the gaps between items) arrive with insertion points.
#[derive(PartialEq)]
pub enum Selection {
    Edge(Path),
}

impl Selection {
    pub fn path(&self) -> &[Id] {
        match self {
            Selection::Edge(path) => path,
        }
    }
}

/// A projected value's settled position: the path it stands for and the
/// rect it occupied, collected fresh every frame. Keyboard navigation
/// will read this to step selection by geometry and hierarchy relative
/// to the current one; clicks go through each descend's own handler,
/// not this list.
#[allow(dead_code)] // read once keyboard navigation lands (next step)
pub struct Descend {
    pub path: Path,
    pub rect: Rect,
}

/// Placement contexts that accumulate descends as the projection
/// places, so the shell can step selection by keyboard.
pub trait HasDescends {
    fn descends(&mut self) -> &mut Vec<Descend>;
}

/// Marks `child` as the projection of the edge at `path`. On placement
/// it draws the highlight when this is the selected edge, registers a
/// click that selects the edge (innermost wins by handler precedence),
/// and records its rect so keyboard navigation can find it by geometry.
fn descend<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    cx: &Cx,
    path: Path,
    select: &Rc<dyn Fn(&mut C, Selection)>,
    child: Node<P>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let selected = cx.selected(&path);
    let select = select.clone();
    decorate(child, move |p, rect| {
        if selected {
            let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
            // Translucent system blue, like the Swift version's selection.
            p.fill(bg, Color::new([0.0, 0.48, 1.0, 0.22]), Affine::IDENTITY);
        }
        let select = select.clone();
        let target = path.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    select(ctx, Selection::Edge(target.clone()));
                    true
                }
        });
        p.descends().push(Descend { path, rect });
    })
}

pub fn project<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    doc: &Document,
    selection: Option<&Selection>,
    collapse: &Collapse,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    on_select: impl Fn(&mut C, Selection) + 'static,
) -> Node<P> {
    let select: Rc<dyn Fn(&mut C, Selection)> = Rc::new(on_select);
    let cx = Cx {
        gid: &doc.gid,
        collapse,
        styles,
        selection,
    };
    // Raw shows the pure graph with no assumptions: the root is
    // projected directly, so a cons-list root renders as its cells, not
    // as a list. List sugar belongs to a convention-aware projection.
    value_view::<C, P>(&cx, tcx, &[], &HashSet::new(), &doc.root, &select)
}

/// A node rendered as a block: its identicon/name header over its
/// edges, indented and recursively projected. Collapsed — by default a
/// cycle, or forced by the collapse trie — it shows only the header,
/// marked with an ellipsis when it hides edges.
fn node_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    node: NodeId,
    select: &Rc<dyn Fn(&mut C, Selection)>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let id = Id::from(node);
    let edges = sorted_edges(cx.gid, &id);
    let header = node_identicon(node, 18.0 * scale);

    if edges.is_empty() {
        return header;
    }
    if cx.collapse.collapsed(path, ancestors.contains(&id)) {
        return row(6.0 * scale, vec![header, text(tcx, "…", &cx.styles.dim)]);
    }

    let mut inner = ancestors.clone();
    inner.insert(id);
    let rows = edges
        .into_iter()
        .map(|(label, value)| {
            let mut child = path.to_vec();
            child.push(label.clone());
            let label = label_view(cx, tcx, &label);
            let value = value_view(cx, tcx, &child, &inner, &value, select);
            row(6.0 * scale, vec![label, arrow(cx.styles), value])
        })
        .collect();
    col(
        HAlign::Start,
        0,
        4.0 * scale,
        vec![
            header,
            pad(
                Insets::new(26.0 * scale, 0.0, 0.0, 0.0),
                col(HAlign::Start, 0, 4.0 * scale, rows),
            ),
        ],
    )
}

/// A node's edges, sorted for stable order. All of them — `name` is not
/// special here.
fn sorted_edges(gid: &MutGid, id: &Id) -> Vec<(Id, Id)> {
    let mut edges: Vec<(Id, Id)> = gid
        .edges(id)
        .map(|edges| edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    edges.sort();
    edges
}

fn label_view<P: Canvas>(cx: &Cx, tcx: &mut TextCtx, label: &Id) -> Node<P> {
    if let Some(s) = label.as_str() {
        text(tcx, s, &cx.styles.label)
    } else if let Some(uuid) = label.as_node_id() {
        label_identicon(uuid, 14.0 * cx.styles.scale)
    } else {
        unknown_view(label, tcx, cx.styles)
    }
}

/// A small drawn arrow between a label and its value. Reading
/// "label → value", and being a stroke rather than an identicon, it
/// separates an edge's key from its target — labels and values are
/// otherwise both identicons.
fn arrow<P: Canvas>(styles: &RawStyles) -> Node<P> {
    let scale = styles.scale;
    let width = 16.0 * scale;
    let extent = Extent {
        width,
        ascent: 11.0 * scale,
        descent: 3.0 * scale,
    };
    leaf(extent, move |p: &mut P, at| {
        let y = at.y - 4.0 * scale;
        let x0 = at.x + 2.0 * scale;
        let x1 = at.x + width - 2.0 * scale;
        let head = 3.5 * scale;
        let mut path = BezPath::new();
        path.move_to((x0, y));
        path.line_to((x1, y));
        path.move_to((x1 - head, y - head));
        path.line_to((x1, y));
        path.line_to((x1 - head, y + head));
        p.stroke(
            path,
            Stroke::new(1.4 * scale),
            Color::new([0.58, 0.61, 0.67, 1.0]),
            Affine::IDENTITY,
        );
    })
}

fn value_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    value: &Id,
    select: &Rc<dyn Fn(&mut C, Selection)>,
) -> Node<P> {
    let inner = if let Some(s) = value.as_str() {
        text(tcx, &format!("\"{s}\""), &cx.styles.string)
    } else if let Some(n) = value.as_number() {
        text(tcx, &n.to_string(), &cx.styles.number)
    } else if let Some(node) = value.as_node_id() {
        node_view(cx, tcx, path, ancestors, node, select)
    } else {
        unknown_view(value, tcx, cx.styles)
    };
    descend(cx, path.to_vec(), select, inner)
}

/// A value from a space the editor doesn't know: the space's identicon
/// plus the payload as hex. Raw UUIDs are never shown; identicons are
/// their visual form.
fn unknown_view<P: Canvas>(id: &Id, tcx: &mut TextCtx, styles: &RawStyles) -> Node<P> {
    let hex: String = id.payload().iter().map(|b| format!("{b:02x}")).collect();
    row(
        4.0 * styles.scale,
        vec![
            label_identicon(id.space(), 14.0 * styles.scale),
            text(tcx, &hex, &styles.dim),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_default_follows_cycle_and_overrides_win() {
        let mut collapse = Collapse::default();
        let path = vec![Id::from("a"), Id::from("b")];
        // Absent from the trie: expanded outside a cycle, collapsed in.
        assert!(!collapse.collapsed(&path, false));
        assert!(collapse.collapsed(&path, true));
        // An override forces the state against the default either way.
        collapse.overrides.insert(path.clone(), true);
        assert!(collapse.collapsed(&path, false));
        collapse.overrides.insert(path.clone(), false);
        assert!(!collapse.collapsed(&path, true));
    }
}
