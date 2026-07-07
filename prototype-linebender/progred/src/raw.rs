//! The raw projection: any gid document rendered as entity blocks of
//! edge rows, with no schema and no interpretation of semantic
//! conventions — every node is just its short id, every edge a row,
//! including `name`, and lists render as the plain position-labeled
//! nodes they are (list sugar belongs to a convention-aware
//! projection). Known
//! identity spaces render friendly — strings and numbers as their
//! values, node ids as git-style suffixes, positions as their payload
//! hex — and unparsable or unknown ids render as the space-and-bytes
//! they are (an even rawer all-space-and-bytes inspection view could
//! exist; raw itself owns friendly renderings for what it knows).

use crate::conventions::NAME;
use crate::filter;
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
const QUERY_COLOR: [f32; 4] = [0.46, 0.49, 0.55, 1.0];

pub struct RawStyles {
    pub label: TextStyle,
    pub string: TextStyle,
    pub number: TextStyle,
    pub dim: TextStyle,
    /// Byte-identity renderings — short ids and hex — in monospace,
    /// so ids read as ids and align when compared.
    pub id: TextStyle,
    pub edit: EditStyle,
    pub scale: f64,
}

impl RawStyles {
    pub fn new(scale: f64) -> Self {
        let style = |size: f32, color: [f32; 4], weight: Option<f32>| TextStyle {
            size,
            brush: Brush::from(Color::new(color)),
            weight,
            family: parley::style::GenericFamily::SystemUi,
        };
        // A light, native-feeling palette: near-black primary labels,
        // gray secondary labels, restrained literal accents.
        Self {
            label: style(14.0, [0.46, 0.49, 0.55, 1.0], None),
            string: style(14.0, STRING_COLOR, None),
            number: style(14.0, NUMBER_COLOR, None),
            dim: style(13.0, [0.55, 0.58, 0.64, 1.0], None),
            id: TextStyle {
                family: parley::style::GenericFamily::Monospace,
                ..style(13.0, [0.55, 0.58, 0.64, 1.0], None)
            },
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
/// Clones are O(1): the gid shares structure, which is what makes
/// snapshot undo free.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    /// The document root — a Document field, not a gid edge, so the
    /// completion sweep needs it passed alongside the gid.
    root: Option<&'a Id>,
    collapse: &'a Collapse,
    styles: &'a RawStyles,
    selection: Option<&'a Selection>,
    /// The node whose other projections carry the secondary mark.
    secondary: Option<Id>,
}

/// A reported click on a string's text, in text-local coordinates.
/// The shell's selection transition consumes it to seed or advance
/// the editor state — focus and caret placement are one event, as in
/// the Haskell LineEdit's focus-with-initial-selection callback. The
/// count carries double/triple clicks (word and line selection).
pub struct TextClick {
    pub point: Point,
    pub shift: bool,
    pub count: u8,
}

/// Dispatch-time callbacks the shell injects: what selecting a path
/// (optionally with a text click) does, what toggling a node's
/// collapse does, and how a dispatch reaches the selection's editor
/// state and measurement caches.
pub struct Hooks<C> {
    pub select: Rc<dyn Fn(&mut C, Path, Option<TextClick>)>,
    pub toggle: Rc<dyn Fn(&mut C, Path)>,
    /// None when the editor is already gone — retained-frame dispatch
    /// may fire a frame late, and absent state declines.
    pub edit: Rc<dyn for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>>>,
    /// Commit a pointed-at identity into the open pending (value or
    /// label stage); false when nothing is pending, so the click
    /// falls through to selection.
    pub pick: Rc<dyn Fn(&mut C, Id) -> bool>,
}

/// The platform command modifier, for pointer gestures.
pub(crate) fn command(modifiers: &ui_events::keyboard::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.meta()
    } else {
        modifiers.ctrl()
    }
}

impl Cx<'_> {
    /// Whether `path` carries the primary highlight. A label-stage
    /// pending deliberately does not mark its parent — nothing is
    /// selected there, something is being authored inside; the
    /// pending row carries the highlight itself.
    fn selected(&self, path: &[Id]) -> bool {
        match self.selection {
            Some(Selection::Edge { path: selected, .. })
            | Some(Selection::Pending {
                path: selected, ..
            }) => selected.as_slice() == path,
            _ => false,
        }
    }

    /// The pending child label under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Id]) -> Option<Id> {
        match self.selection {
            Some(Selection::Pending { path: pending, .. })
                if pending.split_last().is_some_and(|(_, parent)| parent == path) =>
            {
                pending.last().cloned()
            }
            _ => None,
        }
    }

    /// The label query of a new edge being authored on `path`.
    fn pending_edge_under(&self, path: &[Id]) -> Option<(&LineEditState, usize)> {
        match self.selection {
            Some(Selection::PendingEdge {
                parent,
                query,
                choice,
            }) if parent.as_slice() == path => Some((query, *choice)),
            _ => None,
        }
    }
}

/// A location in the projected spanning tree: the sequence of edge
/// labels from the root. The same node or edge can be projected at
/// several paths, so the path — not the id — is the identity a
/// selection names. List elements sit at position labels sibling
/// edits never move; wraps and unwraps will adjust path-keyed state
/// through one general rewrite — see `docs/model.md`.
pub type Path = Vec<Id>;

/// What is selected: the value at a path, or a nonexistent edge being
/// authored. A selected atom carries its live editor state — every
/// atom is a text editor, focused by selection, and the graph is
/// written through as it edits. A pending selection carries the
/// completion query instead; the query resolves to the identity that
/// commits, and until then the graph is untouched — deselecting
/// discards the pending edge entirely.
pub enum Selection {
    Edge {
        path: Path,
        edit: Option<LineEditState>,
        /// Whether this editor's write-through run has recorded its
        /// undo step: the run is the editor's lifetime, so the first
        /// write records and the rest coalesce by staying silent.
        recorded: bool,
    },
    /// A nonexistent edge's value being authored (the root included).
    Pending {
        path: Path,
        query: LineEditState,
        /// Which completion entry commits; clamped against the
        /// frame's recomputed entries at use.
        choice: usize,
    },
    /// A new edge on `parent` whose label is being authored; resolving
    /// the label advances to the value stage (or selects the existing
    /// edge if the label is taken).
    PendingEdge {
        parent: Path,
        query: LineEditState,
        choice: usize,
    },
}

impl Selection {
    /// Select the edge at `path`; a string or number value brings a
    /// focused editor (the root included — its commits target the
    /// document's root field). Selecting the empty document's root is
    /// already authoring it: there is nothing there to select, only
    /// something to begin, so it pends immediately.
    pub fn edge(doc: &Document, path: Path) -> Self {
        if path.is_empty() && doc.root.is_none() {
            return pending_value(path);
        }
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
        Selection::Edge {
            path,
            edit,
            recorded: false,
        }
    }

    pub fn path(&self) -> &[Id] {
        match self {
            Selection::Edge { path, .. } | Selection::Pending { path, .. } => path,
            Selection::PendingEdge { parent, .. } => parent,
        }
    }

    pub fn edit(&self) -> Option<&LineEditState> {
        match self {
            Selection::Edge { edit, .. } => edit.as_ref(),
            Selection::Pending { query, .. } | Selection::PendingEdge { query, .. } => {
                Some(query)
            }
        }
    }

    pub fn edit_mut(&mut self) -> Option<&mut LineEditState> {
        match self {
            Selection::Edge { edit, .. } => edit.as_mut(),
            Selection::Pending { query, .. } | Selection::PendingEdge { query, .. } => {
                Some(query)
            }
        }
    }
}

// Seeded with the caret at the end: an editor mounted without a text
// click — label click, keyboard landing — starts appending (a
// select-all trial read as dangerous), and a click's caret placement
// overrides it.
fn line_edit(text: &str, color: [f32; 4]) -> LineEditState {
    LineEditState::new(text, 14.0, Brush::from(Color::new(color))).with_cursor_at_end()
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
/// else the previous, else the parent. Also where a discarded pending
/// edge returns to.
pub fn selection_after_delete(descends: &[Descend], path: &[Id]) -> Path {
    sibling(descends, path, true)
        .or_else(|| sibling(descends, path, false))
        .unwrap_or_else(|| {
            path.split_last()
                .map(|(_, parent)| parent.to_vec())
                .unwrap_or_default()
        })
}

/// The sorted position labels of a node's element edges.
fn positions_of(gid: &MutGid, node: &Id) -> Vec<Id> {
    let mut positions: Vec<Id> = gid
        .edges(node)
        .map(|edges| {
            edges
                .keys()
                .filter(|label| position::as_position(label).is_some())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    positions.sort();
    positions
}

/// A value-stage pending: the edge named by `path` does not exist,
/// and its value is being authored.
pub fn pending_value(path: Path) -> Selection {
    Selection::Pending {
        path,
        query: line_edit("", QUERY_COLOR),
        choice: 0,
    }
}

/// A new edge on the node at `parent`, its label to be authored.
/// Raw's one insertion: a node is a bag of labeled edges, and adding
/// to it means adding an edge — no list semantics here (the future
/// list projection owns element gestures).
pub fn pending_edge(doc: &Document, parent: Path) -> Option<Selection> {
    resolve(doc, &parent)?.as_node_id()?;
    Some(Selection::PendingEdge {
        parent,
        query: line_edit("", QUERY_COLOR),
        choice: 0,
    })
}

/// A pending sibling next to the element at `path` (which must sit at
/// a position label), minted between it and its neighbor. Parked for
/// the list projection: raw's gestures no longer mint positions.
#[allow(dead_code)]
fn pending_beside(doc: &Document, path: &[Id], after: bool) -> Option<Selection> {
    let (label, parent_path) = path.split_last()?;
    position::as_position(label)?;
    let parent = resolve(doc, parent_path)?;
    let positions = positions_of(&doc.gid, parent);
    let index = positions.iter().position(|p| p == label)?;
    let fresh = if after {
        position::between(Some(label), positions.get(index + 1))?
    } else {
        position::between(index.checked_sub(1).map(|i| &positions[i]), Some(label))?
    };
    let mut fresh_path = parent_path.to_vec();
    fresh_path.push(fresh);
    Some(pending_value(fresh_path))
}

#[allow(dead_code)]
pub fn pending_after(doc: &Document, path: &[Id]) -> Option<Selection> {
    pending_beside(doc, path, true)
}

#[allow(dead_code)]
pub fn pending_before(doc: &Document, path: &[Id]) -> Option<Selection> {
    pending_beside(doc, path, false)
}

/// A pending element inside the node at `path`, appended at the end
/// or prepended at the front. Element insertion applies where
/// elements plausibly live — nodes with position edges, or empty
/// nodes, which is how lists begin; a node with only record fields
/// declines (field insertion, with its pending label, is a separate
/// gesture).
#[allow(dead_code)]
fn pending_into_at(doc: &Document, path: &[Id], end: bool) -> Option<Selection> {
    let value = resolve(doc, path)?;
    value.as_node_id()?;
    let positions = positions_of(&doc.gid, value);
    let record_only = positions.is_empty()
        && doc.gid.edges(value).is_some_and(|edges| !edges.is_empty());
    (!record_only).then_some(())?;
    let fresh = if end {
        position::between(positions.last(), None)?
    } else {
        position::between(None, positions.first())?
    };
    let mut fresh_path = path.to_vec();
    fresh_path.push(fresh);
    Some(pending_value(fresh_path))
}

/// Appends: "add to this list" goes at the end. Parked with its
/// siblings for the list projection.
#[allow(dead_code)]
pub fn pending_into(doc: &Document, path: &[Id]) -> Option<Selection> {
    pending_into_at(doc, path, true)
}

#[allow(dead_code)]
pub fn pending_into_first(doc: &Document, path: &[Id]) -> Option<Selection> {
    pending_into_at(doc, path, false)
}

/// A pending root for an empty document.
pub fn pending_root(doc: &Document) -> Option<Selection> {
    doc.root.is_none().then(|| pending_value(Vec::new()))
}

/// The identity a pending query resolves to: a leading quote forces a
/// string (the closing quote optional, so string mode holds while
/// typing), text that parses is a number, anything else is the string
/// as typed.
pub fn resolve_query(text: &str) -> Id {
    let trimmed = text.trim();
    match trimmed.strip_prefix('"') {
        Some(inner) => Id::from(inner.strip_suffix('"').unwrap_or(inner)),
        None => trimmed
            .parse::<f64>()
            .map(Id::from)
            .unwrap_or_else(|_| Id::from(text)),
    }
}

/// A completion offer on a pending edge. The display styles itself by
/// the action's kind at draw time.
#[derive(Clone)]
pub struct Entry {
    pub display: String,
    pub detail: Option<String>,
    pub action: EntryAction,
}

#[derive(Clone)]
pub enum EntryAction {
    /// Commit this identity: an inferred atom or a reference.
    Value(Id),
    /// Mint a node, optionally named, and commit it.
    NewNode { name: Option<String> },
}

/// The completion popup a pending row emits during placement; the
/// shell draws it after the body and commits from it. Recomputed
/// every frame like everything else.
pub struct Popup {
    pub anchor: Rect,
    pub entries: Vec<Entry>,
    pub choice: usize,
}

/// Placement contexts that carry the frame's popup.
pub trait HasPopup {
    fn popup(&mut self) -> &mut Option<Popup>;
}

/// The universal completion layer for `query`: the inferred atom,
/// references to everything named (document and orphans alike, ranked
/// by the fuzzy tiers), and a fresh node — named after the query when
/// there is one, the create-on-reference of the floating-definitions
/// design.
fn completion_entries(gid: &MutGid, root: Option<&Id>, query: &str) -> Vec<Entry> {
    let atom = resolve_query(query);
    let display = match atom.as_str() {
        Some(s) => format!("\"{s}\""),
        None => atom
            .as_number()
            .map(|n| n.to_string())
            .unwrap_or_default(),
    };
    // Quotes and numbers state atom intent, so the atom leads;
    // otherwise a confident (non-fuzzy) reference match is likelier
    // the intent than a new literal — typing a visible name or short
    // id should default to the reference, and quoting always forces
    // the string.
    let atom_leads = query.trim_start().starts_with('"') || atom.as_number().is_some();
    let atom_entry = Entry {
        display,
        detail: None,
        action: EntryAction::Value(atom),
    };
    // Every node the document contains is referenceable: named ones
    // by name, unnamed ones by the short id they render as — what
    // you see is what you can type. Unnamed keys start with the
    // ellipsis, which sorts after names, so they trail on an empty
    // query.
    let mut nodes: Vec<(String, Id)> = document_nodes(gid, root)
        .into_iter()
        .map(|node| {
            let id = Id::from(node);
            let key = gid
                .get(&id, &Id::from(NAME))
                .and_then(Id::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| short_id(node));
            (key, id)
        })
        .collect();
    nodes.sort();
    let references: Vec<(Entry, bool)> = filter::rank(nodes, |(key, _)| key, query)
        .into_iter()
        .take(8)
        .map(|ranked| {
            let fuzzy = ranked.fuzzy();
            let (display, id) = ranked.item;
            let detail = id
                .as_node_id()
                .map(short_id)
                .filter(|detail| *detail != display);
            let entry = Entry {
                display,
                detail,
                action: EntryAction::Value(id),
            };
            (entry, fuzzy)
        })
        .collect();
    let mut entries = Vec::new();
    if atom_leads {
        entries.push(atom_entry);
        entries.extend(references.into_iter().map(|(entry, _)| entry));
    } else {
        let (weak, strong): (Vec<_>, Vec<_>) =
            references.into_iter().partition(|(_, fuzzy)| *fuzzy);
        entries.extend(strong.into_iter().map(|(entry, _)| entry));
        entries.push(atom_entry);
        entries.extend(weak.into_iter().map(|(entry, _)| entry));
    }
    let trimmed = query.trim();
    let name_text = trimmed
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(trimmed);
    let name = (!name_text.is_empty()).then(|| name_text.to_string());
    entries.push(Entry {
        display: match &name {
            Some(name) => format!("new node \"{name}\""),
            None => "new node".to_string(),
        },
        detail: None,
        action: EntryAction::NewNode { name },
    });
    entries
}

/// Every GUID node the document contains — entity sources, nodes
/// appearing only as labels or values (an edgeless node referenced
/// somewhere is still referenceable), and the root, whose reference
/// is a Document field rather than a gid edge — a fresh edgeless
/// node at root is still in the document. Sorted for a deterministic
/// offer order.
fn document_nodes(gid: &MutGid, root: Option<&Id>) -> Vec<NodeId> {
    let mut nodes: Vec<NodeId> = gid
        .entities()
        .flat_map(|entity| {
            let edge_nodes = gid
                .edges(&Id::from(*entity))
                .into_iter()
                .flatten()
                .flat_map(|(label, value)| [label.as_node_id(), value.as_node_id()])
                .flatten();
            std::iter::once(*entity).chain(edge_nodes)
        })
        .chain(root.and_then(Id::as_node_id))
        .collect();
    nodes.sort();
    nodes.dedup();
    nodes
}

/// Resolves a chosen entry to the identity it denotes, minting and
/// naming for a new node. Labels and values resolve alike.
pub fn resolve_entry(doc: &mut Document, action: &EntryAction) -> Id {
    match action {
        EntryAction::Value(id) => id.clone(),
        EntryAction::NewNode { name } => {
            let node = new_node_id();
            if let Some(name) = name {
                doc.gid.set(node, Id::from(NAME), Id::from(name.as_str()));
            }
            Id::from(node)
        }
    }
}

/// Commits a pending edge from a chosen entry: resolves the action to
/// an identity and writes it.
pub fn commit_pending(doc: &mut Document, path: &[Id], action: &EntryAction) -> bool {
    let value = resolve_entry(doc, action);
    set_value(doc, path, value)
}

/// Writes `value` at `path` — the empty path writes the document
/// root. The single-location write every edit reduces to.
pub fn set_value(doc: &mut Document, path: &[Id], value: Id) -> bool {
    match path.split_last() {
        Some((label, parent_path)) => {
            match resolve(doc, parent_path).and_then(Id::as_node_id) {
                Some(parent) => {
                    doc.gid.set(parent, label.clone(), value);
                    true
                }
                None => false,
            }
        }
        None => {
            doc.root = Some(value);
            true
        }
    }
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
/// Writes the focused editor's text through to the graph. Returns
/// whether this write OPENED an undo step: true exactly on the first
/// write of the mounted editor's life, so a typing run is one step
/// and history stays a dumb stack.
pub fn write_through(doc: &mut Document, selection: &mut Selection) -> bool {
    let Selection::Edge {
        path,
        edit,
        recorded,
    } = selection
    else {
        return false;
    };
    let target = match path.split_last() {
        Some((label, parent_path)) => resolve(doc, parent_path)
            .and_then(Id::as_node_id)
            .map(|parent| Some((label, parent))),
        None => Some(None),
    };
    if let (Some(edit), Some(target)) = (edit, target) {
        let text = edit.text().to_string();
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
            let first = !*recorded;
            *recorded = true;
            return first;
        }
    }
    false
}

/// Breaks the open edit run: the next write records a fresh undo
/// step. Called after a save, so a run never straddles the mark.
pub fn break_edit_run(selection: &mut Option<Selection>) {
    if let Some(Selection::Edge { recorded, .. }) = selection {
        *recorded = false;
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

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas>(scale: f64, p: &mut P, rect: Rect) {
    let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
    p.fill(bg, Color::new([0.0, 0.48, 1.0, 0.22]), Affine::IDENTITY);
    p.stroke(
        bg,
        Stroke::new(2.5 * scale),
        Color::new([0.0, 0.48, 1.0, 1.0]),
        Affine::IDENTITY,
    );
}

/// Marks `child` as the projection of the edge at `path`. On placement
/// it draws the highlight when this is the selected edge, registers a
/// click that selects the edge (innermost wins by handler precedence)
/// — or, with the command modifier and a pending open, picks `value`
/// into it — and records itself for keyboard navigation.
fn descend<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    cx: &Cx,
    path: Path,
    value: Option<Id>,
    hooks: &Hooks<C>,
    child: Node<P>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let selected = cx.selected(&path);
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    decorate(child, move |p, rect| {
        if selected {
            primary_highlight(scale, p, rect);
        }
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let picked = command(&event.state.modifiers)
                        && value
                            .as_ref()
                            .is_some_and(|id| pick(ctx, id.clone()));
                    if !picked {
                        select(ctx, target.clone(), None);
                    }
                    true
                }
        });
        p.descends().push(Descend { path, rect });
    })
}

/// The identity marked as the secondary selection: the one at the end
/// of the selected edge. An identity can project in many places —
/// GUID nodes, but equally SID strings, NID numbers, and labels — and
/// the marks make that sameness visible, uniformly across spaces.
fn secondary_of(doc: &Document, selection: Option<&Selection>) -> Option<Id> {
    match selection? {
        Selection::Edge { path, .. } => resolve(doc, path).cloned(),
        _ => None,
    }
}

pub fn project<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    doc: &Document,
    selection: Option<&Selection>,
    graph_node: Option<&Id>,
    collapse: &Collapse,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    hooks: Hooks<C>,
) -> Node<P> {
    let cx = Cx {
        gid: &doc.gid,
        root: doc.root.as_ref(),
        collapse,
        styles,
        selection,
        // The graph view's selected node is a secondary here too:
        // its projections are the same identity.
        secondary: secondary_of(doc, selection).or_else(|| graph_node.cloned()),
    };
    // Raw shows the pure graph with no assumptions: the root is
    // projected directly, so a list root renders as its
    // position-labeled edges, not as `[a, b, c]`. List sugar belongs
    // to a convention-aware projection. An empty document is a
    // selectable placeholder at the root path.
    match &doc.root {
        Some(root) => value_view::<C, P>(&cx, tcx, &[], &HashSet::new(), root, &hooks),
        None => match cx.selection {
            Some(Selection::Pending { path, .. }) if path.is_empty() => {
                pending_view(&cx, tcx, Vec::new(), &hooks)
            }
            _ => descend(
                &cx,
                Vec::new(),
                None,
                &hooks,
                text(tcx, "empty document", &cx.styles.dim),
            ),
        },
    }
}

/// A node rendered as a block: its short-id header over its edges,
/// indented and recursively projected. A node with edges carries a
/// disclosure delta to the right of the header — outside the indent —
/// that toggles collapse; collapsed (by default a cycle, or forced by
/// an override) it shows only the header.
fn node_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Id],
    ancestors: &HashSet<Id>,
    node: NodeId,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let id = Id::from(node);
    let mut entries: Vec<(Id, Option<Id>)> = sorted_edges(cx.gid, &id)
        .into_iter()
        .map(|(label, value)| (label, Some(value)))
        .collect();
    if let Some(label) = cx.pending_child_of(path) {
        entries.push((label, None));
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let id_text = text(tcx, &short_id(node), &cx.styles.id);

    let pending_edge = cx.pending_edge_under(path).is_some();
    if entries.is_empty() && !pending_edge {
        return id_text;
    }
    // A pending child or edge forces the node open so it can be seen.
    let collapsed = !pending_edge
        && entries.iter().all(|(_, value)| value.is_some())
        && cx.collapse.collapsed(path, ancestors.contains(&id));
    let header = row(
        4.0 * scale,
        vec![id_text, disclosure(path.to_vec(), collapsed, hooks, cx.styles)],
    );
    if collapsed {
        return header;
    }

    let mut inner = ancestors.clone();
    inner.insert(id);
    let mut rows: Vec<Node<P>> = entries
        .into_iter()
        .map(|(label, value)| {
            let mut child = path.to_vec();
            child.push(label.clone());
            // A real edge's label and arrow select the edge, like its
            // value — grouped so one target spans both and the gap
            // between. A pending row's plain click falls through (the
            // not-yet-edge can't be selected), but command still
            // picks its label's identity.
            let head = row(
                6.0 * scale,
                vec![label_view(cx, tcx, &label), arrow(cx.styles)],
            );
            let head = match &value {
                Some(_) => select_target(child.clone(), label.clone(), hooks, head),
                None => pick_target(label.clone(), hooks, head),
            };
            let content = match value {
                Some(value) => value_view(cx, tcx, &child, &inner, &value, hooks),
                None => pending_view(cx, tcx, child, hooks),
            };
            row(6.0 * scale, vec![head, content])
        })
        .collect();
    // A new edge being authored: the label query, unsorted until it
    // has a label to sort by.
    if let Some((query, choice)) = cx.pending_edge_under(path) {
        let pending_row = row(
            6.0 * scale,
            vec![
                query_content(cx, tcx, query, choice, hooks),
                arrow(cx.styles),
                text(tcx, "…", &cx.styles.dim),
            ],
        );
        // The authoring locus carries the primary itself; its parent
        // is deliberately unmarked.
        rows.push(decorate(pending_row, move |p: &mut P, rect| {
            primary_highlight(scale, p, rect);
        }));
    }
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
/// A collision within a document is unlikely (about 0.5% somewhere in
/// a hundred-node document) and the display can grow if it ever
/// matters.
pub fn short_id(id: NodeId) -> String {
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
    let inner = if let Some(s) = label.as_str() {
        text(tcx, s, &cx.styles.label)
    } else if let Some(n) = label.as_number() {
        text(tcx, &n.to_string(), &cx.styles.number)
    } else if let Some(uuid) = label.as_node_id() {
        text(tcx, &short_id(uuid), &cx.styles.id)
    } else if let Some(bytes) = position::as_position(label) {
        text(tcx, &hex(bytes), &cx.styles.id)
    } else {
        unknown_view(label, tcx, cx.styles)
    };
    secondary_mark(cx, label, inner)
}

/// The secondary selection's mark: a subtle wash over another whole
/// projection of the selected edge's node — an expanded block, a
/// collapsed header, or a GUID label. The primary selection's
/// geometry at lower strength, so the two read as one family.
fn secondary_mark<P: Canvas>(cx: &Cx, id: &Id, content: Node<P>) -> Node<P> {
    if cx.secondary.as_ref() != Some(id) {
        return content;
    }
    let scale = cx.styles.scale;
    decorate(content, move |p: &mut P, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, Color::new([0.0, 0.48, 1.0, 0.10]), Affine::IDENTITY);
        p.stroke(
            bg,
            Stroke::new(1.5 * scale),
            Color::new([0.0, 0.48, 1.0, 0.55]),
            Affine::IDENTITY,
        );
    })
}

pub fn hex(bytes: &[u8]) -> String {
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
/// "label → value", and being a stroke rather than an id, it
/// separates an edge's key from its target — labels and values are
/// otherwise both ids.
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

fn value_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
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
        let fallback = text(tcx, s, &cx.styles.string);
        let content = atom_content(editing, fallback, tcx, cx.styles, hooks);
        row(0.0, vec![
            text(tcx, "\"", &cx.styles.string),
            cursor_target(path.to_vec(), value.clone(), hooks, content),
            text(tcx, "\"", &cx.styles.string),
        ])
    } else if let Some(n) = value.as_number() {
        let fallback = text(tcx, &n.to_string(), &cx.styles.number);
        let content = atom_content(editing, fallback, tcx, cx.styles, hooks);
        cursor_target(path.to_vec(), value.clone(), hooks, content)
    } else if let Some(node) = value.as_node_id() {
        node_view(cx, tcx, path, ancestors, node, hooks)
    } else if let Some(bytes) = position::as_position(value) {
        text(tcx, &hex(bytes), &cx.styles.id)
    } else {
        unknown_view(value, tcx, cx.styles)
    };
    // Other projections of the selected edge's node carry the
    // secondary mark; the selected one has the primary highlight.
    let inner = if cx.selected(path) {
        inner
    } else {
        secondary_mark(cx, value, inner)
    };
    descend(cx, path.to_vec(), Some(value.clone()), hooks, inner)
}

/// A nonexistent edge the selection is authoring: the completion
/// query's focused editor, wrapped as an ordinary descend so it
/// highlights, clicks, and navigates like the edge it may become. Its
/// placement emits the completion popup for the shell to draw over
/// the body.
fn pending_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Path,
    hooks: &Hooks<C>,
) -> Node<P> {
    let content = match cx.selection {
        Some(Selection::Pending {
            path: pending,
            query,
            choice,
        }) if pending.as_slice() == path.as_slice() => {
            query_content(cx, tcx, query, *choice, hooks)
        }
        _ => text(tcx, "…", &cx.styles.dim),
    };
    descend(cx, path, None, hooks, content)
}

/// A focused completion query: the editor plus its popup, emitted at
/// placement for the shell to draw over the body. Serves both pending
/// stages — a value and a new edge's label.
fn query_content<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    choice: usize,
    hooks: &Hooks<C>,
) -> Node<P> {
    let entries = completion_entries(cx.gid, cx.root, query.text());
    let fallback = text(tcx, "…", &cx.styles.dim);
    let content = atom_content(Some(query), fallback, tcx, cx.styles, hooks);
    decorate(content, move |p: &mut P, rect| {
        *p.popup() = Some(Popup {
            anchor: rect,
            entries,
            choice,
        });
    })
}

/// The drawn completion card: entry rows under the pending anchor,
/// the chosen one highlighted, styled by what each entry commits.
/// The shell places it after the body, so it overlays and its
/// handlers win: clicking a row commits it, and the card swallows
/// every other click so nothing lands on content underneath.
pub fn popup_view<C: 'static, P: Canvas + HasHandler<C>>(
    tcx: &mut TextCtx,
    styles: &RawStyles,
    popup: &Popup,
    commit: impl Fn(&mut C, &EntryAction) + Clone + 'static,
) -> Node<P> {
    let scale = styles.scale;
    let choice = popup.choice.min(popup.entries.len().saturating_sub(1));
    let rows: Vec<Node<P>> = popup
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let style = match &entry.action {
                EntryAction::Value(id) if id.as_str().is_some() => &styles.string,
                EntryAction::Value(id) if id.as_number().is_some() => &styles.number,
                EntryAction::Value(_) => &styles.label,
                EntryAction::NewNode { .. } => &styles.dim,
            };
            let mut cells: Vec<Node<P>> = vec![text(tcx, &entry.display, style)];
            if let Some(detail) = &entry.detail {
                cells.push(text(tcx, detail, &styles.id));
            }
            let content = pad(
                Insets::new(8.0 * scale, 2.0 * scale, 8.0 * scale, 2.0 * scale),
                row(8.0 * scale, cells),
            );
            let chosen = index == choice;
            let action = entry.action.clone();
            let commit = commit.clone();
            decorate(content, move |p: &mut P, rect| {
                if chosen {
                    p.fill(
                        RoundedRect::from_rect(rect, 4.0 * scale),
                        Color::new([0.0, 0.48, 1.0, 0.14]),
                        Affine::IDENTITY,
                    );
                }
                p.handler().on_pointer_down(move |ctx, event| {
                    event.button == Some(PointerButton::Primary)
                        && rect.contains(Point::new(
                            event.state.position.x,
                            event.state.position.y,
                        ))
                        && {
                            commit(ctx, &action);
                            true
                        }
                });
            })
        })
        .collect();
    let card = pad(
        Insets::uniform(4.0 * scale),
        col(HAlign::Start, 0, 2.0 * scale, rows),
    );
    decorate(card, move |p: &mut P, rect| {
        let shape = RoundedRect::from_rect(rect, 6.0 * scale);
        p.fill(shape, Color::new([1.0, 1.0, 1.0, 1.0]), Affine::IDENTITY);
        p.stroke(
            shape,
            Stroke::new(1.0 * scale),
            Color::new([0.75, 0.77, 0.81, 1.0]),
            Affine::IDENTITY,
        );
        p.handler().on_pointer_down(move |_, event| {
            rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
    })
}

/// An editable atom's content: the selection's focused editor when
/// this atom is being edited, its static text otherwise.
fn atom_content<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    editing: Option<&LineEditState>,
    fallback: Node<P>,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    hooks: &Hooks<C>,
) -> Node<P> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            text_edit(line, true, &styles.edit, tcx, move |c| edit_ctx(c))
        }
        None => fallback,
    }
}

/// A command-click pick target with no plain-click behavior — for
/// parts like a pending row's label, whose plain click deliberately
/// falls through.
fn pick_target<C: 'static, P: Canvas + HasHandler<C>>(
    id: Id,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let pick = hooks.pick.clone();
    decorate(content, move |p, rect| {
        let pick = pick.clone();
        let id = id.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && command(&event.state.modifiers)
                && pick(ctx, id.clone())
        });
    })
}

/// A plain click-to-select target for `path` — for edge parts like
/// labels that select without carrying an editor click. With the
/// command modifier and a pending open, picks `label` — the identity
/// the label displays — into it instead.
fn select_target<C: 'static, P: Canvas + HasHandler<C>>(
    path: Path,
    label: Id,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    decorate(content, move |p, rect| {
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let label = label.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let picked =
                        command(&event.state.modifiers) && pick(ctx, label.clone());
                    if !picked {
                        select(ctx, target.clone(), None);
                    }
                    true
                }
        });
    })
}

/// A click on a string's text reports what happened — this path, this
/// text-local position — and nothing more; the shell's selection
/// transition decides what it means. One report serves the first
/// click and every one after. With the command modifier and a pending
/// open, picks the atom's identity into it instead.
fn cursor_target<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    path: Path,
    value: Id,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    decorate(content, move |p, rect| {
        let pick = pick.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    if command(&event.state.modifiers) && pick(ctx, value.clone()) {
                        return true;
                    }
                    let click = TextClick {
                        point: Point::new(
                            event.state.position.x - rect.x0,
                            event.state.position.y - rect.y0,
                        ),
                        shift: event.state.modifiers.shift(),
                        count: event.state.count.max(1),
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
            text(tcx, &short_id(id.space()), &styles.id),
            text(tcx, &hex(id.payload()), &styles.id),
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
            recorded: false,
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

        selection.edit_mut().unwrap().set_text("2.5");
        write_through(&mut doc, &mut selection);
        assert_eq!(resolve(&doc, &path), Some(&Id::from(2.5)));

        // Half-typed states leave the last parsed value in place.
        for unparsable in ["2.5e", "", "-", "abc"] {
            selection.edit_mut().unwrap().set_text(unparsable);
            write_through(&mut doc, &mut selection);
            assert_eq!(resolve(&doc, &path), Some(&Id::from(2.5)));
        }

        selection.edit_mut().unwrap().set_text("-3");
        write_through(&mut doc, &mut selection);
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
        selection.edit_mut().unwrap().set_text("new");
        write_through(&mut doc, &mut selection);
        assert_eq!(resolve(&doc, &[Id::from("name")]), Some(&Id::from("new")));
        // A selection without an editor writes nothing.
        let mut plain = Selection::edge(&doc, vec![Id::from("missing")]);
        assert!(!write_through(&mut doc, &mut plain));
        assert_eq!(resolve(&doc, &[Id::from("name")]), Some(&Id::from("new")));
    }

    #[test]
    fn write_through_opens_one_step_per_editor_life() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("a"));
        let mut doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        let mut selection = Selection::edge(&doc, vec![Id::from("name")]);

        // First write opens the step; the rest of the run is silent,
        // as are no-op rewrites.
        selection.edit_mut().unwrap().set_text("ab");
        assert!(write_through(&mut doc, &mut selection));
        selection.edit_mut().unwrap().set_text("abc");
        assert!(!write_through(&mut doc, &mut selection));
        assert!(!write_through(&mut doc, &mut selection));

        // Breaking the run (a save) makes the next write a new step.
        let mut holder = Some(selection);
        break_edit_run(&mut holder);
        let mut selection = holder.unwrap();
        selection.edit_mut().unwrap().set_text("abcd");
        assert!(write_through(&mut doc, &mut selection));

        // A re-minted editor is a new run by construction.
        let mut fresh = Selection::edge(&doc, vec![Id::from("name")]);
        fresh.edit_mut().unwrap().set_text("x");
        assert!(write_through(&mut doc, &mut fresh));
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
        selection.edit_mut().unwrap().set_text("new");
        write_through(&mut doc, &mut selection);
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
    fn pending_insertions_mint_between_neighbors() {
        let mut gid = MutGid::new();
        let items = list(&mut gid, vec![Id::from("a"), Id::from("b")]);
        let doc = Document {
            root: Some(items.clone()),
            gid,
        };
        let positions = positions_of(&doc.gid, &items);
        let first = vec![positions[0].clone()];

        let Some(Selection::Pending { path, .. }) = pending_after(&doc, &first) else {
            panic!("pending after");
        };
        assert!(positions[0] < path[0] && path[0] < positions[1]);

        let Some(Selection::Pending { path, .. }) = pending_before(&doc, &first) else {
            panic!("pending before");
        };
        assert!(path[0] < positions[0]);

        // Into the root list appends at the end; the first variant
        // prepends.
        let Some(Selection::Pending { path, .. }) = pending_into(&doc, &[]) else {
            panic!("pending into");
        };
        assert!(positions[1] < path[0]);
        let Some(Selection::Pending { path, .. }) = pending_into_first(&doc, &[]) else {
            panic!("pending into first");
        };
        assert!(path[0] < positions[0]);

        // A record field has no position to sit beside.
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("x"));
        let doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        assert!(pending_after(&doc, &[Id::from("name")]).is_none());
        // Its value being an atom declines "into", and so does the
        // record itself — a node with only field edges takes no
        // positional element.
        assert!(pending_into(&doc, &[Id::from("name")]).is_none());
        assert!(pending_into(&doc, &[]).is_none());
        assert!(pending_into_first(&doc, &[]).is_none());
        // An empty document offers the root; a rooted one does not.
        assert!(pending_root(&doc).is_none());
        let empty = Document {
            root: None,
            gid: MutGid::new(),
        };
        assert!(matches!(
            pending_root(&empty),
            Some(Selection::Pending { path, .. }) if path.is_empty()
        ));
        // Selecting the empty root pends immediately.
        assert!(matches!(
            Selection::edge(&empty, Vec::new()),
            Selection::Pending { .. }
        ));
    }

    #[test]
    fn secondary_is_the_selected_edges_value_in_any_space() {
        let mut gid = MutGid::new();
        let shared = new_node_id();
        gid.set(shared, Id::from("x"), Id::from(1.0));
        let root = new_node_id();
        gid.set(root, Id::from("a"), Id::from(shared));
        gid.set(root, Id::from("b"), Id::from(shared));
        let doc = Document {
            root: Some(Id::from(root)),
            gid,
        };
        let edge = |path: Vec<Id>| Selection::Edge {
            path,
            edit: None,
            recorded: false,
        };

        assert_eq!(
            secondary_of(&doc, Some(&edge(vec![Id::from("a")]))),
            Some(Id::from(shared))
        );
        // Atoms are identities too — SIDs and NIDs mark like GUIDs.
        assert_eq!(
            secondary_of(&doc, Some(&edge(vec![Id::from("a"), Id::from("x")]))),
            Some(Id::from(1.0))
        );
        // Pendings and no selection don't mark.
        assert_eq!(
            secondary_of(&doc, Some(&pending_value(vec![Id::from("c")]))),
            None
        );
        assert_eq!(secondary_of(&doc, None), None);
    }

    #[test]
    fn pending_edge_targets_nodes_only() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("name"), Id::from("x"));
        let doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        assert!(matches!(
            pending_edge(&doc, Vec::new()),
            Some(Selection::PendingEdge { parent, .. }) if parent.is_empty()
        ));
        // An atom takes no edges; an empty document has no node.
        assert!(pending_edge(&doc, vec![Id::from("name")]).is_none());
        let empty = Document {
            root: None,
            gid: MutGid::new(),
        };
        assert!(pending_edge(&empty, Vec::new()).is_none());
    }

    #[test]
    fn completion_offers_atom_references_and_new_node() {
        let mut gid = MutGid::new();
        for name in ["origin", "corner"] {
            let node = new_node_id();
            gid.set(node, Id::from(NAME), Id::from(name));
        }

        // A confident reference match outranks the typed string;
        // "corner" has no i, so only "origin" matches the query.
        let entries = completion_entries(&gid, None, "orig");
        assert_eq!(entries[0].display, "origin");
        assert!(entries[0].detail.is_some());
        assert_eq!(entries[1].display, "\"orig\"");
        assert!(matches!(&entries[1].action, EntryAction::Value(id) if id.as_str().is_some()));
        assert_eq!(entries.len(), 3);
        assert!(matches!(
            &entries.last().unwrap().action,
            EntryAction::NewNode { name: Some(name) } if name == "orig"
        ));

        // A leading quote forces the string back on top, closed or
        // not, and the new node's name drops the quotes.
        let entries = completion_entries(&gid, None, "\"orig");
        assert!(matches!(&entries[0].action, EntryAction::Value(id) if id.as_str() == Some("orig")));
        assert!(matches!(
            &entries.last().unwrap().action,
            EntryAction::NewNode { name: Some(name) } if name == "orig"
        ));

        // An empty query offers every node — the NAME label is
        // itself an unnamed node here — plus unnamed creation and
        // the empty string.
        let entries = completion_entries(&gid, None, "");
        assert_eq!(entries.len(), 5);
        assert!(matches!(
            &entries.last().unwrap().action,
            EntryAction::NewNode { name: None }
        ));

        // Numbers infer as the atom entry and lead.
        let entries = completion_entries(&gid, None, "2.5");
        assert!(matches!(&entries[0].action, EntryAction::Value(id) if id.as_number() == Some(2.5)));

        // Unnamed nodes are offered too, searchable by the short id
        // they render as — including edgeless ones only referenced
        // as values — and an exact suffix ranks first. A hex suffix
        // can happen to parse as a number ("12345", "12e45") and
        // legitimately cede the lead to the atom, so pick one that
        // doesn't.
        let orphan = std::iter::repeat_with(new_node_id)
            .find(|node| {
                short_id(*node)
                    .trim_start_matches('…')
                    .parse::<f64>()
                    .is_err()
            })
            .unwrap();
        let scratch = new_node_id();
        gid.set(scratch, Id::from("ref"), Id::from(orphan));
        let suffix = short_id(orphan);
        let entries = completion_entries(&gid, None, suffix.trim_start_matches('…'));
        assert_eq!(entries[0].display, suffix);
        assert!(entries[0].detail.is_none());
        assert!(
            matches!(&entries[0].action, EntryAction::Value(id) if id.as_node_id() == Some(orphan))
        );

        // The root's reference is a Document field, not a gid edge -
        // a fresh edgeless node at root is still offered.
        let root = new_node_id();
        let root_id = Id::from(root);
        let entries = completion_entries(&gid, Some(&root_id), "");
        assert!(entries.iter().any(
            |entry| matches!(&entry.action, EntryAction::Value(id) if id.as_node_id() == Some(root))
        ));
    }

    #[test]
    fn commit_pending_mints_named_nodes() {
        let mut gid = MutGid::new();
        let root = new_node_id();
        gid.set(root, Id::from("x"), Id::from(1.0));
        let mut doc = Document {
            root: Some(Id::from(root)),
            gid,
        };
        let path = vec![Id::from("fresh")];
        assert!(commit_pending(
            &mut doc,
            &path,
            &EntryAction::NewNode {
                name: Some("thing".into())
            }
        ));
        let value = resolve(&doc, &path).unwrap().clone();
        assert!(value.as_node_id().is_some());
        assert_eq!(
            doc.gid.get(&value, &Id::from(NAME)),
            Some(&Id::from("thing"))
        );
    }

    #[test]
    fn resolve_query_infers_the_identity() {
        assert_eq!(resolve_query("3.5"), Id::from(3.5));
        assert_eq!(resolve_query(" -2 "), Id::from(-2.0));
        assert_eq!(resolve_query("abc"), Id::from("abc"));
        assert_eq!(resolve_query("\"3.5\""), Id::from("3.5"));
        assert_eq!(resolve_query(""), Id::from(""));
        // A leading quote is string mode even before it closes.
        assert_eq!(resolve_query("\"3.5"), Id::from("3.5"));
        assert_eq!(resolve_query("\""), Id::from(""));
    }

    #[test]
    fn set_value_writes_edges_and_the_root() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("x"), Id::from(1.0));
        let mut doc = Document {
            root: Some(Id::from(node)),
            gid,
        };
        assert!(set_value(&mut doc, &[Id::from("x")], Id::from(2.0)));
        assert_eq!(resolve(&doc, &[Id::from("x")]), Some(&Id::from(2.0)));
        assert!(set_value(&mut doc, &[], Id::from("root")));
        assert_eq!(doc.root, Some(Id::from("root")));
        // A parent that is not a node declines.
        assert!(!set_value(
            &mut doc,
            &[Id::from("x"), Id::from("y")],
            Id::from(0.0)
        ));
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
