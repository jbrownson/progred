//! The raw projection: any document rendered as entity blocks of
//! edge rows, with no schema — every node is just its short id, every
//! edge a row, including `name` — plus the list projection, which
//! renders list values as inline literals or dashed element rows.
//! Atoms render as their values, node ids as git-style suffixes;
//! positions are session bookkeeping and never render at all.

use crate::conventions::{NAME, Name, Names};
use crate::filter;
use crate::sources::Sources;
use progred_graph::{Atom, MutGid, NodeId, Position, Step, Value, new_node_id, position};
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
    /// A node's own name, projected as its header: the strongest text
    /// in a block.
    pub name: TextStyle,
    pub string: TextStyle,
    pub number: TextStyle,
    pub dim: TextStyle,
    /// Byte-identity renderings — short ids — in monospace, so ids
    /// read as ids and align when compared.
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
            name: style(14.0, [0.13, 0.14, 0.16, 1.0], None),
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

/// A rooted graph: the document is its `root` value plus the entity
/// table that stores its maps. Every projection path starts at
/// `root` — typically a map keying the document's parts by role. The
/// root is a location like any other — the empty path — so edits
/// there commit to this field, and deleting it empties the document.
/// Clones are O(1): the gid and list values share structure, which is
/// what makes snapshot undo free.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub root: Option<Value>,
    pub gid: MutGid,
}

/// A small document shaped like a real one. The root is a MAP of
/// roles — a scene holding a shape and a shared style — because a
/// document keys its parts by what they are to it; lists collect
/// same-kind elements only (points, dash lengths, holes). Names name
/// individuals ("roof", not its kind — kinds are a future isa
/// convention's job). The stroke-width field definition FLOATS:
/// referenced as a key, never enumerated. The corner knows its roof
/// (cycle collapse on a real pattern); the style is unnamed and
/// referenced twice (short-id headers, secondary marks).
pub fn sample_document() -> Document {
    let mut gid = MutGid::new();
    let name = Atom::Node(NAME);
    let roof = new_node_id();

    let origin = new_node_id();
    gid.set(origin, name.clone(), Value::from("origin"));
    gid.set(origin, Atom::from("x"), Value::from(0.0));
    gid.set(origin, Atom::from("y"), Value::from(0.0));

    let corner = new_node_id();
    gid.set(corner, name.clone(), Value::from("corner"));
    gid.set(corner, Atom::from("x"), Value::from(4.0));
    gid.set(corner, Atom::from("y"), Value::from(2.5));
    // A part that knows its whole: the cycle a real document has,
    // rendered as a collapsed header rather than recursing forever.
    gid.set(corner, Atom::from("of"), Value::from(roof));

    let stroke_width = new_node_id();
    gid.set(stroke_width, name.clone(), Value::from("stroke-width"));

    let style = new_node_id();
    gid.set(style, Atom::from("color"), Value::from("rebeccapurple"));

    gid.set(roof, name, Value::from("roof"));
    // Lists are values, inline at their edges; the edge that holds
    // one is its name in context.
    gid.set(
        roof,
        Atom::from("points"),
        Value::list([Value::from(origin), Value::from(corner)]),
    );
    gid.set(roof, Atom::Node(stroke_width), Value::from(1.5));
    gid.set(
        roof,
        Atom::from("dash"),
        Value::list([Value::from(2.0), Value::from(3.0)]),
    );
    gid.set(roof, Atom::from("holes"), Value::list([]));
    gid.set(roof, Atom::from("style"), Value::from(style));

    let scene = new_node_id();
    gid.set(scene, Atom::from("shape"), Value::from(roof));
    gid.set(scene, Atom::from("style"), Value::from(style));
    Document {
        root: Some(Value::from(scene)),
        gid,
    }
}

/// Per-path collapse overrides. An absent entry means "use the
/// default", which is collapsed inside a cycle and expanded otherwise;
/// a present entry forces it the other way. Sparse: only overrides are
/// stored.
#[derive(Default)]
pub struct Collapse {
    overrides: std::collections::HashMap<Path, bool>,
}

impl Collapse {
    fn collapsed(&self, path: &[Step], in_cycle: bool) -> bool {
        self.overrides.get(path).copied().unwrap_or(in_cycle)
    }
}

/// Read-only projection context threaded through every view.
struct Cx<'a> {
    /// The reading context: the document read over its library.
    sources: Sources<'a>,
    /// The editor's name policy; every display-name check asks it,
    /// through [`Cx::name`], which derives from the raw bit.
    names: &'a Names,
    /// The Raw view, ONE bit of view state: convention layers derive
    /// from it — names answer None through [`Cx::name`]; domain
    /// projections, when they arrive, stand down through the same
    /// bit. Nothing else is swapped anywhere.
    raw: bool,
    collapse: &'a Collapse,
    styles: &'a RawStyles,
    selection: Option<&'a Selection>,
    /// The value whose other projections carry the secondary mark.
    secondary: Option<Value>,
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
    /// Commit a pointed-at value into the open pending (value or
    /// label stage); false when nothing is pending, so the click
    /// falls through to selection.
    pub pick: Rc<dyn Fn(&mut C, Value) -> bool>,
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
    /// The display name at this projection: the policy's answer,
    /// derived through the raw bit — Raw shows bare identities with
    /// no policy swapped anywhere.
    fn name(&self, node: NodeId) -> Option<Name> {
        (!self.raw).then(|| self.names.of(&self.sources, node))?
    }

    /// Whether `path` carries the primary highlight. A label-stage
    /// pending deliberately does not mark its parent — nothing is
    /// selected there, something is being authored inside; the
    /// pending row carries the highlight itself.
    fn selected(&self, path: &[Step]) -> bool {
        match self.selection {
            Some(Selection::Edge { path: selected, .. })
            | Some(Selection::Pending {
                path: selected, ..
            }) => selected.as_slice() == path,
            _ => false,
        }
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Step]) -> Option<Step> {
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
    fn pending_edge_under(&self, path: &[Step]) -> Option<(&LineEditState, usize)> {
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

/// A location in the projected spanning tree: key steps through map
/// edges, element steps into list values. The same value can be
/// projected at several paths, so the path — not the value — is the
/// identity a selection names. List elements sit at positions
/// sibling edits never move; wraps and unwraps will adjust
/// path-keyed state through one general rewrite — see
/// `docs/model.md`.
pub type Path = Vec<Step>;

/// What is selected: the value at a path, or a nonexistent edge being
/// authored. A selected atom carries its live editor state — every
/// atom is a text editor, focused by selection, and the graph is
/// written through as it edits. A pending selection carries the
/// completion query instead; the query resolves to the value that
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
    pub fn edge(sources: &Sources, path: Path) -> Self {
        if path.is_empty() && sources.root().is_none() {
            return pending_value(path);
        }
        // An editor mounts only where write-through can land: the
        // spine's owning entity must not be external.
        let edit = spine_writable(sources, &path)
            .then(|| {
                sources.resolve(&path).and_then(|value| {
                    value
                        .as_str()
                        .map(|s| line_edit(s, STRING_COLOR))
                        .or_else(|| {
                            value
                                .as_number()
                                .map(|n| line_edit(&n.to_string(), NUMBER_COLOR))
                        })
                })
            })
            .flatten();
        Selection::Edge {
            path,
            edit,
            recorded: false,
        }
    }

    pub fn path(&self) -> &[Step] {
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

/// The path's last Key step: the map edge every write below it lands
/// on. Everything after it is Element steps — a value spine.
fn last_key(path: &[Step]) -> Option<(usize, &Atom)> {
    path.iter()
        .enumerate()
        .rev()
        .find_map(|(index, step)| match step {
            Step::Key(key) => Some((index, key)),
            Step::Element(_) => None,
        })
}

/// Whether a write at `path` can land: the owning entity — the one
/// holding the edge at the path's last Key step — must not be
/// external. A pure element spine above the root is the document's
/// own and always writable.
fn spine_writable(sources: &Sources, path: &[Step]) -> bool {
    match last_key(path) {
        Some((index, _)) => sources
            .resolve(&path[..index])
            .and_then(Value::as_node)
            .is_some_and(|entity| sources.writable(entity)),
        None => true,
    }
}

/// Deletes the value at `path`. A key step is detachment: the value
/// and anything under it stay in the graph for the orphan pool. An
/// element step rebuilds the list value without it, at the owning
/// edge. The empty path empties the document's root; paths that no
/// longer resolve decline.
pub fn delete_edge(doc: &mut Document, library: &MutGid, path: &[Step]) -> bool {
    match path.split_last() {
        None => doc.root.take().is_some(),
        Some((Step::Key(key), parent_path)) => {
            let sources = Sources { doc: &*doc, library };
            let entity = sources
                .resolve(path)
                .and(sources.resolve(parent_path))
                .and_then(Value::as_node)
                .filter(|entity| sources.writable(*entity));
            match entity {
                Some(entity) => {
                    let key = key.clone();
                    doc.gid.delete(entity, &key);
                    true
                }
                None => false,
            }
        }
        Some((Step::Element(position), parent_path)) => {
            let next = {
                let sources = Sources { doc: &*doc, library };
                sources
                    .resolve(parent_path)
                    .and_then(Value::as_list)
                    .filter(|elements| elements.contains_key(position))
                    .map(|elements| Value::List(elements.without(position)))
            };
            match next {
                Some(next) => set_value(doc, library, parent_path, next),
                None => false,
            }
        }
    }
}

/// Where the selection lands after deleting `path`: the next sibling,
/// else the previous, else the parent. Also where a discarded pending
/// edge returns to.
pub fn selection_after_delete(descends: &[Descend], path: &[Step]) -> Path {
    sibling(descends, path, true)
        .or_else(|| sibling(descends, path, false))
        .unwrap_or_else(|| {
            path.split_last()
                .map(|(_, parent)| parent.to_vec())
                .unwrap_or_default()
        })
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
/// Raw's one insertion: a node is a bag of keyed edges, and adding
/// to it means adding an edge. Only nodes qualify — atoms and lists
/// have no edges, structurally. EXTERNAL entities — the library the
/// authority — decline: a lone document edge would shadow the
/// library's facts wholesale (the per-entity fallback), silently
/// de-naming the conventions. A document that owns the entity (a
/// fork, copy/paste's job) authors freely.
pub fn pending_edge(sources: &Sources, parent: Path) -> Option<Selection> {
    let entity = sources.resolve(&parent)?.as_node()?;
    sources.writable(entity).then_some(())?;
    Some(Selection::PendingEdge {
        parent,
        query: line_edit("", QUERY_COLOR),
        choice: 0,
    })
}

/// A pending sibling next to the element at `path` (which must sit at
/// an element step), minted between it and its neighbor. The list
/// projection's gesture.
fn pending_beside(sources: &Sources, path: &[Step], after: bool) -> Option<Selection> {
    let (step, parent_path) = path.split_last()?;
    let Step::Element(position) = step else {
        return None;
    };
    let elements = sources.resolve(parent_path)?.as_list()?;
    // Stated, not incidental: a list under an external entity takes
    // no minted siblings (the write would decline anyway, but a
    // pending that opens and cannot commit is an affordance lie).
    spine_writable(sources, parent_path).then_some(())?;
    let positions: Vec<&Position> = elements.keys().collect();
    let index = positions.iter().position(|p| *p == position)?;
    let fresh = if after {
        position::between(Some(position), positions.get(index + 1).copied())?
    } else {
        position::between(index.checked_sub(1).map(|i| positions[i]), Some(position))?
    };
    let mut fresh_path = parent_path.to_vec();
    fresh_path.push(Step::Element(fresh));
    Some(pending_value(fresh_path))
}

pub fn pending_after(sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_beside(sources, path, true)
}

pub fn pending_before(sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_beside(sources, path, false)
}

/// A pending element inside the list at `path`, appended at the end
/// or prepended at the front. Only lists take elements — by type, not
/// by gate — and the owning entity must be writable, as in
/// [`pending_edge`].
fn pending_into_at(sources: &Sources, path: &[Step], end: bool) -> Option<Selection> {
    let elements = sources.resolve(path)?.as_list()?;
    spine_writable(sources, path).then_some(())?;
    let positions: Vec<&Position> = elements.keys().collect();
    let fresh = if end {
        position::between(positions.last().copied(), None)?
    } else {
        position::between(None, positions.first().copied())?
    };
    let mut fresh_path = path.to_vec();
    fresh_path.push(Step::Element(fresh));
    Some(pending_value(fresh_path))
}

/// Appends: "add to this list" goes at the end — the within chord's
/// meaning on a list, where field edges don't exist.
pub fn pending_into(sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_into_at(sources, path, true)
}

pub fn pending_into_first(sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_into_at(sources, path, false)
}

/// Plain Enter: a new peer BESIDE the selection — continue the
/// enumeration you are in. An element pends a sibling (before with
/// shift); a field value pends a new field on its parent; the root
/// has nothing beside it and falls within — a field edge on a map, an
/// appended element on a list.
pub fn pending_enter(sources: &Sources, path: &[Step], before: bool) -> Option<Selection> {
    let beside = if before {
        pending_before(sources, path)
    } else {
        pending_after(sources, path)
    };
    beside
        .or_else(|| {
            path.split_last()
                .and_then(|(_, parent)| pending_edge(sources, parent.to_vec()))
        })
        .or_else(|| pending_edge(sources, path.to_vec()))
        .or_else(|| pending_into(sources, path))
}

/// The command chord: author WITHIN the selection — a new field edge
/// on the selected node, or, on a list, which has no fields, an
/// element appended at the end. With shift, the front instead —
/// prepend. Atoms have no within and decline.
pub fn pending_insert(sources: &Sources, path: &[Step], front: bool) -> Option<Selection> {
    if front {
        pending_into_first(sources, path)
    } else {
        pending_edge(sources, path.to_vec()).or_else(|| pending_into(sources, path))
    }
}

/// A pending root for an empty document.
pub fn pending_root(sources: &Sources) -> Option<Selection> {
    sources.root().is_none().then(|| pending_value(Vec::new()))
}

/// The value a pending query resolves to: a leading quote forces a
/// string (the closing quote optional, so string mode holds while
/// typing), text that parses is a number, anything else is the string
/// as typed.
pub fn resolve_query(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.strip_prefix('"') {
        Some(inner) => Value::from(inner.strip_suffix('"').unwrap_or(inner)),
        None => trimmed
            .parse::<f64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::from(text)),
    }
}

/// The clipboard spelling of a value — SHALLOW by design: one value,
/// a node reference being its identity alone, no entity edges
/// traveling (deep copy waits on the projection-boundary design; see
/// docs/model.md). Atoms spell as the query language — "quoted"
/// strings, bare numbers — so they read in other apps and
/// [`from_clipboard`] reads them back; nodes and lists spell as
/// Value JSON.
pub fn to_clipboard(value: &Value) -> String {
    match value {
        Value::Atom(Atom::String(_) | Atom::Number(_)) => value.to_string(),
        _ => serde_json::to_string(value).expect("values serialize"),
    }
}

/// The value a clipboard text denotes: Value JSON when it parses,
/// else the query reading — numbers, quoted strings, bare text — so
/// text copied anywhere pastes sensibly.
pub fn from_clipboard(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| resolve_query(text))
}

/// A completion offer on a pending edge. The display styles itself by
/// the action's kind at draw time.
#[derive(Clone)]
pub struct Entry {
    pub display: String,
    pub detail: Option<String>,
    /// Byte spans of `display` the query matched, for highlighting.
    pub matches: Vec<filter::Match>,
    pub action: EntryAction,
}

#[derive(Clone)]
pub enum EntryAction {
    /// Commit this value: an inferred atom or a reference.
    Value(Value),
    /// Mint a node, optionally named, and commit it.
    NewNode { name: Option<String> },
    /// Commit an empty list value. Never named: a list holds elements
    /// only, and is named by the edge that holds it.
    NewList,
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
/// design. The label stage (`labels`) offers atoms only: a key must
/// mean, so "new list" stays a value offer.
fn completion_entries(
    sources: &Sources,
    names: &Names,
    raw: bool,
    labels: bool,
    query: &str,
) -> Vec<Entry> {
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
    // The typed text is always insertable as itself: a numeric query
    // offers its string form right below the number (a quote already
    // states string intent, so quoted queries stay string-only).
    let string_entry = atom.as_number().is_some().then(|| Entry {
        display: format!("\"{query}\""),
        detail: None,
        matches: Vec::new(),
        action: EntryAction::Value(Value::from(query)),
    });
    let atom_entry = Entry {
        display,
        detail: None,
        matches: Vec::new(),
        action: EntryAction::Value(atom),
    };
    // Every node the document contains is referenceable: named ones
    // by name, unnamed ones by the short id they render as — what
    // you see is what you can type. Unnamed keys start with the
    // ellipsis, which sorts after names, so they trail on an empty
    // query. "new list" ranks among them under its own display text:
    // type toward it and it surfaces, type away and it leaves.
    let mut nodes: Vec<(String, EntryAction)> = document_nodes(sources)
        .into_iter()
        .map(|node| {
            let key = (!raw)
                .then(|| names.of(sources, node))
                .flatten()
                .map(|name| name.text)
                .unwrap_or_else(|| short_id(node));
            (key, EntryAction::Value(Value::from(node)))
        })
        .collect();
    nodes.sort_by(|a, b| a.0.cmp(&b.0));
    if !labels {
        nodes.push(("new list".to_string(), EntryAction::NewList));
    }
    let references: Vec<(Entry, bool)> = filter::rank(nodes, |(key, _)| key, query)
        .into_iter()
        .take(8)
        .map(|ranked| {
            let fuzzy = ranked.fuzzy();
            let matches = ranked.matches;
            let (display, action) = ranked.item;
            let detail = match &action {
                EntryAction::Value(value) => value
                    .as_node()
                    .map(short_id)
                    .filter(|detail| *detail != display),
                _ => None,
            };
            let entry = Entry {
                display,
                detail,
                matches,
                action,
            };
            (entry, fuzzy)
        })
        .collect();
    let mut entries = Vec::new();
    if atom_leads {
        entries.push(atom_entry);
        entries.extend(string_entry);
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
        matches: Vec::new(),
        action: EntryAction::NewNode { name },
    });
    entries
}

/// The node references a value carries, lists walked inline —
/// they're values, so their contents are right here.
fn value_nodes(value: &Value, nodes: &mut Vec<NodeId>) {
    match value {
        Value::Atom(atom) => nodes.extend(atom.as_node()),
        Value::List(elements) => {
            for element in elements.values() {
                value_nodes(element, nodes);
            }
        }
    }
}

/// Every node the document or its library contains — entity sources,
/// nodes appearing as keys or inside values (an edgeless node
/// referenced somewhere is still referenceable), and the root, whose
/// reference is a Document field rather than a gid edge. Library
/// nodes are offered so the conventions are typeable from keystroke
/// one. Sorted for a deterministic offer order.
fn document_nodes(sources: &Sources) -> Vec<NodeId> {
    let mut nodes = Vec::new();
    for entity in sources.entities() {
        nodes.push(*entity);
        for (key, value) in sources.edges(*entity).into_iter().flatten() {
            nodes.extend(key.as_node());
            value_nodes(value, &mut nodes);
        }
    }
    if let Some(root) = sources.root() {
        value_nodes(root, &mut nodes);
    }
    nodes.sort();
    nodes.dedup();
    nodes
}

/// Resolves a chosen entry to the value it denotes, minting and
/// naming for a new node. Labels and values resolve alike — the label
/// stage never offers a non-atom action.
pub fn resolve_entry(doc: &mut Document, action: &EntryAction) -> Value {
    match action {
        EntryAction::Value(value) => value.clone(),
        EntryAction::NewNode { name } => {
            let node = new_node_id();
            if let Some(name) = name {
                doc.gid
                    .set(node, Atom::Node(NAME), Value::from(name.as_str()));
            }
            Value::from(node)
        }
        EntryAction::NewList => Value::list([]),
    }
}

/// Commits a pending edge from a chosen entry: resolves the action to
/// a value and writes it.
pub fn commit_pending(
    doc: &mut Document,
    library: &MutGid,
    path: &[Step],
    action: &EntryAction,
) -> bool {
    let value = resolve_entry(doc, action);
    set_value(doc, library, path, value)
}

/// Rebuilds the value spine along `elements` (element steps only),
/// replacing the leaf: surviving elements keep their positions, and
/// the final step inserts or replaces at its position. Deeper steps
/// need an existing element to descend through.
fn respine(current: Option<&Value>, elements: &[Step], leaf: Value) -> Option<Value> {
    match elements.split_first() {
        None => Some(leaf),
        Some((Step::Element(position), rest)) => {
            let list = current?.as_list()?;
            let child = list.get(position);
            if !rest.is_empty() && child.is_none() {
                return None;
            }
            let rebuilt = respine(child, rest, leaf)?;
            Some(Value::List(list.update(position.clone(), rebuilt)))
        }
        Some((Step::Key(_), _)) => None,
    }
}

/// Writes `value` at `path` — the empty path writes the document
/// root. The single write every edit reduces to: the path's last Key
/// step names the owning, authority-gated entity edge; the element
/// steps below it are a value spine, rebuilt around the new leaf.
pub fn set_value(doc: &mut Document, library: &MutGid, path: &[Step], value: Value) -> bool {
    let write = {
        let sources = Sources { doc: &*doc, library };
        match last_key(path) {
            Some((index, key)) => sources
                .resolve(&path[..index])
                .and_then(Value::as_node)
                .filter(|entity| sources.writable(*entity))
                .and_then(|entity| {
                    let current = sources.get(entity, key);
                    respine(current, &path[index + 1..], value)
                        .map(|rebuilt| (Some((entity, key.clone())), rebuilt))
                }),
            None => respine(sources.root(), path, value).map(|rebuilt| (None, rebuilt)),
        }
    };
    match write {
        Some((Some((entity, key)), rebuilt)) => {
            doc.gid.set(entity, key, rebuilt);
            true
        }
        Some((None, rebuilt)) => {
            doc.root = Some(rebuilt);
            true
        }
        None => false,
    }
}

/// Toggle the collapse override for the value at `path`, staying
/// sparse: an override matching the default (collapsed inside a
/// cycle, expanded otherwise) is removed rather than stored. Declines
/// unless there is something to collapse — a node with edges, or a
/// list with elements.
pub fn toggle_collapse(sources: &Sources, collapse: &mut Collapse, path: &[Step]) -> bool {
    sources
        .resolve(path)
        .filter(|value| match value {
            Value::Atom(atom) => atom
                .as_node()
                .and_then(|node| sources.edges(node))
                .is_some_and(|edges| !edges.is_empty()),
            Value::List(elements) => !elements.is_empty(),
        })
        .map(|value| {
            let in_cycle = (0..path.len())
                .filter_map(|end| sources.resolve(&path[..end]))
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

/// Writes the selection's editor text through to its location after
/// every handled event — the graph is the source of truth. The edited
/// kind follows the current value: strings write every keystroke;
/// numbers only when the text parses, since a half-typed state like
/// `3.` has no value to write. Everything funnels through
/// [`set_value`], so an element edit rebuilds its list at the owning
/// edge and a location that no longer takes the write drops it
/// silently — the malformed-graph rule at the mutation boundary.
/// Returns whether this write OPENED an undo step: true exactly on
/// the first write of the mounted editor's life, so a typing run is
/// one step and history stays a dumb stack.
pub fn write_through(doc: &mut Document, library: &MutGid, selection: &mut Selection) -> bool {
    let Selection::Edge {
        path,
        edit,
        recorded,
    } = selection
    else {
        return false;
    };
    let Some(edit) = edit else {
        return false;
    };
    let text = edit.text().to_string();
    let (current, next) = {
        let sources = Sources { doc: &*doc, library };
        let current = sources.resolve(path);
        let next = match current {
            Some(Value::Atom(Atom::String(_))) => Some(Value::from(text)),
            Some(Value::Atom(Atom::Number(_))) => {
                text.trim().parse::<f64>().ok().map(Value::from)
            }
            _ => None,
        };
        (current.cloned(), next)
    };
    if let Some(next) = next
        && current.as_ref() != Some(&next)
        && set_value(doc, library, path, next)
    {
        let first = !*recorded;
        *recorded = true;
        return first;
    }
    false
}

/// Breaks the open edit run: the next write records a fresh undo
/// step. Called after a save, so a run never straddles the mark.
pub fn break_edit_run(selection: Option<&mut Selection>) {
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
    /// The settled rect, for scroll-to-selection.
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

/// The neighboring sibling, continuing through ancestors at the
/// ends: past the last child, Down flows to the enclosing next
/// sibling (and Up mirrors), instead of dead-ending.
fn sibling(descends: &[Descend], path: &[Step], next: bool) -> Option<Path> {
    let mut path = path.to_vec();
    loop {
        let (_, parent) = path.split_last()?;
        let siblings: Vec<&Path> = descends
            .iter()
            .map(|descend| &descend.path)
            .filter(|p| p.split_last().is_some_and(|(_, prefix)| prefix == parent))
            .collect();
        let index = siblings.iter().position(|p| **p == path)?;
        let neighbor = if next {
            siblings.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|index| siblings.get(index))
        };
        match neighbor {
            Some(found) => return Some((*found).clone()),
            None => {
                path.pop();
            }
        }
    }
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
    value: Option<Value>,
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
                            .is_some_and(|value| pick(ctx, value.clone()));
                    if !picked {
                        select(ctx, target.clone(), None);
                    }
                    true
                }
        });
        p.descends().push(Descend { path, rect });
    })
}

/// The value marked as the secondary selection: the one at the end
/// of the selected edge. A value can project in many places — node
/// references, but equally strings, numbers, and equal lists — and
/// the marks make that sameness visible.
fn secondary_of(sources: &Sources, selection: Option<&Selection>) -> Option<Value> {
    match selection? {
        Selection::Edge { path, .. } => sources.resolve(path).cloned(),
        _ => None,
    }
}

// The explicit-state boundary: everything a pass reads arrives here.
#[allow(clippy::too_many_arguments)]
pub fn project<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    sources: &Sources,
    selection: Option<&Selection>,
    graph_node: Option<&Value>,
    collapse: &Collapse,
    names: &Names,
    raw: bool,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    hooks: Hooks<C>,
) -> Node<P> {
    let cx = Cx {
        sources: *sources,
        names,
        raw,
        collapse,
        styles,
        selection,
        // The graph view's selected node is a secondary here too:
        // its projections are the same value.
        secondary: secondary_of(sources, selection).or_else(|| graph_node.cloned()),
    };
    // The Raw view derives from the one bit: names answer None and
    // nothing else changes — lists render as lists there too, since
    // kind is data, not convention. An empty document is a
    // selectable placeholder at the root path.
    match sources.root() {
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
    path: &[Step],
    ancestors: &HashSet<NodeId>,
    node: NodeId,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let name = cx.name(node);
    let name_label = name.as_ref().and_then(|name| name.label.clone());
    // The name edge is consumed by the header, so the listing skips it.
    let mut entries: Vec<(Atom, Option<Value>)> = sorted_edges(&cx.sources, node)
        .into_iter()
        .filter(|(key, _)| name_label.as_ref() != Some(key))
        .map(|(key, value)| (key, Some(value)))
        .collect();
    if let Some(Step::Key(key)) = cx.pending_child_of(path) {
        entries.push((key, None));
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let head = head_view(cx, tcx, path, node, &name, hooks);

    let pending_edge = cx.pending_edge_under(path).is_some();
    if entries.is_empty() && !pending_edge {
        return head;
    }
    // A pending child or edge forces the node open so it can be seen.
    let collapsed = !pending_edge
        && entries.iter().all(|(_, value)| value.is_some())
        && cx.collapse.collapsed(path, ancestors.contains(&node));
    let header = row(
        4.0 * scale,
        vec![head, disclosure(path.to_vec(), collapsed, hooks, cx.styles)],
    );
    if collapsed {
        return header;
    }

    let mut inner = ancestors.clone();
    inner.insert(node);
    let mut rows: Vec<Node<P>> = entries
        .into_iter()
        .map(|(key, value)| edge_row(cx, tcx, path, &inner, key, value, hooks))
        .collect();
    // A new edge being authored: the label query, unsorted until it
    // has a label to sort by.
    if let Some((query, choice)) = cx.pending_edge_under(path) {
        rows.push(pending_edge_row(cx, tcx, query, choice, hooks));
    }
    block(header, rows, cx.styles)
}

/// A named node's head: the consumed name edge projected as the
/// header — the text is the name, and it selects, edits, marks, and
/// deletes as the edge it is, replacing that edge's ordinary row.
/// An unnamed node heads with its short id; a name without an edge
/// (computed) is plain text.
///
/// The name text stands for the NODE until the node is selected: a
/// cold click falls through to the block's own target — click the
/// thing to select it, click again to rename — and only then does
/// the name engage as a text target. The pass decides from current
/// state; single-shot dispatch means the second click always sees
/// the engaged successor. Cold, the edge stays keyboard-reachable
/// and markable, just not a pointer target.
fn head_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    node: NodeId,
    name: &Option<Name>,
    hooks: &Hooks<C>,
) -> Node<P> {
    let Some(name) = name else {
        return text(tcx, &short_id(node), &cx.styles.id);
    };
    let fallback = text(tcx, &name.text, &cx.styles.name);
    let Some(label) = &name.label else {
        return fallback;
    };
    let Some(value) = cx.sources.get(node, label).cloned() else {
        return fallback;
    };
    let mut edge = path.to_vec();
    edge.push(Step::Key(label.clone()));
    let editing = cx
        .selection
        .filter(|selection| selection.path() == edge.as_slice())
        .and_then(Selection::edit);
    let content = atom_content(editing, fallback, tcx, cx.styles, hooks);
    if cx.selected(path) || cx.selected(&edge) {
        let content = cursor_target(edge.clone(), value.clone(), hooks, content);
        let content = if cx.selected(&edge) {
            content
        } else {
            secondary_mark(cx, &value, content)
        };
        descend(cx, edge, Some(value), hooks, content)
    } else {
        let content = secondary_mark(cx, &value, content);
        decorate(content, move |p: &mut P, rect| {
            p.descends().push(Descend { path: edge, rect });
        })
    }
}

/// A node block: the header over its indented rows.
fn block<P: Canvas>(header: Node<P>, rows: Vec<Node<P>>, styles: &RawStyles) -> Node<P> {
    let scale = styles.scale;
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

/// One keyed edge row: the label-and-arrow head, then the value (or
/// its pending query). A real edge's label and arrow select the edge,
/// like its value — grouped so one target spans both and the gap
/// between. A pending row's plain click falls through (the
/// not-yet-edge can't be selected), but command still picks its
/// label's identity.
fn edge_row<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<NodeId>,
    key: Atom,
    value: Option<Value>,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let mut child = path.to_vec();
    child.push(Step::Key(key.clone()));
    let head = row(6.0 * scale, vec![label_view(cx, tcx, &key), arrow(cx.styles)]);
    let head = match &value {
        Some(_) => select_target(child.clone(), key.clone(), hooks, head),
        None => pick_target(key.clone(), hooks, head),
    };
    let content = match value {
        Some(value) => value_view(cx, tcx, &child, ancestors, &value, hooks),
        None => pending_view(cx, tcx, child, hooks),
    };
    row(6.0 * scale, vec![head, content])
}

/// The label-query row of a new edge being authored on a node. The
/// authoring locus carries the primary itself; its parent is
/// deliberately unmarked.
fn pending_edge_row<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    choice: usize,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let pending_row = row(
        6.0 * scale,
        vec![
            query_content(cx, tcx, query, choice, true, hooks),
            arrow(cx.styles),
            text(tcx, "…", &cx.styles.dim),
        ],
    );
    decorate(pending_row, move |p: &mut P, rect| {
        primary_highlight(scale, p, rect);
        // The row owns its clicks: nothing here means "select the
        // parent", so nothing may fall through to it. (The query's
        // caret target, registered after, still wins inside itself.)
        p.handler().on_pointer_down(move |_, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
    })
}

/// A list value: its elements as bare ordered rows — the position is
/// session identity, not information; order carries it. An atom-only
/// list reads as an inline literal; collapsed shows the element
/// count. Lists have no identity, so there is no header id and no
/// cycle through them — only their node elements can recurse.
fn list_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<NodeId>,
    elements: &im::OrdMap<Position, Value>,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let mut items: Vec<(Position, Option<Value>)> = elements
        .iter()
        .map(|(position, value)| (position.clone(), Some(value.clone())))
        .collect();
    if let Some(Step::Element(position)) = cx.pending_child_of(path) {
        items.push((position, None));
        items.sort_by(|a, b| a.0.cmp(&b.0));
    }

    // An atom list reads as a literal: `[1, "two", 3]` on one line,
    // dim punctuation, each element still an ordinary descend (click,
    // edit, mark) — a pending one included. Node elements and nested
    // lists take the block form; width-aware grouping (Wadler) waits,
    // so a long list will run wide for now.
    let inline = items.iter().all(|(_, value)| {
        value
            .as_ref()
            .is_none_or(|v| v.as_str().is_some() || v.as_number().is_some())
    });
    if inline {
        let mut cells: Vec<Node<P>> = vec![text(tcx, "[", &cx.styles.dim)];
        for (index, (position, value)) in items.into_iter().enumerate() {
            if index > 0 {
                cells.push(text(tcx, ", ", &cx.styles.dim));
            }
            let mut child = path.to_vec();
            child.push(Step::Element(position));
            cells.push(match value {
                Some(value) => value_view(cx, tcx, &child, ancestors, &value, hooks),
                None => pending_view(cx, tcx, child, hooks),
            });
        }
        cells.push(text(tcx, "]", &cx.styles.dim));
        return row(0.0, cells);
    }

    // A pending child forces the list open so it can be seen. Lists
    // have no identity to be an ancestor, so only an override
    // collapses one.
    let collapsed = items.iter().all(|(_, value)| value.is_some())
        && cx.collapse.collapsed(path, false);
    let head = text(tcx, "[", &cx.styles.dim);
    let mut header = vec![head, disclosure(path.to_vec(), collapsed, hooks, cx.styles)];
    if collapsed {
        let count = format!(
            "{} element{}",
            items.len(),
            if items.len() == 1 { "" } else { "s" }
        );
        header.push(text(tcx, &count, &cx.styles.dim));
        header.push(text(tcx, "]", &cx.styles.dim));
        return row(4.0 * scale, header);
    }
    let rows: Vec<Node<P>> = items
        .into_iter()
        .map(|(position, value)| {
            let mut child = path.to_vec();
            child.push(Step::Element(position));
            let content = match value {
                Some(value) => value_view(cx, tcx, &child, ancestors, &value, hooks),
                None => pending_view(cx, tcx, child, hooks),
            };
            // The list vernacular: a dim leading dash marks the
            // element rows.
            row(6.0 * scale, vec![text(tcx, "-", &cx.styles.dim), content])
        })
        .collect();
    // The block form closes its bracket at the header's indent.
    col(
        HAlign::Start,
        0,
        4.0 * scale,
        vec![
            row(4.0 * scale, header),
            pad(
                Insets::new(26.0 * scale, 0.0, 0.0, 0.0),
                col(HAlign::Start, 0, 4.0 * scale, rows),
            ),
            text(tcx, "]", &cx.styles.dim),
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
fn sorted_edges(sources: &Sources, node: NodeId) -> Vec<(Atom, Value)> {
    let mut edges: Vec<(Atom, Value)> = sources
        .edges(node)
        .map(|edges| edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    edges.sort();
    edges
}

fn label_view<P: Canvas>(cx: &Cx, tcx: &mut TextCtx, key: &Atom) -> Node<P> {
    let inner = match key {
        Atom::String(s) => text(tcx, s, &cx.styles.label),
        Atom::Number(n) => text(tcx, &n.get().to_string(), &cx.styles.number),
        // A named node used as a key reads by its name, through the
        // editor's one name policy.
        Atom::Node(node) => match cx.name(*node) {
            Some(name) => text(tcx, &name.text, &cx.styles.label),
            None => text(tcx, &short_id(*node), &cx.styles.id),
        },
    };
    secondary_mark(cx, &Value::Atom(key.clone()), inner)
}

/// A node projection's ground, painted only at authority
/// TRANSITIONS: an external entity under document authority takes
/// the dark tint — no lock, just "from elsewhere" — and a
/// document-authority entity under an external one takes its light
/// ground back (opaque, since an alpha wash can't be undone by
/// another wash). Runs of the same authority draw nothing, so
/// nesting never stacks tints. The enclosing authority is the
/// spine's owning entity, so a node inside a list carries its list's
/// owner as context. Wraps outside the descend so the node's own
/// selection highlight draws over its ground.
fn ground<P: Canvas>(cx: &Cx, path: &[Step], value: &Value, content: Node<P>) -> Node<P> {
    let Some(node) = value.as_node() else {
        return content;
    };
    let external = cx.sources.external(node);
    let parent_external = last_key(path)
        .and_then(|(index, _)| cx.sources.resolve(&path[..index]))
        .and_then(Value::as_node)
        .is_some_and(|entity| cx.sources.external(entity));
    if external == parent_external {
        return content;
    }
    let scale = cx.styles.scale;
    let color = if external {
        Color::new([0.13, 0.14, 0.16, 0.05])
    } else {
        Color::new([0.965, 0.965, 0.972, 1.0])
    };
    decorate(content, move |p: &mut P, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, color, Affine::IDENTITY);
    })
}

/// The secondary selection's mark: a subtle wash over another whole
/// projection of the selected edge's value — an expanded block, a
/// collapsed header, or a label. The primary selection's geometry at
/// lower strength, so the two read as one family.
fn secondary_mark<P: Canvas>(cx: &Cx, value: &Value, content: Node<P>) -> Node<P> {
    if cx.secondary.as_ref() != Some(value) {
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
/// "label → value", and being a stroke rather than text, it
/// separates an edge's key from its target.
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
    path: &[Step],
    ancestors: &HashSet<NodeId>,
    value: &Value,
    hooks: &Hooks<C>,
) -> Node<P> {
    let editing = cx
        .selection
        .filter(|selection| selection.path() == path)
        .and_then(Selection::edit);
    let inner = match value {
        Value::Atom(Atom::String(s)) => {
            let fallback = text(tcx, s, &cx.styles.string);
            let content = atom_content(editing, fallback, tcx, cx.styles, hooks);
            row(0.0, vec![
                text(tcx, "\"", &cx.styles.string),
                cursor_target(path.to_vec(), value.clone(), hooks, content),
                text(tcx, "\"", &cx.styles.string),
            ])
        }
        Value::Atom(Atom::Number(n)) => {
            let fallback = text(tcx, &n.get().to_string(), &cx.styles.number);
            let content = atom_content(editing, fallback, tcx, cx.styles, hooks);
            cursor_target(path.to_vec(), value.clone(), hooks, content)
        }
        // The hardcoded projection chain, decided per value: lists
        // render as lists — in the Raw view too, the kind being data
        // — nodes as raw blocks. A registry waits for user-defined
        // projections.
        Value::Atom(Atom::Node(node)) => node_view(cx, tcx, path, ancestors, *node, hooks),
        Value::List(elements) => list_view(cx, tcx, path, ancestors, elements, hooks),
    };
    // Other projections of the selected edge's value carry the
    // secondary mark; the selected one has the primary highlight.
    let inner = if cx.selected(path) {
        inner
    } else {
        secondary_mark(cx, value, inner)
    };
    let placed = descend(cx, path.to_vec(), Some(value.clone()), hooks, inner);
    ground(cx, path, value, placed)
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
            query_content(cx, tcx, query, *choice, false, hooks)
        }
        _ => text(tcx, "…", &cx.styles.dim),
    };
    descend(cx, path, None, hooks, content)
}

/// A focused completion query: the editor plus its popup, emitted at
/// placement for the shell to draw over the body. Serves both pending
/// stages — a value and a new edge's label (`labels` narrows the
/// offers to atoms there).
fn query_content<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    choice: usize,
    labels: bool,
    hooks: &Hooks<C>,
) -> Node<P> {
    let entries = completion_entries(&cx.sources, cx.names, cx.raw, labels, query.text());
    let fallback = text(tcx, "…", &cx.styles.dim);
    let content = atom_content(Some(query), fallback, tcx, cx.styles, hooks);
    let edit = hooks.edit.clone();
    let scale = cx.styles.scale;
    decorate(content, move |p: &mut P, rect| {
        *p.popup() = Some(Popup {
            anchor: rect,
            entries,
            choice,
        });
        // Clicks in the query place the caret, straight through the
        // edit hook — the selection transition is never involved, so
        // clicking what you are typing can't discard it.
        let edit = edit.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && edit(ctx).is_some_and(|edit| {
                    edit.state.pointer_down(
                        edit.fonts,
                        edit.layouts,
                        scale as f32,
                        Point::new(
                            event.state.position.x - rect.x0,
                            event.state.position.y - rect.y0,
                        ),
                        event.state.modifiers.shift(),
                        event.state.count.max(1),
                    );
                    true
                })
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
    // Cells first, so rows can pad out to the widest and the chosen
    // highlight spans the card, not just its own content.
    let cells: Vec<(Node<P>, Option<Node<P>>)> = popup
        .entries
        .iter()
        .map(|entry| {
            let style = match &entry.action {
                EntryAction::Value(value) if value.as_str().is_some() => &styles.string,
                EntryAction::Value(value) if value.as_number().is_some() => &styles.number,
                EntryAction::Value(_) => &styles.label,
                EntryAction::NewNode { .. } | EntryAction::NewList => &styles.dim,
            };
            let display = highlighted(tcx, &entry.display, &entry.matches, style);
            let detail = entry
                .detail
                .as_ref()
                .map(|detail| text(tcx, detail, &styles.id));
            (display, detail)
        })
        .collect();
    let widths: Vec<f64> = cells
        .iter()
        .map(|(display, detail)| {
            display.extent.width
                + detail
                    .as_ref()
                    .map_or(0.0, |detail| 8.0 * scale + detail.extent.width)
        })
        .collect();
    let max_width = widths.iter().copied().fold(0.0, f64::max);
    let rows: Vec<Node<P>> = cells
        .into_iter()
        .zip(widths)
        .enumerate()
        .map(|(index, ((display, detail), width))| {
            let mut cells: Vec<Node<P>> = vec![display];
            if let Some(detail) = detail {
                cells.push(detail);
            }
            let content = pad(
                Insets::new(
                    8.0 * scale,
                    2.0 * scale,
                    8.0 * scale + (max_width - width),
                    2.0 * scale,
                ),
                row(8.0 * scale, cells),
            );
            let chosen = index == choice;
            let action = popup.entries[index].action.clone();
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

/// Entry text with the query's matched spans in bold — the fuzzy
/// filter's byte offsets drawn, not recomputed.
fn highlighted<P: Canvas>(
    tcx: &mut TextCtx,
    s: &str,
    matches: &[filter::Match],
    style: &TextStyle,
) -> Node<P> {
    if matches.is_empty() {
        return text(tcx, s, style);
    }
    let bold = TextStyle {
        weight: Some(700.0),
        ..style.clone()
    };
    let mut segments: Vec<Node<P>> = Vec::new();
    let mut at = 0;
    for span in matches {
        if span.start > at {
            segments.push(text(tcx, &s[at..span.start], style));
        }
        segments.push(text(tcx, &s[span.start..span.start + span.len], &bold));
        at = span.start + span.len;
    }
    if at < s.len() {
        segments.push(text(tcx, &s[at..], style));
    }
    row(0.0, segments)
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
    key: Atom,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let pick = hooks.pick.clone();
    decorate(content, move |p, rect| {
        let pick = pick.clone();
        let key = key.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && command(&event.state.modifiers)
                && pick(ctx, Value::Atom(key.clone()))
        });
    })
}

/// A plain click-to-select target for `path` — for edge parts like
/// labels that select without carrying an editor click. With the
/// command modifier and a pending open, picks `key` — the value the
/// label displays — into it instead.
fn select_target<C: 'static, P: Canvas + HasHandler<C>>(
    path: Path,
    key: Atom,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    decorate(content, move |p, rect| {
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let key = key.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let picked = command(&event.state.modifiers)
                        && pick(ctx, Value::Atom(key.clone()));
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
/// open, picks the atom's value into it instead.
fn cursor_target<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    path: Path,
    value: Value,
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

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::Gid;
    use ui_events::keyboard::{KeyState, Modifiers};

    fn src<'a>(doc: &'a Document, library: &'a MutGid) -> Sources<'a> {
        Sources { doc, library }
    }

    fn key(s: &str) -> Step {
        Step::Key(Atom::from(s))
    }

    /// The ordered positions of a list value's elements.
    fn positions(value: &Value) -> Vec<Position> {
        value.as_list().unwrap().keys().cloned().collect()
    }

    fn descends(paths: &[Vec<&str>]) -> Vec<Descend> {
        paths
            .iter()
            .map(|path| Descend {
                path: path.iter().map(|s| key(s)).collect(),
                rect: Rect::ZERO,
            })
            .collect()
    }

    fn arrow(named: NamedKey) -> KeyboardEvent {
        KeyboardEvent {
            key: Key::Named(named),
            state: KeyState::Down,
            modifiers: Modifiers::empty(),
            ..Default::default()
        }
    }

    fn stepped(ds: &[Descend], from: Option<&[&str]>, named: NamedKey) -> Option<Path> {
        let selection = from.map(|p| Selection::Edge {
            path: p.iter().map(|s| key(s)).collect(),
            edit: None,
            recorded: false,
        });
        step_selection(ds, selection.as_ref(), &arrow(named))
    }

    #[test]
    fn arrows_step_selection_through_the_tree() {
        // Placement order: pre-order, parents before children.
        let ds = descends(&[vec![], vec!["a"], vec!["a", "x"], vec!["a", "y"], vec!["b"]]);
        let path = |p: &[&str]| p.iter().map(|s| key(s)).collect::<Vec<_>>();
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
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("name"), Value::from("old"));
        gid.set(node, Atom::from("x"), Value::from(1.5));
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let at = |labels: &[&str]| {
            Selection::edge(&src(&doc, &lib), labels.iter().map(|s| key(s)).collect())
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
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("x"), Value::from(1.5));
        let mut doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let path = vec![key("x")];
        let mut selection = Selection::edge(&src(&doc, &lib), path.clone());

        selection.edit_mut().unwrap().set_text("2.5");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(src(&doc, &lib).resolve(&path), Some(&Value::from(2.5)));

        // Half-typed states leave the last parsed value in place.
        for unparsable in ["2.5e", "", "-", "abc"] {
            selection.edit_mut().unwrap().set_text(unparsable);
            write_through(&mut doc, &lib, &mut selection);
            assert_eq!(src(&doc, &lib).resolve(&path), Some(&Value::from(2.5)));
        }

        selection.edit_mut().unwrap().set_text("-3");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(src(&doc, &lib).resolve(&path), Some(&Value::from(-3.0)));
    }

    #[test]
    fn edits_write_through_to_the_edge() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("name"), Value::from("old"));
        let mut doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let mut selection = Selection::edge(&src(&doc, &lib), vec![key("name")]);
        selection.edit_mut().unwrap().set_text("new");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(src(&doc, &lib).resolve(&[key("name")]), Some(&Value::from("new")));
        // A selection without an editor writes nothing.
        let mut plain = Selection::edge(&src(&doc, &lib), vec![key("missing")]);
        assert!(!write_through(&mut doc, &lib, &mut plain));
        assert_eq!(src(&doc, &lib).resolve(&[key("name")]), Some(&Value::from("new")));
    }

    #[test]
    fn element_edits_rebuild_the_list_at_its_edge() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(
            node,
            Atom::from("dash"),
            Value::list([Value::from(2.0), Value::from(3.0)]),
        );
        let mut doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let list_path = vec![key("dash")];
        let ps = positions(src(&doc, &lib).resolve(&list_path).unwrap());
        let element = vec![key("dash"), Step::Element(ps[1].clone())];

        // Editing an element writes the whole rebuilt list at the
        // owning edge; the sibling keeps its position and value.
        let mut selection = Selection::edge(&src(&doc, &lib), element.clone());
        selection.edit_mut().unwrap().set_text("9");
        assert!(write_through(&mut doc, &lib, &mut selection));
        assert_eq!(src(&doc, &lib).resolve(&element), Some(&Value::from(9.0)));
        assert_eq!(
            src(&doc, &lib).resolve(&list_path),
            Some(&Value::list([Value::from(2.0), Value::from(9.0)]))
        );
        assert_eq!(positions(src(&doc, &lib).resolve(&list_path).unwrap()), ps);
    }

    #[test]
    fn set_value_writes_edges_elements_and_the_root() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("x"), Value::from(1.0));
        let mut doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        assert!(set_value(&mut doc, &lib, &[key("x")], Value::from(2.0)));
        assert_eq!(src(&doc, &lib).resolve(&[key("x")]), Some(&Value::from(2.0)));

        // A fresh element step INSERTS at its position — commit's
        // shape — and a deep spine rebuilds through nested lists.
        let inner = Value::list([Value::from(1.0)]);
        gid_set(&mut doc, node, "items", Value::list([inner]));
        let items = src(&doc, &lib).resolve(&[key("items")]).unwrap().clone();
        let outer = positions(&items);
        let inner_positions = positions(items.as_list().unwrap().values().next().unwrap());
        let deep = vec![
            key("items"),
            Step::Element(outer[0].clone()),
            Step::Element(inner_positions[0].clone()),
        ];
        assert!(set_value(&mut doc, &lib, &deep, Value::from(7.0)));
        assert_eq!(src(&doc, &lib).resolve(&deep), Some(&Value::from(7.0)));
        let fresh = position::between(Some(&outer[0]), None).unwrap();
        let appended = vec![key("items"), Step::Element(fresh.clone())];
        assert!(set_value(&mut doc, &lib, &appended, Value::from("tail")));
        assert_eq!(src(&doc, &lib).resolve(&appended), Some(&Value::from("tail")));
        // A deeper step under a missing element declines.
        let gone = position::between(Some(&fresh), None).unwrap();
        assert!(!set_value(
            &mut doc,
            &lib,
            &[key("items"), Step::Element(gone), Step::Element(fresh)],
            Value::from(0.0)
        ));

        assert!(set_value(&mut doc, &lib, &[], Value::from("root")));
        assert_eq!(doc.root, Some(Value::from("root")));
        // A parent that is not a node declines.
        assert!(!set_value(&mut doc, &lib, &[key("x"), key("y")], Value::from(0.0)));
    }

    fn gid_set(doc: &mut Document, entity: NodeId, label: &str, value: Value) {
        doc.gid.set(entity, Atom::from(label), value);
    }

    #[test]
    fn write_through_opens_one_step_per_editor_life() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("name"), Value::from("a"));
        let mut doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let mut selection = Selection::edge(&src(&doc, &lib), vec![key("name")]);

        // First write opens the step; the rest of the run is silent,
        // as are no-op rewrites.
        selection.edit_mut().unwrap().set_text("ab");
        assert!(write_through(&mut doc, &lib, &mut selection));
        selection.edit_mut().unwrap().set_text("abc");
        assert!(!write_through(&mut doc, &lib, &mut selection));
        assert!(!write_through(&mut doc, &lib, &mut selection));

        // Breaking the run (a save) makes the next write a new step.
        break_edit_run(Some(&mut selection));
        selection.edit_mut().unwrap().set_text("abcd");
        assert!(write_through(&mut doc, &lib, &mut selection));

        // A re-minted editor is a new run by construction.
        let mut fresh = Selection::edge(&src(&doc, &lib), vec![key("name")]);
        fresh.edit_mut().unwrap().set_text("x");
        assert!(write_through(&mut doc, &lib, &mut fresh));
    }

    #[test]
    fn delete_edge_detaches_and_the_root_empties_the_document() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let root = new_node_id();
        let child = new_node_id();
        gid.set(root, Atom::from("child"), Value::from(child));
        gid.set(child, Atom::from("name"), Value::from("c"));
        let mut doc = Document {
            root: Some(Value::from(root)),
            gid,
        };

        assert!(!delete_edge(&mut doc, &lib, &[key("missing")]));

        assert!(delete_edge(&mut doc, &lib, &[key("child")]));
        assert_eq!(src(&doc, &lib).resolve(&[key("child")]), None);
        // Detachment, not destruction: the orphan keeps its edges.
        assert!(doc.gid.edges(child).is_some());
        // Already gone: declines.
        assert!(!delete_edge(&mut doc, &lib, &[key("child")]));

        assert!(delete_edge(&mut doc, &lib, &[]));
        assert_eq!(doc.root, None);
        assert_eq!(src(&doc, &lib).resolve(&[]), None);
        assert!(!delete_edge(&mut doc, &lib, &[]));
    }

    #[test]
    fn deleting_an_element_rebuilds_the_list_without_it() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(
            node,
            Atom::from("dash"),
            Value::list([Value::from(2.0), Value::from(3.0)]),
        );
        let mut doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let list_path = vec![key("dash")];
        let ps = positions(src(&doc, &lib).resolve(&list_path).unwrap());
        let first = vec![key("dash"), Step::Element(ps[0].clone())];

        assert!(delete_edge(&mut doc, &lib, &first));
        assert_eq!(
            src(&doc, &lib).resolve(&list_path),
            Some(&Value::list([Value::from(3.0)]))
        );
        // The survivor keeps its position; the deleted one is a
        // stale path now and declines again.
        assert_eq!(
            positions(src(&doc, &lib).resolve(&list_path).unwrap()),
            vec![ps[1].clone()]
        );
        assert!(!delete_edge(&mut doc, &lib, &first));

        // Emptying the list leaves the empty list, not nothing.
        let second = vec![key("dash"), Step::Element(ps[1].clone())];
        assert!(delete_edge(&mut doc, &lib, &second));
        assert_eq!(src(&doc, &lib).resolve(&list_path), Some(&Value::list([])));
    }

    #[test]
    fn root_edits_commit_to_the_document_root() {
        let lib = MutGid::new();
        let mut doc = Document {
            root: Some(Value::from("old")),
            gid: MutGid::new(),
        };
        let mut selection = Selection::edge(&src(&doc, &lib), vec![]);
        selection.edit_mut().unwrap().set_text("new");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(doc.root, Some(Value::from("new")));
    }

    #[test]
    fn selection_after_delete_prefers_next_then_previous_then_parent() {
        let ds = descends(&[vec![], vec!["a"], vec!["a", "x"], vec!["a", "y"], vec!["b"]]);
        let path = |p: &[&str]| p.iter().map(|s| key(s)).collect::<Vec<_>>();
        assert_eq!(selection_after_delete(&ds, &path(&["a"])), path(&["b"]));
        assert_eq!(selection_after_delete(&ds, &path(&["b"])), path(&["a"]));
        assert_eq!(selection_after_delete(&ds, &path(&["a", "x"])), path(&["a", "y"]));
        // An only child falls back to its parent.
        let only = descends(&[vec![], vec!["a"], vec!["a", "x"]]);
        assert_eq!(selection_after_delete(&only, &path(&["a", "x"])), path(&["a"]));
    }

    #[test]
    fn pending_insertions_mint_between_neighbors() {
        let lib = MutGid::new();
        let doc = Document {
            root: Some(Value::list([Value::from("a"), Value::from("b")])),
            gid: MutGid::new(),
        };
        let ps = positions(doc.root.as_ref().unwrap());
        let first = vec![Step::Element(ps[0].clone())];

        let Some(Selection::Pending { path, .. }) = pending_after(&src(&doc, &lib), &first) else {
            panic!("pending after");
        };
        let Step::Element(minted) = &path[0] else {
            panic!("an element step");
        };
        assert!(ps[0] < *minted && *minted < ps[1]);

        let Some(Selection::Pending { path, .. }) = pending_before(&src(&doc, &lib), &first) else {
            panic!("pending before");
        };
        let Step::Element(minted) = &path[0] else {
            panic!("an element step");
        };
        assert!(*minted < ps[0]);

        // Into the root list appends at the end; the first variant
        // prepends.
        let Some(Selection::Pending { path, .. }) = pending_into(&src(&doc, &lib), &[]) else {
            panic!("pending into");
        };
        let Step::Element(minted) = &path[0] else {
            panic!("an element step");
        };
        assert!(ps[1] < *minted);
        let Some(Selection::Pending { path, .. }) = pending_into_first(&src(&doc, &lib), &[]) else {
            panic!("pending into first");
        };
        let Step::Element(minted) = &path[0] else {
            panic!("an element step");
        };
        assert!(*minted < ps[0]);

        // A record field has no position to sit beside, and a record
        // takes no positional element — lists do, by type.
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("name"), Value::from("x"));
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        assert!(pending_after(&src(&doc, &lib), &[key("name")]).is_none());
        assert!(pending_into(&src(&doc, &lib), &[key("name")]).is_none());
        assert!(pending_into(&src(&doc, &lib), &[]).is_none());
        assert!(pending_into_first(&src(&doc, &lib), &[]).is_none());
        // An empty document offers the root; a rooted one does not.
        assert!(pending_root(&src(&doc, &lib)).is_none());
        let empty = Document {
            root: None,
            gid: MutGid::new(),
        };
        assert!(matches!(
            pending_root(&src(&empty, &lib)),
            Some(Selection::Pending { path, .. }) if path.is_empty()
        ));
        // Selecting the empty root pends immediately.
        assert!(matches!(
            Selection::edge(&src(&empty, &lib), Vec::new()),
            Selection::Pending { .. }
        ));
    }

    #[test]
    fn an_empty_list_takes_its_first_element() {
        let mut gid = MutGid::new();
        let root = new_node_id();
        gid.set(root, Atom::from("holes"), Value::list([]));
        gid.set(root, Atom::from("x"), Value::from(1.0));
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let lib = MutGid::new();

        let Some(Selection::Pending { path, .. }) =
            pending_insert(&src(&doc, &lib), &[key("holes")], true)
        else {
            panic!("first element into the empty list");
        };
        assert_eq!(path.len(), 2);
        assert!(matches!(&path[1], Step::Element(_)));

        // A record still declines the positional chord.
        assert!(pending_insert(&src(&doc, &lib), &[], true).is_none());
    }

    #[test]
    fn within_on_a_list_appends_an_element() {
        let mut gid = MutGid::new();
        let root = new_node_id();
        let items = Value::list([Value::from(1.0)]);
        gid.set(root, Atom::from("items"), items.clone());
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let lib = MutGid::new();
        let sources = src(&doc, &lib);

        // Lists hold no fields — structurally: the label pending
        // declines, and the within chord appends an element instead.
        let path = vec![key("items")];
        assert!(pending_edge(&sources, path.clone()).is_none());
        let Some(Selection::Pending { path: fresh, .. }) = pending_insert(&sources, &path, false)
        else {
            panic!("within on a list appends");
        };
        let Step::Element(minted) = &fresh[1] else {
            panic!("an element step");
        };
        assert!(positions(&items).last().unwrap() < minted);

        // Enter on a root list falls within the same way.
        let doc = Document {
            root: Some(items),
            gid: doc.gid.clone(),
        };
        assert!(matches!(
            pending_enter(&src(&doc, &lib), &[], false),
            Some(Selection::Pending { .. })
        ));
    }

    #[test]
    fn enter_continues_beside_and_the_chord_authors_within() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let nested = new_node_id();
        let items = Value::list([Value::from("a"), Value::from(nested)]);
        let record = new_node_id();
        gid.set(record, Atom::from("items"), items.clone());
        gid.set(record, Atom::from("x"), Value::from(1.0));
        let doc = Document {
            root: Some(Value::from(record)),
            gid,
        };
        let ps = positions(&items);
        let items_path = vec![key("items")];
        let first = vec![key("items"), Step::Element(ps[0].clone())];
        let second = vec![key("items"), Step::Element(ps[1].clone())];
        let minted_in = |path: &[Step]| match path.last() {
            Some(Step::Element(position)) => position.clone(),
            _ => panic!("an element step"),
        };

        // Enter continues the enumeration: any element — atom or node
        // — pends a sibling (before with shift).
        let Some(Selection::Pending { path, .. }) = pending_enter(&src(&doc, &lib), &first, false) else {
            panic!("sibling after");
        };
        let minted = minted_in(&path);
        assert!(ps[0] < minted && minted < ps[1]);
        let Some(Selection::Pending { path, .. }) = pending_enter(&src(&doc, &lib), &first, true) else {
            panic!("sibling before");
        };
        assert!(minted_in(&path) < ps[0]);
        let Some(Selection::Pending { path, .. }) = pending_enter(&src(&doc, &lib), &second, false) else {
            panic!("node element sibling");
        };
        assert!(ps[1] < minted_in(&path));

        // A field value — atom or node — pends the parent's next
        // field; the root has nothing beside it and takes the field
        // on itself.
        assert!(matches!(
            pending_enter(&src(&doc, &lib), &[key("x")], false),
            Some(Selection::PendingEdge { parent, .. }) if parent.is_empty()
        ));
        assert!(matches!(
            pending_enter(&src(&doc, &lib), &items_path, false),
            Some(Selection::PendingEdge { parent, .. }) if parent.is_empty()
        ));
        assert!(matches!(
            pending_enter(&src(&doc, &lib), &[], false),
            Some(Selection::PendingEdge { parent, .. }) if parent.is_empty()
        ));

        // The chord authors within: a field edge on the selected map
        // — empty nodes included — and an appended element on a
        // list, which holds no fields; atoms decline.
        let Some(Selection::Pending { path, .. }) =
            pending_insert(&src(&doc, &lib), &items_path, false)
        else {
            panic!("within on a list appends");
        };
        assert!(ps[1] < minted_in(&path));
        assert!(matches!(
            pending_insert(&src(&doc, &lib), &second, false),
            Some(Selection::PendingEdge { parent, .. }) if parent == second
        ));
        assert!(pending_insert(&src(&doc, &lib), &first, false).is_none());

        // Shift is the positional variant: a first element at the
        // front — prepend on a list — declining on records, empty
        // nodes (maps take fields, not elements), and atoms.
        let Some(Selection::Pending { path, .. }) = pending_insert(&src(&doc, &lib), &items_path, true)
        else {
            panic!("prepend");
        };
        assert!(minted_in(&path) < ps[0]);
        assert!(pending_insert(&src(&doc, &lib), &second, true).is_none());
        assert!(pending_insert(&src(&doc, &lib), &[], true).is_none());
        assert!(pending_insert(&src(&doc, &lib), &first, true).is_none());
    }

    #[test]
    fn sibling_stepping_continues_through_ancestors() {
        let all = descends(&[
            vec![],
            vec!["a"],
            vec!["a", "x"],
            vec!["a", "y"],
            vec!["b"],
        ]);
        let at = |labels: &[&str]| -> Path { labels.iter().map(|s| key(s)).collect() };

        // Within a parent: plain sibling steps.
        assert_eq!(sibling(&all, &at(&["a", "x"]), true), Some(at(&["a", "y"])));
        // Past the last child, Down flows to the enclosing next
        // sibling; Up mirrors from the first child.
        assert_eq!(sibling(&all, &at(&["a", "y"]), true), Some(at(&["b"])));
        assert_eq!(sibling(&all, &at(&["b"]), false), Some(at(&["a"])));
        assert_eq!(sibling(&all, &at(&["a", "x"]), false), None);
        // The document's ends still end.
        assert_eq!(sibling(&all, &at(&["b"]), true), None);
    }

    #[test]
    fn secondary_is_the_selected_edges_value_lists_included() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let shared = new_node_id();
        gid.set(shared, Atom::from("x"), Value::from(1.0));
        let root = new_node_id();
        gid.set(root, Atom::from("a"), Value::from(shared));
        gid.set(root, Atom::from("b"), Value::from(shared));
        gid.set(root, Atom::from("pair"), Value::list([Value::from(2.0)]));
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let edge = |path: Vec<Step>| Selection::Edge {
            path,
            edit: None,
            recorded: false,
        };

        assert_eq!(
            secondary_of(&src(&doc, &lib), Some(&edge(vec![key("a")]))),
            Some(Value::from(shared))
        );
        // Atoms are values too, and so are lists — equal lists mark
        // alike, value semantics displayed honestly.
        assert_eq!(
            secondary_of(&src(&doc, &lib), Some(&edge(vec![key("a"), key("x")]))),
            Some(Value::from(1.0))
        );
        assert_eq!(
            secondary_of(&src(&doc, &lib), Some(&edge(vec![key("pair")]))),
            Some(Value::list([Value::from(2.0)]))
        );
        // Pendings and no selection don't mark.
        assert_eq!(
            secondary_of(&src(&doc, &lib), Some(&pending_value(vec![key("c")]))),
            None
        );
        assert_eq!(secondary_of(&src(&doc, &lib), None), None);
    }

    #[test]
    fn pending_edge_targets_nodes_only() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("name"), Value::from("x"));
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        assert!(matches!(
            pending_edge(&src(&doc, &lib), Vec::new()),
            Some(Selection::PendingEdge { parent, .. }) if parent.is_empty()
        ));
        // An atom takes no edges; an empty document has no node.
        assert!(pending_edge(&src(&doc, &lib), vec![key("name")]).is_none());
        let empty = Document {
            root: None,
            gid: MutGid::new(),
        };
        assert!(pending_edge(&src(&empty, &lib), Vec::new()).is_none());
    }

    #[test]
    fn completion_offers_atom_references_and_new_node() {
        let mut gid = MutGid::new();
        for name in ["origin", "corner"] {
            let node = new_node_id();
            gid.set(node, Atom::Node(NAME), Value::from(name));
        }
        let names = Names::convention();
        // An empty library keeps the offer counts about the document.
        let empty = MutGid::new();
        let complete = |gid: &MutGid, root: Option<&Value>, query: &str, names: &Names| {
            let doc = Document {
                root: root.cloned(),
                gid: gid.clone(),
            };
            completion_entries(&src(&doc, &empty), names, false, false, query)
        };

        // A confident reference match outranks the typed string;
        // "corner" has no i, so only "origin" matches the query.
        let entries = complete(&gid, None, "orig", &names);
        assert_eq!(entries[0].display, "origin");
        assert!(entries[0].detail.is_some());
        assert_eq!(entries[1].display, "\"orig\"");
        assert!(matches!(&entries[1].action, EntryAction::Value(v) if v.as_str().is_some()));
        // "new list" ranks like a reference and "orig" doesn't match
        // it, so the named creation offer is the one trailer.
        assert_eq!(entries.len(), 3);
        assert!(matches!(
            &entries.last().unwrap().action,
            EntryAction::NewNode { name: Some(name) } if name == "orig"
        ));

        // A leading quote forces the string back on top, closed or
        // not, and the new node's name drops the quotes.
        let entries = complete(&gid, None, "\"orig", &names);
        assert!(matches!(&entries[0].action, EntryAction::Value(v) if v.as_str() == Some("orig")));
        assert!(matches!(
            &entries.last().unwrap().action,
            EntryAction::NewNode { name: Some(name) } if name == "orig"
        ));

        // An empty query offers every node — the NAME label is
        // itself an unnamed node here — plus "new list", unnamed
        // creation, and the empty string.
        let entries = complete(&gid, None, "", &names);
        assert_eq!(entries.len(), 6);
        assert!(
            entries
                .iter()
                .any(|entry| matches!(entry.action, EntryAction::NewList))
        );
        assert!(matches!(
            &entries.last().unwrap().action,
            EntryAction::NewNode { name: None }
        ));

        // Typing toward "new list" surfaces it (a prefix match leads
        // the references); typing away drops it entirely. The label
        // stage never offers it: a key must mean.
        let entries = complete(&gid, None, "new li", &names);
        assert!(matches!(entries[0].action, EntryAction::NewList));
        let entries = complete(&gid, None, "asdf", &names);
        assert!(
            !entries
                .iter()
                .any(|entry| matches!(entry.action, EntryAction::NewList))
        );
        let doc = Document {
            root: None,
            gid: gid.clone(),
        };
        let label_entries = completion_entries(&src(&doc, &empty), &names, false, true, "");
        assert!(
            !label_entries
                .iter()
                .any(|entry| matches!(entry.action, EntryAction::NewList))
        );

        // Numbers infer as the atom entry and lead — with the typed
        // text always insertable as a string right below.
        let entries = complete(&gid, None, "2.5", &names);
        assert!(matches!(&entries[0].action, EntryAction::Value(v) if v.as_number() == Some(2.5)));
        assert!(matches!(&entries[1].action, EntryAction::Value(v) if v.as_str() == Some("2.5")));
        // A quote is stated string intent: no number offer rides along.
        let entries = complete(&gid, None, "\"2.5", &names);
        assert!(matches!(&entries[0].action, EntryAction::Value(v) if v.as_str() == Some("2.5")));
        assert!(
            !entries
                .iter()
                .any(|entry| matches!(&entry.action, EntryAction::Value(v) if v.as_number().is_some()))
        );

        // Unnamed nodes are offered too, searchable by the short id
        // they render as — including edgeless ones referenced only
        // inside a list value — and an exact suffix ranks first. A
        // hex suffix can happen to parse as a number ("12345",
        // "12e45") and legitimately cede the lead to the atom, so
        // pick one that doesn't.
        let orphan = std::iter::repeat_with(new_node_id)
            .find(|node| {
                short_id(*node)
                    .trim_start_matches('…')
                    .parse::<f64>()
                    .is_err()
            })
            .unwrap();
        let scratch = new_node_id();
        gid.set(scratch, Atom::from("refs"), Value::list([Value::from(orphan)]));
        let suffix = short_id(orphan);
        let entries = complete(&gid, None, suffix.trim_start_matches('…'), &names);
        assert_eq!(entries[0].display, suffix);
        assert!(entries[0].detail.is_none());
        assert!(
            matches!(&entries[0].action, EntryAction::Value(v) if v.as_node() == Some(orphan))
        );

        // The root's reference is a Document field, not a gid edge -
        // a fresh edgeless node at root is still offered.
        let root = new_node_id();
        let root_value = Value::from(root);
        let entries = complete(&gid, Some(&root_value), "", &names);
        assert!(entries.iter().any(
            |entry| matches!(&entry.action, EntryAction::Value(v) if v.as_node() == Some(root))
        ));
    }

    #[test]
    fn the_name_policy_is_editor_state() {
        let library = crate::conventions::library();
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::Node(NAME), Value::from("origin"));
        let doc = Document {
            root: None,
            gid: gid.clone(),
        };
        let sources = src(&doc, &library);

        // The name carries its provenance: the consumed edge label.
        let name = Names::convention().of(&sources, node).unwrap();
        assert_eq!(name.text, "origin");
        assert_eq!(name.label, Some(Atom::Node(NAME)));

        // The convention node's own name is library DATA, and a
        // document's stored name shadows it.
        let convention = Names::convention().of(&sources, NAME).unwrap();
        assert_eq!(convention.text, "name");
        assert_eq!(convention.label, Some(Atom::Node(NAME)));
        gid.set(NAME, Atom::Node(NAME), Value::from("nombre"));
        let doc = Document { root: None, gid };
        let sources = src(&doc, &library);
        let stored = Names::convention().of(&sources, NAME).unwrap();
        assert_eq!(stored.text, "nombre");

        // Completion keys derive from the raw bit: raw offers the
        // node by its short id, so the name no longer matches — the
        // policy itself is never swapped.
        let entries = completion_entries(&sources, &Names::convention(), true, false, "orig");
        assert!(!entries.iter().any(|entry| entry.display == "origin"));
        let entries = completion_entries(&sources, &Names::convention(), false, false, "orig");
        assert_eq!(entries[0].display, "origin");
    }

    #[test]
    fn the_library_reads_through_but_never_writes() {
        let library = crate::conventions::library();
        let mut doc = Document {
            root: Some(Value::from(NAME)),
            gid: MutGid::new(),
        };
        let sources = src(&doc, &library);
        let name_path = vec![Step::Key(Atom::Node(NAME))];

        // Completion offers the conventions from keystroke one, as
        // references: picking "name" yields the NAME node, not a
        // lookalike string label.
        let entries = completion_entries(&sources, &Names::convention(), false, false, "nam");
        assert_eq!(entries[0].display, "name");
        assert!(matches!(
            &entries[0].action,
            EntryAction::Value(v) if v.as_node() == Some(NAME)
        ));

        // Library facts render through the sources, and paths now
        // RESOLVE through them...
        assert_eq!(sorted_edges(&sources, NAME).len(), 1);
        assert_eq!(
            src(&doc, &library).resolve(&name_path),
            Some(&Value::from("name"))
        );
        // ...but writes gate on entity authority: the NAME entity is
        // external, so editing and deleting decline with no read-only
        // flag anywhere.
        assert!(!delete_edge(&mut doc, &library, &name_path));
        assert!(!set_value(&mut doc, &library, &name_path, Value::from("x")));
        assert!(
            Selection::edge(&src(&doc, &library), name_path.clone())
                .edit()
                .is_none()
        );

        // Authoring gestures decline on a library-described entity
        // too — one document edge would shadow its facts wholesale —
        // while authoring BESIDE the reference stays a document edit.
        let mut gid = MutGid::new();
        let root = new_node_id();
        gid.set(root, Atom::from("k"), Value::from(NAME));
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let path = vec![key("k")];
        assert!(pending_edge(&src(&doc, &library), path.clone()).is_none());
        assert!(pending_insert(&src(&doc, &library), &path, false).is_none());
        assert!(pending_insert(&src(&doc, &library), &path, true).is_none());
        assert!(matches!(
            pending_enter(&src(&doc, &library), &path, false),
            Some(Selection::PendingEdge { parent, .. }) if parent.is_empty()
        ));
    }

    #[test]
    fn commit_pending_mints_named_nodes_and_bare_lists() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let root = new_node_id();
        gid.set(root, Atom::from("x"), Value::from(1.0));
        let mut doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let path = vec![key("fresh")];
        assert!(commit_pending(
            &mut doc,
            &lib,
            &path,
            &EntryAction::NewNode {
                name: Some("thing".into())
            }
        ));
        let value = src(&doc, &lib).resolve(&path).unwrap().clone();
        let node = value.as_node().unwrap();
        assert_eq!(
            doc.gid.get(node, &Atom::Node(NAME)),
            Some(&Value::from("thing"))
        );

        // "new list" commits the empty list value — pure, no entity
        // minted anywhere.
        let entities = doc.gid.entities().count();
        assert!(commit_pending(&mut doc, &lib, &[key("items")], &EntryAction::NewList));
        assert_eq!(
            src(&doc, &lib).resolve(&[key("items")]),
            Some(&Value::list([]))
        );
        assert_eq!(doc.gid.entities().count(), entities);
    }

    #[test]
    fn the_clipboard_round_trips_values_and_reads_foreign_text() {
        let node = new_node_id();
        let cases = [
            Value::from("hello"),
            Value::from("42"),
            Value::from(2.5),
            Value::from(node),
            Value::list([Value::from(1.0), Value::from(node)]),
            Value::list([]),
        ];
        for value in cases {
            assert_eq!(from_clipboard(&to_clipboard(&value)), value, "{value}");
        }
        // Atoms travel as the query language, readable elsewhere; the
        // string "42" keeps its quotes so it comes back a string.
        assert_eq!(to_clipboard(&Value::from("hello")), "\"hello\"");
        assert_eq!(to_clipboard(&Value::from("42")), "\"42\"");
        assert_eq!(to_clipboard(&Value::from(2.5)), "2.5");
        // Foreign text pastes by the query reading.
        assert_eq!(from_clipboard("3.5"), Value::from(3.5));
        assert_eq!(from_clipboard("plain words"), Value::from("plain words"));
    }

    #[test]
    fn resolve_query_infers_the_value() {
        assert_eq!(resolve_query("3.5"), Value::from(3.5));
        assert_eq!(resolve_query(" -2 "), Value::from(-2.0));
        assert_eq!(resolve_query("abc"), Value::from("abc"));
        assert_eq!(resolve_query("\"3.5\""), Value::from("3.5"));
        assert_eq!(resolve_query(""), Value::from(""));
        // A leading quote is string mode even before it closes.
        assert_eq!(resolve_query("\"3.5"), Value::from("3.5"));
        assert_eq!(resolve_query("\""), Value::from(""));
    }

    #[test]
    fn toggle_collapse_stays_sparse_and_respects_cycle_defaults() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let root = new_node_id();
        let child = new_node_id();
        gid.set(root, Atom::from("child"), Value::from(child));
        gid.set(child, Atom::from("back"), Value::from(root));
        gid.set(child, Atom::from("name"), Value::from("c"));
        gid.set(root, Atom::from("pair"), Value::list([Value::from(1.0)]));
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let mut collapse = Collapse::default();
        let child_path = vec![key("child")];
        let back_path = vec![key("child"), key("back")];

        // Expanded by default: toggling stores a collapse override,
        // toggling again removes it.
        assert!(toggle_collapse(&src(&doc, &lib), &mut collapse, &child_path));
        assert!(collapse.collapsed(&child_path, false));
        assert!(toggle_collapse(&src(&doc, &lib), &mut collapse, &child_path));
        assert!(collapse.overrides.is_empty());

        // The back-edge to the root is in a cycle, so its default is
        // collapsed; toggling expands it.
        assert!(toggle_collapse(&src(&doc, &lib), &mut collapse, &back_path));
        assert!(!collapse.collapsed(&back_path, true));

        // A list with elements collapses too; strings and missing
        // paths have nothing to collapse.
        assert!(toggle_collapse(&src(&doc, &lib), &mut collapse, &[key("pair")]));
        assert!(!toggle_collapse(
            &src(&doc, &lib),
            &mut collapse,
            &[key("child"), key("name")]
        ));
        assert!(!toggle_collapse(&src(&doc, &lib), &mut collapse, &[key("nope")]));
    }

    #[test]
    fn collapse_default_follows_cycle_and_overrides_win() {
        let mut collapse = Collapse::default();
        let path = vec![key("a"), key("b")];
        // Absent from the overrides: expanded outside a cycle, collapsed in.
        assert!(!collapse.collapsed(&path, false));
        assert!(collapse.collapsed(&path, true));
        // An override forces the state against the default either way.
        collapse.overrides.insert(path.clone(), true);
        assert!(collapse.collapsed(&path, false));
        collapse.overrides.insert(path.clone(), false);
        assert!(!collapse.collapsed(&path, true));
    }

    use puri::draw::{GlyphRun, Shape};
    use puri::handler::Handler;
    use puri::layout::place_top_left;

    /// A placement context that keeps only what assertions need:
    /// descends, the popup, and a dispatchable handler; drawing is
    /// discarded.
    #[derive(Default)]
    struct Probe<C = ()> {
        handler: Handler<C>,
        descends: Vec<Descend>,
        popup: Option<Popup>,
    }

    impl<C> Canvas for Probe<C> {
        fn fill(&mut self, _: impl Into<Shape>, _: impl Into<Brush>, _: Affine) {}
        fn stroke(&mut self, _: impl Into<Shape>, _: Stroke, _: impl Into<Brush>, _: Affine) {}
        fn glyph_run(&mut self, _: GlyphRun) {}
        fn clip(&mut self, _: impl Into<Shape>, _: Affine, content: impl FnOnce(&mut Self)) {
            content(self);
        }
    }

    impl<C> HasHandler<C> for Probe<C> {
        fn handler(&mut self) -> &mut Handler<C> {
            &mut self.handler
        }
    }

    impl<C> HasDescends for Probe<C> {
        fn descends(&mut self) -> &mut Vec<Descend> {
            &mut self.descends
        }
    }

    impl<C> HasPopup for Probe<C> {
        fn popup(&mut self) -> &mut Option<Popup> {
            &mut self.popup
        }
    }

    fn probe_hooks() -> Hooks<()> {
        Hooks {
            select: Rc::new(|_, _, _| {}),
            toggle: Rc::new(|_, _| {}),
            edit: Rc::new(|_: &mut ()| None),
            pick: Rc::new(|_, _| false),
        }
    }

    /// Hooks whose context records every select's path.
    fn logging_hooks() -> Hooks<Vec<Path>> {
        Hooks {
            select: Rc::new(|log: &mut Vec<Path>, path, _| log.push(path)),
            toggle: Rc::new(|_, _| {}),
            edit: Rc::new(|_: &mut Vec<Path>| None),
            pick: Rc::new(|_, _| false),
        }
    }

    fn down_at(point: Point) -> ui_events::pointer::PointerButtonEvent {
        let mut state = ui_events::pointer::PointerState::default();
        state.position.x = point.x;
        state.position.y = point.y;
        ui_events::pointer::PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: ui_events::pointer::PointerInfo {
                pointer_id: Some(ui_events::pointer::PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: ui_events::pointer::PointerType::Mouse,
            },
            state,
        }
    }

    fn place_probe_with<C: 'static + Default>(
        doc: &Document,
        selection: Option<&Selection>,
        names: &Names,
        raw: bool,
        hooks: Hooks<C>,
    ) -> Probe<C> {
        let mut fonts = parley::FontContext::new();
        let mut layouts = parley::LayoutContext::new();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
        };
        let styles = RawStyles::new(1.0);
        let library = crate::conventions::library();
        let node = project::<C, Probe<C>>(
            &src(doc, &library),
            selection,
            None,
            &Collapse::default(),
            names,
            raw,
            &mut tcx,
            &styles,
            hooks,
        );
        let mut probe = Probe::default();
        place_top_left(node, &mut probe, Point::ZERO);
        probe
    }

    fn place_probe_in(doc: &Document, names: &Names, raw: bool) -> Probe {
        place_probe_with(doc, None, names, raw, probe_hooks())
    }

    fn place_probe(doc: &Document) -> Probe {
        place_probe_in(doc, &Names::convention(), false)
    }

    #[test]
    fn node_elements_take_the_block_form_in_position_order() {
        let mut gid = MutGid::new();
        let nested = new_node_id();
        gid.set(nested, Atom::from("x"), Value::from(9.0));
        let items = Value::list([Value::from(1.0), Value::from(nested)]);
        let ps = positions(&items);
        let doc = Document {
            root: Some(items),
            gid,
        };
        let probe = place_probe(&doc);

        // A node element takes the list off the inline literal:
        // element rows in position order, each below the last.
        let paths: Vec<Path> = probe.descends.iter().map(|d| d.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                Vec::new(),
                vec![Step::Element(ps[0].clone())],
                vec![Step::Element(ps[1].clone())],
                vec![Step::Element(ps[1].clone()), key("x")],
            ]
        );
        let tops: Vec<f64> = probe.descends.iter().skip(1).map(|d| d.rect.y0).collect();
        assert!(tops.is_sorted_by(|a, b| a < b), "{tops:?}");
    }

    #[test]
    fn named_nodes_project_the_name_as_their_header() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::Node(NAME), Value::from("thing"));
        gid.set(node, Atom::from("x"), Value::from(1.0));
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let probe = place_probe(&doc);

        // The name edge is consumed by the header: it still descends
        // (selectable, editable), but as the un-indented head above
        // the field rows, not as a row of its own.
        let paths: Vec<Path> = probe.descends.iter().map(|d| d.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                Vec::new(),
                vec![Step::Key(Atom::Node(NAME))],
                vec![key("x")]
            ]
        );
        let name_rect = probe.descends[1].rect;
        let x_rect = probe.descends[2].rect;
        assert!(name_rect.y0 < x_rect.y0);
        assert!(name_rect.x0 < x_rect.x0);

        // A node whose only edge is its name is just its name.
        let mut gid = MutGid::new();
        let sole = new_node_id();
        gid.set(sole, Atom::Node(NAME), Value::from("leaf"));
        let doc = Document {
            root: Some(Value::from(sole)),
            gid,
        };
        let probe = place_probe(&doc);
        let paths: Vec<Path> = probe.descends.iter().map(|d| d.path.clone()).collect();
        assert_eq!(paths, vec![Vec::new(), vec![Step::Key(Atom::Node(NAME))]]);
    }

    #[test]
    fn a_named_header_selects_its_node_first_then_engages_as_text() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::Node(NAME), Value::from("thing"));
        gid.set(node, Atom::from("x"), Value::from(1.0));
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };

        // Cold: a click on the name falls through to the block,
        // selecting the NODE edge.
        let probe = place_probe_with(&doc, None, &Names::convention(), false, logging_hooks());
        let name_rect = probe.descends[1].rect;
        let mut log: Vec<Path> = Vec::new();
        assert!(probe.handler.dispatch_pointer_down(&mut log, &down_at(name_rect.center())));
        assert_eq!(log, vec![Vec::<Step>::new()]);

        // With the node selected, the successor pass engages the
        // name as a text target: the same click now selects the name
        // edge (and carries the caret placement).
        let selection = Selection::edge(&src(&doc, &lib), Vec::new());
        let probe = place_probe_with(
            &doc,
            Some(&selection),
            &Names::convention(),
            false,
            logging_hooks(),
        );
        let name_rect = probe.descends[1].rect;
        let mut log: Vec<Path> = Vec::new();
        assert!(probe.handler.dispatch_pointer_down(&mut log, &down_at(name_rect.center())));
        assert_eq!(log, vec![vec![Step::Key(Atom::Node(NAME))]]);
    }

    #[test]
    fn a_label_pending_owns_its_clicks_and_the_parent_stays_clickable() {
        let mut gid = MutGid::new();
        let lib = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::from("x"), Value::from(1.0));
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let pending = pending_edge(&src(&doc, &lib), Vec::new()).unwrap();
        let probe = place_probe_with(
            &doc,
            Some(&pending),
            &Names::convention(),
            false,
            logging_hooks(),
        );

        // A click on the pending row is swallowed — handled, but no
        // selection reported — so the pending survives its own
        // clicks. (The caret target declines here because the
        // logging hooks expose no editor.)
        let anchor = probe.popup.as_ref().unwrap().anchor;
        let mut log: Vec<Path> = Vec::new();
        assert!(probe.handler.dispatch_pointer_down(&mut log, &down_at(anchor.center())));
        assert!(log.is_empty());

        // A click on the parent's header reports the parent — a real
        // selection change, not a nudge of the pending.
        let header = probe.descends[0].rect;
        assert!(probe.handler.dispatch_pointer_down(
            &mut log,
            &down_at(Point::new(header.x0 + 2.0, header.y0 + 2.0)),
        ));
        assert_eq!(log, vec![Vec::<Step>::new()]);
    }

    #[test]
    fn forked_entities_edit_in_place_under_library_structure() {
        // A library with real structure: S --x--> B, B --y--> 1.
        let s_node = new_node_id();
        let b = new_node_id();
        let mut library = MutGid::new();
        library.set(s_node, Atom::from("x"), Value::from(b));
        library.set(b, Atom::from("y"), Value::from(1.0));
        // The document references S.
        let mut gid = MutGid::new();
        let root = new_node_id();
        gid.set(root, Atom::from("k"), Value::from(s_node));
        let mut doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let b_path = vec![key("k"), key("x")];
        let y_path = vec![key("k"), key("x"), key("y")];

        // Reading resolves through the library; writing declines
        // while the library is the authority for B.
        assert_eq!(src(&doc, &library).resolve(&b_path), Some(&Value::from(b)));
        assert!(pending_edge(&src(&doc, &library), b_path.clone()).is_none());
        assert!(
            Selection::edge(&src(&doc, &library), y_path.clone())
                .edit()
                .is_none()
        );
        assert!(!set_value(&mut doc, &library, &y_path, Value::from(2.0)));
        assert!(!delete_edge(&mut doc, &library, &y_path));

        // The fork: the document takes B over (copy/paste's eventual
        // job — same identity, copied wholesale). The structure now
        // shows the document's B, editable IN PLACE under S.
        doc.gid.set(b, Atom::from("y"), Value::from(1.0));
        assert!(pending_edge(&src(&doc, &library), b_path.clone()).is_some());
        assert!(
            Selection::edge(&src(&doc, &library), y_path.clone())
                .edit()
                .is_some()
        );
        assert!(set_value(&mut doc, &library, &y_path, Value::from(2.0)));
        assert_eq!(src(&doc, &library).resolve(&y_path), Some(&Value::from(2.0)));
        assert!(delete_edge(&mut doc, &library, &y_path));

        // S itself stays external and inert: no fields on it, no
        // retargeting its edges.
        assert!(pending_edge(&src(&doc, &library), vec![key("k")]).is_none());
        assert!(!set_value(&mut doc, &library, &b_path, Value::from(9.0)));
    }

    #[test]
    fn lists_under_external_entities_take_no_edits() {
        // A library entity holding a list value: the list reads
        // through, but its spine owner is external, so nothing mints
        // beside or into it and elements take no editors.
        let mut library = MutGid::new();
        let entity = new_node_id();
        library.set(entity, Atom::from("items"), Value::list([Value::from("a")]));
        let mut gid = MutGid::new();
        let root = new_node_id();
        gid.set(root, Atom::from("m"), Value::from(entity));
        let doc = Document {
            root: Some(Value::from(root)),
            gid,
        };
        let list_path = vec![key("m"), key("items")];
        let ps = positions(src(&doc, &library).resolve(&list_path).unwrap());
        let element = vec![key("m"), key("items"), Step::Element(ps[0].clone())];

        assert_eq!(src(&doc, &library).resolve(&element), Some(&Value::from("a")));
        assert!(pending_enter(&src(&doc, &library), &element, false).is_none());
        assert!(pending_insert(&src(&doc, &library), &list_path, true).is_none());
        assert!(
            Selection::edge(&src(&doc, &library), element.clone())
                .edit()
                .is_none()
        );
    }

    #[test]
    fn the_raw_view_stands_names_down_but_lists_stay_lists() {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Atom::Node(NAME), Value::from("thing"));
        gid.set(
            node,
            Atom::from("items"),
            Value::list([Value::from(1.0), Value::from(2.0)]),
        );
        let doc = Document {
            root: Some(Value::from(node)),
            gid,
        };
        let probe = place_probe_in(&doc, &Names::convention(), true);

        // The name is a plain labeled row again — nothing consumed
        // into a header — but the list still projects as a list:
        // kind is data, not convention, so Raw keeps the brackets
        // (and the session-minted positions stay out of view).
        let paths: Vec<Path> = probe.descends.iter().map(|d| d.path.clone()).collect();
        assert_eq!(paths.len(), 5);
        assert!(paths.contains(&vec![Step::Key(Atom::Node(NAME))]));
        // Rows are indented under the id header, the name's included.
        assert!(probe.descends[1].rect.x0 > probe.descends[0].rect.x0);
        // The two atom elements sit inline on one line.
        let elements: Vec<Rect> = probe
            .descends
            .iter()
            .filter(|d| d.path.len() == 2)
            .map(|d| d.rect)
            .collect();
        let (a, b) = (elements[0], elements[1]);
        assert!((a.y0 - b.y0).abs() < 0.5, "{a:?} vs {b:?}");
    }

    #[test]
    fn atom_only_lists_project_inline() {
        let items = Value::list([Value::from(1.0), Value::from("two")]);
        let ps = positions(&items);
        let doc = Document {
            root: Some(items),
            gid: MutGid::new(),
        };
        let probe = place_probe(&doc);

        let paths: Vec<Path> = probe.descends.iter().map(|d| d.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                Vec::new(),
                vec![Step::Element(ps[0].clone())],
                vec![Step::Element(ps[1].clone())],
            ]
        );
        // One line, reading left to right.
        let (a, b) = (probe.descends[1].rect, probe.descends[2].rect);
        assert!((a.y0 - b.y0).abs() < 0.5, "{a:?} vs {b:?}");
        assert!(a.x1 <= b.x0);
    }
}
