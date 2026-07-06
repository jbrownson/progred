//! The raw projection: any gid document rendered as entity blocks of
//! edge rows, with no schema and no interpretation of semantic
//! conventions — every node is just its identicon, every edge a row,
//! including `name`, and cons cells render as the plain nodes they
//! are (list sugar belongs to a convention-aware projection). String
//! and number labels render as their values, node labels and values
//! get identicons, unparsable ids render as what they are.

use crate::conventions::{EMPTY, HEAD, NAME, TAIL};
use crate::identicon::{label_identicon, node_identicon};
use parley::StyleProperty;
use parley::style::FontFamily;
use progred_graph::{Gid, Id, MutGid, NodeId, new_node_id};
use puri::draw::Canvas;
use puri::edit::{EditCtx, EditStyle, LineEditState, text_edit};
use puri::handler::HasHandler;
use puri::layout::{Extent, HAlign, Node, col, decorate, leaf, pad, row};
use puri::text::{TextCtx, TextStyle, text};
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, BezPath, Insets, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Brush, Color};

/// Shared with the mounted string editor so edited text keeps the
/// string color.
const STRING_COLOR: [f32; 4] = [0.55, 0.33, 0.28, 1.0];

pub struct RawStyles {
    pub label: TextStyle,
    pub string: TextStyle,
    pub number: TextStyle,
    pub dim: TextStyle,
    pub edit: EditStyle,
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
            string: style(14.0, STRING_COLOR, None),
            number: style(14.0, [0.16, 0.40, 0.62, 1.0], None),
            dim: style(13.0, [0.55, 0.58, 0.64, 1.0], None),
            edit: EditStyle {
                selection: Brush::from(Color::new([0.0, 0.48, 1.0, 0.30])),
                cursor: Brush::from(Color::new([0.13, 0.14, 0.16, 1.0])),
            },
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

/// A reported click on a string's text, in text-local coordinates.
/// The shell's selection transition consumes it to seed or advance
/// the editor state — focus and caret placement are one event, as in
/// the Haskell LineEdit's focus-with-initial-selection callback.
pub struct TextClick {
    pub point: Point,
    pub shift: bool,
}

/// Dispatch-time callbacks the shell injects: what selecting a path
/// (optionally with a text click) does, and how a dispatch reaches
/// the selection's editor state and measurement caches.
struct Hooks<C> {
    select: Rc<dyn Fn(&mut C, Path, Option<TextClick>)>,
    edit: Rc<dyn for<'a> Fn(&'a mut C) -> EditCtx<'a>>,
}

impl Cx<'_> {
    fn selected(&self, path: &[Id]) -> bool {
        self.selection.map(Selection::path) == Some(path)
    }
}

/// A location in the projected spanning tree: the sequence of edge
/// labels from the root. The same node or edge can be projected at
/// several paths, so the path — not the id — is the identity a
/// selection names. Through a cons list a path is a chain of tail
/// labels — positional, not the cell-anchored stability
/// `docs/model.md` asks of splices — so path elements grow cell
/// anchors when edits land.
pub type Path = Vec<Id>;

/// What is selected. An edge (the value at a path) for now; splice
/// selections (the gaps between items) arrive with insertion points.
/// A selected string edge carries its live editor state — every
/// string is a text editor, focused by selection, and the graph is
/// written through as it edits.
pub enum Selection {
    Edge { path: Path, edit: Option<LineEditState> },
}

impl Selection {
    /// Select the edge at `path`; a string value at an edge brings a
    /// focused editor.
    pub fn edge(doc: &Document, path: Path) -> Self {
        let edit = path
            .split_last()
            .and_then(|_| resolve(doc, &path))
            .and_then(Id::as_str)
            .map(line_edit);
        Selection::Edge { path, edit }
    }

    pub fn path(&self) -> &[Id] {
        match self {
            Selection::Edge { path, .. } => path,
        }
    }

    pub fn edit(&self) -> Option<&LineEditState> {
        match self {
            Selection::Edge { edit, .. } => edit.as_ref(),
        }
    }

    pub fn edit_mut(&mut self) -> Option<&mut LineEditState> {
        match self {
            Selection::Edge { edit, .. } => edit.as_mut(),
        }
    }
}

fn line_edit(text: &str) -> LineEditState {
    let mut line = LineEditState::new(text, 14.0);
    line.editor
        .edit_styles()
        .insert(StyleProperty::Brush(Brush::from(Color::new(STRING_COLOR))));
    line.editor
        .edit_styles()
        .insert(FontFamily::from("system-ui").into());
    line
}

/// The value at `path`, following each label from the root.
pub fn resolve<'a>(doc: &'a Document, path: &[Id]) -> Option<&'a Id> {
    path.iter()
        .try_fold(&doc.root, |node, label| doc.gid.get(node, label))
}

/// Write the selection's editor text through to its edge: the graph
/// is the source of truth, updated after every handled event. A
/// parent that no longer resolves to a node drops the write silently
/// — the malformed-graph rule at the mutation boundary.
pub fn sync_edit(doc: &mut Document, selection: &Selection) {
    let Selection::Edge { path, edit } = selection;
    if let (Some(edit), Some((label, parent_path))) = (edit, path.split_last()) {
        if let Some(parent) = resolve(doc, parent_path).and_then(Id::as_node_id) {
            let value = Id::from(edit.editor.text().to_string());
            if resolve(doc, path) != Some(&value) {
                doc.gid.set(parent, label.clone(), value);
            }
        }
    }
}

/// A projected value's settled position: the path it stands for and the
/// rect it occupied, collected fresh every frame in placement order.
/// [`step_selection`] reads it to move the selection by keyboard;
/// clicks go through each descend's own handler, not this list.
pub struct Descend {
    pub path: Path,
    /// Kept for geometric stepping and scroll-to-selection.
    #[allow(dead_code)]
    pub rect: Rect,
}

/// Keyboard navigation over the frame's descends: a plain arrow steps
/// the selection through the projected tree — left to the parent,
/// right into the first placed child, up and down between siblings in
/// placement order — and any arrow selects the root when nothing is
/// selected. Returns the path to select, or `None` for keys
/// navigation doesn't own.
pub fn step_selection(
    descends: &[Descend],
    selection: Option<&Selection>,
    event: &KeyboardEvent,
) -> Option<Path> {
    let modified = event.modifiers.ctrl()
        || event.modifiers.meta()
        || event.modifiers.alt()
        || event.modifiers.shift();
    let arrow = match &event.key {
        Key::Named(
            named @ (NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::ArrowUp
            | NamedKey::ArrowDown),
        ) => Some(*named),
        _ => None,
    }
    .filter(|_| event.state.is_down() && !modified)?;
    match (arrow, selection) {
        (_, None) => Some(Vec::new()),
        (NamedKey::ArrowLeft, Some(selection)) => selection
            .path()
            .split_last()
            .map(|(_, parent)| parent.to_vec()),
        (NamedKey::ArrowRight, Some(selection)) => {
            let path = selection.path();
            descends
                .iter()
                .map(|descend| &descend.path)
                .find(|p| p.len() == path.len() + 1 && p.starts_with(path))
                .cloned()
        }
        (NamedKey::ArrowUp, Some(selection)) => sibling(descends, selection.path(), false),
        (NamedKey::ArrowDown, Some(selection)) => sibling(descends, selection.path(), true),
        _ => None,
    }
}

fn sibling(descends: &[Descend], path: &[Id], next: bool) -> Option<Path> {
    let (_, parent) = path.split_last()?;
    let siblings: Vec<&Path> = descends
        .iter()
        .map(|descend| &descend.path)
        .filter(|p| p.split_last().is_some_and(|(_, prefix)| prefix == parent))
        .collect();
    let index = siblings.iter().position(|p| p.as_slice() == path)?;
    let index = if next { index + 1 } else { index.checked_sub(1)? };
    siblings.get(index).map(|p| (*p).clone())
}


/// Placement contexts that accumulate descends as the projection
/// places, so the shell can step selection by keyboard.
pub trait HasDescends {
    fn descends(&mut self) -> &mut Vec<Descend>;
}

/// Marks `child` as the projection of the edge at `path`. On placement
/// it draws the highlight when this is the selected edge, registers a
/// click that selects the edge (innermost wins by handler precedence),
/// and records itself for keyboard navigation.
fn descend<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    cx: &Cx,
    path: Path,
    hooks: &Hooks<C>,
    child: Node<P>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let selected = cx.selected(&path);
    let select = hooks.select.clone();
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
                    select(ctx, target.clone(), None);
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
    on_select: impl Fn(&mut C, Path, Option<TextClick>) + 'static,
    edit_ctx: impl for<'a> Fn(&'a mut C) -> EditCtx<'a> + 'static,
) -> Node<P> {
    let hooks = Hooks {
        select: Rc::new(on_select),
        edit: Rc::new(edit_ctx),
    };
    let cx = Cx {
        gid: &doc.gid,
        collapse,
        styles,
        selection,
    };
    // Raw shows the pure graph with no assumptions: the root is
    // projected directly, so a cons-list root renders as its cells, not
    // as a list. List sugar belongs to a convention-aware projection.
    value_view::<C, P>(&cx, tcx, &[], &HashSet::new(), &doc.root, &hooks)
}

/// A node rendered as a block: its identicon/name header over its
/// edges, indented and recursively projected. Collapsed — by default a
/// cycle, or forced by an override — it shows only the header,
/// marked with an ellipsis when it hides edges.
fn node_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    node: NodeId,
    hooks: &Hooks<C>,
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
            let value = value_view(cx, tcx, &child, &inner, &value, hooks);
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
    } else if let Some(n) = label.as_number() {
        text(tcx, &n.to_string(), &cx.styles.number)
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
    hooks: &Hooks<C>,
) -> Node<P> {
    let inner = if let Some(s) = value.as_str() {
        let editing = cx
            .selection
            .filter(|selection| selection.path() == path)
            .and_then(Selection::edit);
        let content = match editing {
            Some(line) => {
                let edit_ctx = hooks.edit.clone();
                text_edit(line, true, &cx.styles.edit, move |c| edit_ctx(c))
            }
            None => text(tcx, s, &cx.styles.string),
        };
        row(0.0, vec![
            text(tcx, "\"", &cx.styles.string),
            cursor_target(path.to_vec(), hooks, content),
            text(tcx, "\"", &cx.styles.string),
        ])
    } else if let Some(n) = value.as_number() {
        text(tcx, &n.to_string(), &cx.styles.number)
    } else if let Some(node) = value.as_node_id() {
        node_view(cx, tcx, path, ancestors, node, hooks)
    } else {
        unknown_view(value, tcx, cx.styles)
    };
    descend(cx, path.to_vec(), hooks, inner)
}

/// A click on a string's text reports what happened — this path, this
/// text-local position — and nothing more; the shell's selection
/// transition decides what it means. One report serves the first
/// click and every one after.
fn cursor_target<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    path: Path,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let select = hooks.select.clone();
    decorate(content, move |p, rect| {
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let click = TextClick {
                        point: Point::new(
                            event.state.position.x - rect.x0,
                            event.state.position.y - rect.y0,
                        ),
                        shift: event.state.modifiers.shift(),
                    };
                    select(ctx, path.clone(), Some(click));
                    true
                }
        });
    })
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
    use ui_events::keyboard::{KeyState, Modifiers};

    fn descends(paths: &[Vec<&str>]) -> Vec<Descend> {
        paths
            .iter()
            .map(|path| Descend {
                path: path.iter().map(|s| Id::from(*s)).collect(),
                rect: Rect::ZERO,
            })
            .collect()
    }

    fn arrow(key: NamedKey) -> KeyboardEvent {
        KeyboardEvent {
            key: Key::Named(key),
            state: KeyState::Down,
            modifiers: Modifiers::empty(),
            ..Default::default()
        }
    }

    fn stepped(ds: &[Descend], from: Option<&[&str]>, key: NamedKey) -> Option<Path> {
        let selection = from.map(|p| Selection::Edge {
            path: p.iter().map(|s| Id::from(*s)).collect(),
            edit: None,
        });
        step_selection(ds, selection.as_ref(), &arrow(key))
    }

    #[test]
    fn arrows_step_selection_through_the_tree() {
        // Placement order: pre-order, parents before children.
        let ds = descends(&[vec![], vec!["a"], vec!["a", "x"], vec!["a", "y"], vec!["b"]]);
        let path = |p: &[&str]| p.iter().map(|s| Id::from(*s)).collect::<Vec<_>>();
        // Nothing selected: any arrow lands on the root.
        assert_eq!(stepped(&ds, None, NamedKey::ArrowDown), Some(vec![]));
        // Right descends to the first placed child, left back to the parent.
        assert_eq!(stepped(&ds, Some(&[]), NamedKey::ArrowRight), Some(path(&["a"])));
        assert_eq!(
            stepped(&ds, Some(&["a", "x"]), NamedKey::ArrowLeft),
            Some(path(&["a"]))
        );
        // Up and down move between siblings in placement order.
        assert_eq!(stepped(&ds, Some(&["a"]), NamedKey::ArrowDown), Some(path(&["b"])));
        assert_eq!(stepped(&ds, Some(&["b"]), NamedKey::ArrowUp), Some(path(&["a"])));
        assert_eq!(
            stepped(&ds, Some(&["a", "x"]), NamedKey::ArrowDown),
            Some(path(&["a", "y"]))
        );
        // Boundaries decline: no parent or sibling above the root, no
        // children below a leaf, no sibling past the last.
        assert_eq!(stepped(&ds, Some(&[]), NamedKey::ArrowLeft), None);
        assert_eq!(stepped(&ds, Some(&[]), NamedKey::ArrowUp), None);
        assert_eq!(stepped(&ds, Some(&["a", "x"]), NamedKey::ArrowRight), None);
        assert_eq!(stepped(&ds, Some(&["b"]), NamedKey::ArrowDown), None);
    }

    #[test]
    fn navigation_declines_modified_keys_releases_and_other_keys() {
        let ds = descends(&[vec![], vec!["a"]]);
        let shifted = KeyboardEvent {
            modifiers: Modifiers::SHIFT,
            ..arrow(NamedKey::ArrowDown)
        };
        assert!(step_selection(&ds, None, &shifted).is_none());
        let released = KeyboardEvent {
            state: KeyState::Up,
            ..arrow(NamedKey::ArrowDown)
        };
        assert!(step_selection(&ds, None, &released).is_none());
        assert!(step_selection(&ds, None, &arrow(NamedKey::Escape)).is_none());
    }

    #[test]
    fn selecting_a_string_edge_brings_an_editor() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("old"));
        gid.set(node, Id::from("x"), Id::from(1.5));
        let doc = Document {
            root: Id::from(node),
            gid,
        };
        let at = |labels: &[&str]| {
            Selection::edge(&doc, labels.iter().map(|s| Id::from(*s)).collect())
        };
        assert!(at(&["name"]).edit().is_some());
        // Numbers, missing edges, and the root carry no editor.
        assert!(at(&["x"]).edit().is_none());
        assert!(at(&["missing"]).edit().is_none());
        assert!(at(&[]).edit().is_none());
    }

    #[test]
    fn edits_write_through_to_the_edge() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("old"));
        let mut doc = Document {
            root: Id::from(node),
            gid,
        };
        let mut selection = Selection::edge(&doc, vec![Id::from("name")]);
        selection.edit_mut().unwrap().editor.set_text("new");
        sync_edit(&mut doc, &selection);
        assert_eq!(resolve(&doc, &[Id::from("name")]), Some(&Id::from("new")));
        // A selection without an editor writes nothing.
        let plain = Selection::edge(&doc, vec![Id::from("missing")]);
        sync_edit(&mut doc, &plain);
        assert_eq!(resolve(&doc, &[Id::from("name")]), Some(&Id::from("new")));
    }

    #[test]
    fn collapse_default_follows_cycle_and_overrides_win() {
        let mut collapse = Collapse::default();
        let path = vec![Id::from("a"), Id::from("b")];
        // Absent from the overrides: expanded outside a cycle, collapsed in.
        assert!(!collapse.collapsed(&path, false));
        assert!(collapse.collapsed(&path, true));
        // An override forces the state against the default either way.
        collapse.overrides.insert(path.clone(), true);
        assert!(collapse.collapsed(&path, false));
        collapse.overrides.insert(path.clone(), false);
        assert!(!collapse.collapsed(&path, true));
    }
}
