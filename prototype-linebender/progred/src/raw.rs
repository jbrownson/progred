//! The raw projection: any gid document rendered as entity blocks of
//! edge rows, with no schema required. The happy path is smooth (names
//! inline, cons chains as `[a, b, c]`) and everything else falls
//! through — SID labels render plain, GUID labels get circular
//! identicons, unparsable ids render as what they are.

use crate::conventions::{EMPTY, HEAD, NAME, TAIL};
use crate::identicon::{label_identicon, node_identicon};
use progred_graph::{Gid, Id, MutGid, NodeId, new_node_id};
use puri::draw::Canvas;
use std::collections::HashSet;
use puri::handler::HasHandler;
use puri::interact::clickable;
use puri::layout::{HAlign, Node, col, decorate, pad, row};
use puri::text::{TextCtx, TextStyle, text};
use vello::kurbo::{Affine, Insets, RoundedRect};
use vello::peniko::{Brush, Color};

pub struct RawStyles {
    pub name: TextStyle,
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
        Self {
            name: style(15.0, [0.95, 0.95, 0.97, 1.0], Some(600.0)),
            label: style(14.0, [0.62, 0.66, 0.74, 1.0], None),
            string: style(14.0, [0.62, 0.83, 0.63, 1.0], None),
            number: style(14.0, [0.90, 0.72, 0.48, 1.0], None),
            dim: style(13.0, [0.48, 0.51, 0.58, 1.0], None),
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
}

pub fn project<C: 'static, P: Canvas + HasHandler<C>>(
    doc: &Document,
    selection: Option<&Id>,
    collapse: &Collapse,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    on_select: impl Fn(&mut C, Id) + Clone + 'static,
) -> Node<P> {
    let cx = Cx {
        gid: &doc.gid,
        collapse,
        styles,
    };
    // The root is projected directly; a list root lays its items out as
    // blocks (the top-level view), anything else as a single block. The
    // root list carries no brackets — that framing is for nested lists.
    let items = list_items(cx.gid, &doc.root).unwrap_or_else(|| vec![doc.root.clone()]);
    let blocks = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let path = list_item_path(&[], i);
            let block = value_view::<P>(&cx, tcx, &path, &HashSet::new(), item);
            let selected = selection == Some(item);
            let block = highlight(block, selected, styles);
            let id = item.clone();
            let select = on_select.clone();
            clickable(block, move |ctx| select(ctx, id.clone()))
        })
        .collect();
    col(HAlign::Start, 0, 14.0 * styles.scale, blocks)
}

/// The graph path to the `i`th item of a cons list rooted at `base`:
/// `i` tail steps then a head step. Distinct per item (they differ in
/// tail depth), so it is a stable key for selection and collapse.
fn list_item_path(base: &[Id], i: usize) -> Vec<Id> {
    let mut path = base.to_vec();
    path.extend(std::iter::repeat(Id::from(TAIL)).take(i));
    path.push(Id::from(HEAD));
    path
}

/// Draws a rounded fill behind a selected block. Runs before the block
/// places (decorate's order), so it sits behind the content.
fn highlight<P: Canvas>(node: Node<P>, selected: bool, styles: &RawStyles) -> Node<P> {
    if !selected {
        return node;
    }
    let scale = styles.scale;
    decorate(node, move |p, rect| {
        let pad = 5.0 * scale;
        let bg = RoundedRect::from_rect(rect.inset(pad), 6.0 * scale);
        p.fill(bg, Color::new([0.17, 0.24, 0.38, 1.0]), Affine::IDENTITY);
    })
}

/// A node rendered as a block: its identicon/name header over its
/// edges, indented and recursively projected. Collapsed — by default a
/// cycle, or forced by the collapse trie — it shows only the header,
/// marked with an ellipsis when it hides edges.
fn node_view<P: Canvas>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    node: NodeId,
) -> Node<P> {
    let scale = cx.styles.scale;
    let id = Id::from(node);
    let edges = non_name_edges(cx.gid, &id);
    let header = header_row(cx, tcx, node);

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
            let value = value_view(cx, tcx, &child, &inner, &value);
            row(6.0 * scale, vec![label, value])
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

fn header_row<P: Canvas>(cx: &Cx, tcx: &mut TextCtx, node: NodeId) -> Node<P> {
    let mut parts = vec![node_identicon(node, 18.0 * cx.styles.scale)];
    if let Some(name) = entity_name(cx.gid, &Id::from(node)) {
        parts.push(text(tcx, &name, &cx.styles.name));
    }
    row(6.0 * cx.styles.scale, parts)
}

/// A node's edges minus its name, sorted for stable order.
fn non_name_edges(gid: &MutGid, id: &Id) -> Vec<(Id, Id)> {
    let mut edges: Vec<(Id, Id)> = gid
        .edges(id)
        .map(|edges| edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    edges.retain(|(label, _)| label.as_node_id() != Some(NAME));
    edges.sort();
    edges
}

fn label_view<P: Canvas>(cx: &Cx, tcx: &mut TextCtx, label: &Id) -> Node<P> {
    if let Some(s) = label.as_str() {
        text(tcx, s, &cx.styles.label)
    } else if let Some(uuid) = label.as_node_id() {
        let mut parts = vec![label_identicon(uuid, 14.0 * cx.styles.scale)];
        if let Some(name) = entity_name(cx.gid, label) {
            parts.push(text(tcx, &name, &cx.styles.label));
        }
        row(4.0 * cx.styles.scale, parts)
    } else {
        unknown_view(label, tcx, cx.styles)
    }
}

fn value_view<P: Canvas>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    value: &Id,
) -> Node<P> {
    if let Some(s) = value.as_str() {
        text(tcx, &format!("\"{s}\""), &cx.styles.string)
    } else if let Some(n) = value.as_number() {
        text(tcx, &n.to_string(), &cx.styles.number)
    } else if let Some(node) = value.as_node_id() {
        match list_items(cx.gid, value) {
            Some(items) => list_view(cx, tcx, path, ancestors, &items),
            None => node_view(cx, tcx, path, ancestors, node),
        }
    } else {
        unknown_view(value, tcx, cx.styles)
    }
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

/// A cons list, rendered inline as `[a, b, c]`. Items may themselves be
/// multi-line blocks — the box algebra nests them freely, so nothing
/// forces the list to break vertically.
fn list_view<P: Canvas>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    items: &[Id],
) -> Node<P> {
    let mut parts = vec![text(tcx, "[", &cx.styles.label)];
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            parts.push(text(tcx, ", ", &cx.styles.label));
        }
        parts.push(value_view(cx, tcx, &list_item_path(path, i), ancestors, item));
    }
    parts.push(text(tcx, "]", &cx.styles.label));
    row(0.0, parts)
}

/// Walks a cons chain; `Some` if the value is list-shaped (the empty
/// list or a chain of head/tail cells), returning the items. A cons
/// cycle — a cell reached twice — is not a valid list, so it is `None`.
pub fn list_items(gid: &impl Gid, value: &Id) -> Option<Vec<Id>> {
    let (head, tail, empty) = (Id::from(HEAD), Id::from(TAIL), Id::from(EMPTY));
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = value.clone();
    loop {
        if cursor == empty {
            return Some(items);
        }
        if !seen.insert(cursor.clone()) {
            return None;
        }
        items.push(gid.get(&cursor, &head)?.clone());
        cursor = gid.get(&cursor, &tail)?.clone();
    }
}

fn entity_name(gid: &impl Gid, id: &Id) -> Option<String> {
    gid.get(id, &Id::from(NAME))?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_items_walks_chains_and_guards_cycles() {
        let mut gid = MutGid::new();
        let (a, b) = (new_node_id(), new_node_id());
        let (c1, c2) = (new_node_id(), new_node_id());
        gid.set(c1, Id::from(HEAD), Id::from(a));
        gid.set(c1, Id::from(TAIL), Id::from(c2));
        gid.set(c2, Id::from(HEAD), Id::from(b));
        gid.set(c2, Id::from(TAIL), Id::from(EMPTY));

        assert_eq!(
            list_items(&gid, &Id::from(c1)),
            Some(vec![Id::from(a), Id::from(b)])
        );
        assert_eq!(list_items(&gid, &Id::from(EMPTY)), Some(vec![]));
        assert_eq!(list_items(&gid, &Id::from(a)), None);

        // A cycle terminates as not-a-list instead of hanging.
        gid.set(c2, Id::from(TAIL), Id::from(c1));
        assert_eq!(list_items(&gid, &Id::from(c1)), None);
    }

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
