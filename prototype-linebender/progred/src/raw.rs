//! The raw projection: any gid document rendered as entity blocks of
//! edge rows, with no schema and no interpretation of semantic
//! conventions — every node is just its identicon, every edge a row,
//! including `name`, and lists render as the plain position-labeled
//! nodes they are (list sugar belongs to a convention-aware
//! projection). Known
//! identity spaces render friendly — strings and numbers as their
//! values, node ids as git-style suffixes, positions as their payload
//! hex — and unparsable or unknown ids render as the space-and-bytes
//! they are (an even rawer all-space-and-bytes inspection view could
//! exist; raw itself owns friendly renderings for what it knows).

use crate::conventions::NAME;
use parley::StyleProperty;
use parley::style::FontFamily;
use progred_graph::{Gid, Id, MutGid, NodeId, new_node_id, position};
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

/// Shared with the mounted editors so edited atoms keep their colors.
const STRING_COLOR: [f32; 4] = [0.55, 0.33, 0.28, 1.0];
const NUMBER_COLOR: [f32; 4] = [0.16, 0.40, 0.62, 1.0];

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
            number: style(14.0, NUMBER_COLOR, None),
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
/// top-level items are just a root that is an in-graph list. The root
/// is a location like any other — the empty path — so edits there
/// commit to this field, and deleting it empties the document.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub root: Option<Id>,
    pub gid: MutGid,
}

/// Builds a list: a fresh node whose element edges are ordered
/// position labels.
fn list(gid: &mut MutGid, items: Vec<Id>) -> Id {
    let node = new_node_id();
    let mut last: Option<Id> = None;
    for item in items {
        let pos = position::between(last.as_ref(), None).expect("appending after a valid position");
        gid.set(node, pos.clone(), item);
        last = Some(pos);
    }
    Id::from(node)
}

/// A small document exercising the model's range: named nodes, SID and
/// GUID labels, position-labeled lists, an unnamed scratch node, and a
/// value from an unknown space. Its root is a list of the top-level
/// entities.
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

    let stroke_width = new_node_id();
    gid.set(stroke_width, name.clone(), Id::from("stroke-width"));

    let polygon = new_node_id();
    gid.set(polygon, name.clone(), Id::from("polygon"));
    let points = list(&mut gid, vec![Id::from(origin), Id::from(corner)]);
    gid.set(polygon, Id::from("points"), points);
    gid.set(polygon, Id::from(stroke_width), Id::from(1.5));
    let dash = list(&mut gid, vec![Id::from(2.0), Id::from(3.0)]);
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

    let root = list(
        &mut gid,
        vec![
            Id::from(polygon),
            Id::from(origin),
            Id::from(corner),
            Id::from(stroke_width),
            Id::from(scratch),
        ],
    );
    Document {
        root: Some(root),
        gid,
    }
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
/// (optionally with a text click) does, what toggling a node's
/// collapse does, and how a dispatch reaches the selection's editor
/// state and measurement caches.
pub struct Hooks<C> {
    pub select: Rc<dyn Fn(&mut C, Path, Option<TextClick>)>,
    pub toggle: Rc<dyn Fn(&mut C, Path)>,
    pub edit: Rc<dyn for<'a> Fn(&'a mut C) -> EditCtx<'a>>,
}

impl Cx<'_> {
    fn selected(&self, path: &[Id]) -> bool {
        self.selection.map(Selection::path) == Some(path)
    }
}

/// A location in the projected spanning tree: the sequence of edge
/// labels from the root. The same node or edge can be projected at
/// several paths, so the path — not the id — is the identity a
/// selection names. List elements sit at position labels sibling
/// edits never move; wraps and unwraps will adjust path-keyed state
/// through one general rewrite — see `docs/model.md`.
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
    /// Select the edge at `path`; a string or number value brings a
    /// focused editor (the root included — its commits target the
    /// document's root field).
    pub fn edge(doc: &Document, path: Path) -> Self {
        let edit = resolve(doc, &path)
            .and_then(|value| {
                value
                    .as_str()
                    .map(|s| line_edit(s, STRING_COLOR))
                    .or_else(|| {
                        value
                            .as_number()
                            .map(|n| line_edit(&n.to_string(), NUMBER_COLOR))
                    })
            });
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

fn line_edit(text: &str, color: [f32; 4]) -> LineEditState {
    let mut line = LineEditState::new(text, 14.0);
    line.editor
        .edit_styles()
        .insert(StyleProperty::Brush(Brush::from(Color::new(color))));
    line.editor
        .edit_styles()
        .insert(FontFamily::from("system-ui").into());
    line
}

/// The value at `path`, following each label from the root.
pub fn resolve<'a>(doc: &'a Document, path: &[Id]) -> Option<&'a Id> {
    path.iter()
        .try_fold(doc.root.as_ref()?, |node, label| doc.gid.get(node, label))
}

/// Deletes the value at `path`. Deletion is detachment: the value and
/// anything under it stay in the graph for the orphan pool. The empty
/// path empties the document's root; paths that no longer resolve
/// decline.
pub fn delete_edge(doc: &mut Document, path: &[Id]) -> bool {
    match path.split_last() {
        Some((label, parent_path)) if resolve(doc, path).is_some() => {
            match resolve(doc, parent_path).and_then(Id::as_node_id) {
                Some(parent) => {
                    doc.gid.delete(&parent, label);
                    true
                }
                None => false,
            }
        }
        Some(_) => false,
        None => doc.root.take().is_some(),
    }
}

/// Where the selection lands after deleting `path`: the next sibling,
/// else the previous, else the parent.
pub fn selection_after_delete(descends: &[Descend], path: &[Id]) -> Path {
    sibling(descends, path, true)
        .or_else(|| sibling(descends, path, false))
        .unwrap_or_else(|| {
            path.split_last()
                .map(|(_, parent)| parent.to_vec())
                .unwrap_or_default()
        })
}

/// Toggle the collapse override for the node at `path`, staying
/// sparse: an override matching the default (collapsed inside a
/// cycle, expanded otherwise) is removed rather than stored. Declines
/// unless the value is a node with edges — anything else has nothing
/// to collapse.
pub fn toggle_collapse(doc: &Document, collapse: &mut Collapse, path: &[Id]) -> bool {
    resolve(doc, path)
        .filter(|value| value.as_node_id().is_some())
        .filter(|value| doc.gid.edges(value).is_some_and(|edges| !edges.is_empty()))
        .map(|value| {
            let in_cycle = (0..path.len())
                .filter_map(|end| resolve(doc, &path[..end]))
                .any(|ancestor| ancestor == value);
            let next = !collapse.collapsed(path, in_cycle);
            if next == in_cycle {
                collapse.overrides.remove(path);
            } else {
                collapse.overrides.insert(path.to_vec(), next);
            }
        })
        .is_some()
}

/// Write the selection's editor text through to its location: the
/// graph is the source of truth, updated after every handled event.
/// The edited kind follows the current value — strings write every
/// keystroke; numbers write only when the text parses, since a
/// half-typed state like `3.` has no identity to write. The empty
/// path commits to the document's root field; an edge whose parent no
/// longer resolves to a node drops the write silently — the
/// malformed-graph rule at the mutation boundary.
pub fn write_through(doc: &mut Document, selection: &Selection) {
    let Selection::Edge { path, edit } = selection;
    let target = match path.split_last() {
        Some((label, parent_path)) => resolve(doc, parent_path)
            .and_then(Id::as_node_id)
            .map(|parent| Some((label, parent))),
        None => Some(None),
    };
    if let (Some(edit), Some(target)) = (edit, target) {
        let text = edit.editor.text().to_string();
        let current = resolve(doc, path);
        let next = match current {
            Some(current) if current.as_str().is_some() => Some(Id::from(text)),
            Some(current) if current.as_number().is_some() => {
                text.trim().parse::<f64>().ok().map(Id::from)
            }
            _ => None,
        };
        if let Some(next) = next
            && current != Some(&next)
        {
            match target {
                Some((label, parent)) => doc.gid.set(parent, label.clone(), next),
                None => doc.root = Some(next),
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
    hooks: Hooks<C>,
) -> Node<P> {
    let cx = Cx {
        gid: &doc.gid,
        collapse,
        styles,
        selection,
    };
    // Raw shows the pure graph with no assumptions: the root is
    // projected directly, so a list root renders as its
    // position-labeled edges, not as `[a, b, c]`. List sugar belongs
    // to a convention-aware projection. An empty document is a
    // selectable placeholder at the root path.
    match &doc.root {
        Some(root) => value_view::<C, P>(&cx, tcx, &[], &HashSet::new(), root, &hooks),
        None => descend(
            &cx,
            Vec::new(),
            &hooks,
            text(tcx, "empty document", &cx.styles.dim),
        ),
    }
}

/// A node rendered as a block: its identicon header over its edges,
/// indented and recursively projected. A node with edges carries a
/// disclosure delta to the right of the header — outside the indent —
/// that toggles collapse; collapsed (by default a cycle, or forced by
/// an override) it shows only the header.
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
    let id_text = text(tcx, &short_id(node), &cx.styles.dim);

    if edges.is_empty() {
        return id_text;
    }
    let collapsed = cx.collapse.collapsed(path, ancestors.contains(&id));
    let header = row(
        4.0 * scale,
        vec![id_text, disclosure(path.to_vec(), collapsed, hooks, cx.styles)],
    );
    if collapsed {
        return header;
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

/// Git-style short form of a node id: an ellipsis and the last five
/// hex digits, fixed length even where fewer would disambiguate.
/// Trialing this over identicons (which remain in the sample sheet
/// and are the likely graph-view rendering); a collision within a
/// document is unlikely (about 0.5% somewhere in a hundred-node
/// document) and the display can grow if it ever matters.
fn short_id(id: NodeId) -> String {
    let hex = id.simple().to_string();
    format!("…{}", &hex[hex.len() - 5..])
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
        text(tcx, &short_id(uuid), &cx.styles.dim)
    } else if let Some(bytes) = position::as_position(label) {
        text(tcx, &hex(bytes), &cx.styles.dim)
    } else {
        unknown_view(label, tcx, cx.styles)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The disclosure delta to the right of a node header: down when
/// expanded, right when collapsed. Clicking it reports a toggle for
/// this path without selecting; the extent spans the header height so
/// the target is comfortable, and living outside the child indent it
/// costs no horizontal space.
fn disclosure<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    path: Path,
    collapsed: bool,
    hooks: &Hooks<C>,
    styles: &RawStyles,
) -> Node<P> {
    let scale = styles.scale;
    let toggle = hooks.toggle.clone();
    let brush = styles.dim.brush.clone();
    let (width, ascent, descent) = (12.0 * scale, 14.4 * scale, 3.6 * scale);
    leaf(
        Extent {
            width,
            ascent,
            descent,
        },
        move |p: &mut P, at| {
            let rect = Rect::new(at.x, at.y - ascent, at.x + width, at.y + descent);
            let (x, y) = (at.x + width / 2.0 - 1.0 * scale, at.y - 5.4 * scale);
            let h = 3.8 * scale;
            let mut tri = BezPath::new();
            if collapsed {
                tri.move_to((x - h * 0.5, y - h * 0.9));
                tri.line_to((x - h * 0.5, y + h * 0.9));
                tri.line_to((x + h * 0.9, y));
            } else {
                tri.move_to((x - h * 0.9, y - h * 0.5));
                tri.line_to((x + h * 0.9, y - h * 0.5));
                tri.line_to((x, y + h * 0.9));
            }
            tri.close_path();
            p.fill(tri, brush.clone(), Affine::IDENTITY);
            let toggle = toggle.clone();
            let target = path.clone();
            p.handler().on_pointer_down(move |ctx, event| {
                event.button == Some(PointerButton::Primary)
                    && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                    && {
                        toggle(ctx, target.clone());
                        true
                    }
            });
        },
    )
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
    let editing = cx
        .selection
        .filter(|selection| selection.path() == path)
        .and_then(Selection::edit);
    let inner = if let Some(s) = value.as_str() {
        let content = atom_content(editing, text(tcx, s, &cx.styles.string), cx.styles, hooks);
        row(0.0, vec![
            text(tcx, "\"", &cx.styles.string),
            cursor_target(path.to_vec(), hooks, content),
            text(tcx, "\"", &cx.styles.string),
        ])
    } else if let Some(n) = value.as_number() {
        let content = atom_content(
            editing,
            text(tcx, &n.to_string(), &cx.styles.number),
            cx.styles,
            hooks,
        );
        cursor_target(path.to_vec(), hooks, content)
    } else if let Some(node) = value.as_node_id() {
        node_view(cx, tcx, path, ancestors, node, hooks)
    } else if let Some(bytes) = position::as_position(value) {
        text(tcx, &hex(bytes), &cx.styles.dim)
    } else {
        unknown_view(value, tcx, cx.styles)
    };
    descend(cx, path.to_vec(), hooks, inner)
}

/// An editable atom's content: the selection's focused editor when
/// this atom is being edited, its static text otherwise.
fn atom_content<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    editing: Option<&LineEditState>,
    fallback: Node<P>,
    styles: &RawStyles,
    hooks: &Hooks<C>,
) -> Node<P> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            text_edit(line, true, &styles.edit, move |c| edit_ctx(c))
        }
        None => fallback,
    }
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

/// A value from a space the editor doesn't know — or a known space's
/// non-canonical spelling: the space's short id plus the payload as
/// hex.
fn unknown_view<P: Canvas>(id: &Id, tcx: &mut TextCtx, styles: &RawStyles) -> Node<P> {
    row(
        4.0 * styles.scale,
        vec![
            text(tcx, &short_id(id.space()), &styles.dim),
            text(tcx, &hex(id.payload()), &styles.dim),
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
    fn selecting_an_atom_edge_brings_an_editor() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("old"));
        gid.set(node, Id::from("x"), Id::from(1.5));
        let doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        let at = |labels: &[&str]| {
            Selection::edge(&doc, labels.iter().map(|s| Id::from(*s)).collect())
        };
        assert!(at(&["name"]).edit().is_some());
        assert!(at(&["x"]).edit().is_some());
        // Missing edges and node values carry no editor (this root is
        // a node; an atom root would).
        assert!(at(&["missing"]).edit().is_none());
        assert!(at(&[]).edit().is_none());
    }

    #[test]
    fn number_edits_write_only_when_they_parse() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("x"), Id::from(1.5));
        let mut doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        let path = vec![Id::from("x")];
        let mut selection = Selection::edge(&doc, path.clone());

        selection.edit_mut().unwrap().editor.set_text("2.5");
        write_through(&mut doc, &selection);
        assert_eq!(resolve(&doc, &path), Some(&Id::from(2.5)));

        // Half-typed states leave the last parsed value in place.
        for unparsable in ["2.5e", "", "-", "abc"] {
            selection.edit_mut().unwrap().editor.set_text(unparsable);
            write_through(&mut doc, &selection);
            assert_eq!(resolve(&doc, &path), Some(&Id::from(2.5)));
        }

        selection.edit_mut().unwrap().editor.set_text("-3");
        write_through(&mut doc, &selection);
        assert_eq!(resolve(&doc, &path), Some(&Id::from(-3.0)));
    }

    #[test]
    fn edits_write_through_to_the_edge() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("old"));
        let mut doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        let mut selection = Selection::edge(&doc, vec![Id::from("name")]);
        selection.edit_mut().unwrap().editor.set_text("new");
        write_through(&mut doc, &selection);
        assert_eq!(resolve(&doc, &[Id::from("name")]), Some(&Id::from("new")));
        // A selection without an editor writes nothing.
        let plain = Selection::edge(&doc, vec![Id::from("missing")]);
        write_through(&mut doc, &plain);
        assert_eq!(resolve(&doc, &[Id::from("name")]), Some(&Id::from("new")));
    }

    #[test]
    fn delete_edge_detaches_and_the_root_empties_the_document() {
        let mut gid = MutGid::new();
        let root = new_node_id();
        let child = new_node_id();
        gid.set(root, Id::from("child"), Id::from(child));
        gid.set(child, Id::from("name"), Id::from("c"));
        let mut doc = Document {
            root: Some(Id::from(root)),
            gid,
        };

        assert!(!delete_edge(&mut doc, &[Id::from("missing")]));

        assert!(delete_edge(&mut doc, &[Id::from("child")]));
        assert_eq!(resolve(&doc, &[Id::from("child")]), None);
        // Detachment, not destruction: the orphan keeps its edges.
        assert!(doc.gid.edges(&Id::from(child)).is_some());
        // Already gone: declines.
        assert!(!delete_edge(&mut doc, &[Id::from("child")]));

        assert!(delete_edge(&mut doc, &[]));
        assert_eq!(doc.root, None);
        assert_eq!(resolve(&doc, &[]), None);
        assert!(!delete_edge(&mut doc, &[]));
    }

    #[test]
    fn root_edits_commit_to_the_document_root() {
        let mut doc = Document {
            root: Some(Id::from("old")),
            gid: MutGid::new(),
        };
        let mut selection = Selection::edge(&doc, vec![]);
        selection.edit_mut().unwrap().editor.set_text("new");
        write_through(&mut doc, &selection);
        assert_eq!(doc.root, Some(Id::from("new")));
    }

    #[test]
    fn selection_after_delete_prefers_next_then_previous_then_parent() {
        let ds = descends(&[vec![], vec!["a"], vec!["a", "x"], vec!["a", "y"], vec!["b"]]);
        let path = |p: &[&str]| p.iter().map(|s| Id::from(*s)).collect::<Vec<_>>();
        assert_eq!(selection_after_delete(&ds, &path(&["a"])), path(&["b"]));
        assert_eq!(selection_after_delete(&ds, &path(&["b"])), path(&["a"]));
        assert_eq!(selection_after_delete(&ds, &path(&["a", "x"])), path(&["a", "y"]));
        // An only child falls back to its parent.
        let only = descends(&[vec![], vec!["a"], vec!["a", "x"]]);
        assert_eq!(selection_after_delete(&only, &path(&["a", "x"])), path(&["a"]));
    }

    #[test]
    fn toggle_collapse_stays_sparse_and_respects_cycle_defaults() {
        let mut gid = MutGid::new();
        let root = new_node_id();
        let child = new_node_id();
        gid.set(root, Id::from("child"), Id::from(child));
        gid.set(child, Id::from("back"), Id::from(root));
        gid.set(child, Id::from("name"), Id::from("c"));
        let doc = Document {
            root: Some(Id::from(root)),
            gid,
        };
        let mut collapse = Collapse::default();
        let child_path = vec![Id::from("child")];
        let back_path = vec![Id::from("child"), Id::from("back")];

        // Expanded by default: toggling stores a collapse override,
        // toggling again removes it.
        assert!(toggle_collapse(&doc, &mut collapse, &child_path));
        assert!(collapse.collapsed(&child_path, false));
        assert!(toggle_collapse(&doc, &mut collapse, &child_path));
        assert!(collapse.overrides.is_empty());

        // The back-edge to the root is in a cycle, so its default is
        // collapsed; toggling expands it.
        assert!(toggle_collapse(&doc, &mut collapse, &back_path));
        assert!(!collapse.collapsed(&back_path, true));

        // Strings and missing paths have nothing to collapse.
        assert!(!toggle_collapse(
            &doc,
            &mut collapse,
            &[Id::from("child"), Id::from("name")]
        ));
        assert!(!toggle_collapse(&doc, &mut collapse, &[Id::from("nope")]));
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
