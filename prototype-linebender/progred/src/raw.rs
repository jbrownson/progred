//! The raw projection: any document rendered with no schema, in the
//! delimiter family — `(` cell `)`, `[` list `]`, `{` record `}`.
//! A cell heads with its name (identity metadata, editable in
//! place) or its short id; records are field rows, lists inline
//! literals or dashed element rows; atoms render as their values;
//! positions are session bookkeeping and never render at all.

use crate::conventions::Names;
use crate::filter;
use crate::sources::Sources;
use im::OrdMap;
use progred_graph::{
    Atom, CellId, Cells, Label, Position, Step, Value, hex_string, new_cell_id, position, spine,
};
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
const NAME_COLOR: [f32; 4] = [0.13, 0.14, 0.16, 1.0];
const QUERY_COLOR: [f32; 4] = [0.46, 0.49, 0.55, 1.0];

pub struct RawStyles {
    pub label: TextStyle,
    /// A cell's own name, projected as its handle: the strongest text
    /// in a block.
    pub name: TextStyle,
    pub string: TextStyle,
    pub dim: TextStyle,
    /// Byte-identity renderings — short ids, blob hex — in monospace,
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
            name: style(14.0, [0.13, 0.14, 0.16, 1.0], None),
            string: style(14.0, STRING_COLOR, None),
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

/// A document: its `root` value plus the cell table holding every
/// identity's current value. Every projection path starts at `root` —
/// typically a link, or an inline record keying the document's parts
/// by role. The root is a location like any other — the empty path —
/// so edits there commit to this field, and deleting it empties the
/// document. Clones are O(1): the table and its values share
/// structure, which is what makes snapshot undo free.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub root: Option<Value>,
    pub cells: Cells,
}

/// A small document shaped like a real one. The root is an inline
/// RECORD of roles — a document keys its parts by what they are to
/// it, and needs no identity of its own to do so. Names live in the
/// cell table and name individuals ("roof", not its kind — kinds are
/// a future isa convention's job). The corner knows its roof (cycle
/// collapse on a real pattern); the style cell is unnamed and
/// referenced twice (short-id heads, secondary marks); the stroke
/// cell is a NAMED BARE floating field definition — a name and
/// nothing else, referenced as a label, never enumerated; the
/// material cell is fully bare — referenced before anything at all
/// is said about it; the swatch is a blob; each point's position is
/// an inline record, point-shaped data that wants to be a value; and
/// the favorite cell holds a bare LINK to the corner — the alias
/// pattern, and the standing repro for the block-in-row rendering
/// seam (a cell whose value blocks inside another cell's parens).
pub fn sample_document() -> Document {
    let mut cells = Cells::new();
    let roof = new_cell_id();

    let origin = new_cell_id();
    cells.set_name(origin, "origin");
    cells.set_value(
        origin,
        Value::record([(
            Label::from("at"),
            Value::record([
                (Label::from("row"), Value::from("top")),
                (Label::from("col"), Value::from("left")),
            ]),
        )]),
    );

    let corner = new_cell_id();
    cells.set_name(corner, "corner");
    cells.set_value(
        corner,
        Value::record([
            (
                Label::from("at"),
                Value::record([
                    (Label::from("row"), Value::from("bottom")),
                    (Label::from("col"), Value::from("right")),
                ]),
            ),
            // A part that knows its whole: the cycle a real document
            // has, rendered as a collapsed head rather than recursing
            // forever.
            (Label::from("of"), Value::from(roof)),
        ]),
    );

    let stroke = new_cell_id();
    cells.set_name(stroke, "stroke");

    let style = new_cell_id();
    cells.set_value(
        style,
        Value::record([
            (Label::from("color"), Value::from("rebeccapurple")),
            // #663399, as bytes.
            (Label::from("swatch"), Value::from(vec![0x66, 0x33, 0x99])),
        ]),
    );

    let material = new_cell_id();

    let favorite = new_cell_id();
    cells.set_name(favorite, "favorite");
    cells.set_value(favorite, Value::from(corner));

    cells.set_name(roof, "roof");
    cells.set_value(
        roof,
        Value::record([
            (
                Label::from("points"),
                Value::list([Value::from(origin), Value::from(corner)]),
            ),
            (Label::Cell(stroke), Value::from("hairline")),
            (
                Label::from("tags"),
                Value::list([Value::from("draft"), Value::from("gabled")]),
            ),
            (Label::from("material"), Value::from(material)),
            (Label::from("style"), Value::from(style)),
        ]),
    );

    Document {
        root: Some(Value::record([
            (Label::from("shape"), Value::from(roof)),
            (Label::from("style"), Value::from(style)),
            (Label::from("favorite"), Value::from(favorite)),
        ])),
        cells,
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
/// (optionally with a text click) does, what toggling a collapse
/// does, and how a dispatch reaches the selection's editor state and
/// measurement caches.
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
    /// The display name at this projection, through the editor's one
    /// read: names are identity data, so Raw shows the table's own —
    /// only the policy stands down there.
    fn name(&self, cell: CellId) -> Option<String> {
        crate::conventions::display_name(&self.sources, self.names, self.raw, cell)
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

    /// The label query of a new field being authored on the record at
    /// `path`.
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

/// A location in the projected spanning tree: Key steps into record
/// fields, Element steps into list values, Follow steps through a
/// link to its cell's current value. The same value can be projected
/// at several paths, so the path — not the value — is the identity a
/// selection names; every reference site unfolds through its own
/// Follow, and no site is the value's home. List elements sit at
/// positions sibling edits never move; wraps and unwraps will adjust
/// path-keyed state through one general rewrite — see
/// `docs/model.md`.
pub type Path = Vec<Step>;

/// What is selected: the value at a path, or a nonexistent field
/// being authored. A selected string carries its live editor state —
/// every string is a text editor, focused by selection, and the graph
/// is written through as it edits. A pending selection carries the
/// completion query instead; the query resolves to the value that
/// commits, and until then the graph is untouched — deselecting
/// discards the pending entirely.
pub enum Selection {
    Edge {
        path: Path,
        edit: Option<LineEditState>,
        /// Whether this editor's write-through run has recorded its
        /// undo step: the run is the editor's lifetime, so the first
        /// write records and the rest coalesce by staying silent.
        recorded: bool,
    },
    /// A nonexistent location's value being authored (the root and a
    /// bare cell's value included).
    Pending {
        path: Path,
        query: LineEditState,
        /// Which completion entry commits; clamped against the
        /// frame's recomputed entries at use.
        choice: usize,
    },
    /// A new field on the record at `parent` whose label is being
    /// authored; resolving the label advances to the value stage (or
    /// selects the existing field if the label is taken).
    PendingEdge {
        parent: Path,
        query: LineEditState,
        choice: usize,
    },
}

impl Selection {
    /// Select the value at `path`; a string value — or a cell's name
    /// at a Name step — brings a focused editor (the root included —
    /// its commits target the document's root field). Selecting an
    /// EMPTY VALUE SLOT is already authoring it — there is nothing
    /// there to select, only something to begin, so it pends
    /// immediately: the empty document's root, and a valueless
    /// writable cell's Follow slot (its rendered placeholder).
    pub fn edge(sources: &Sources, path: Path) -> Self {
        let empty_slot = match path.split_last() {
            None => sources.root().is_none(),
            Some((Step::Follow, parent)) => sources
                .resolve(parent)
                .and_then(Value::as_cell)
                .is_some_and(|cell| {
                    sources.value(cell).is_none() && sources.writable(cell)
                }),
            _ => false,
        };
        if empty_slot {
            return pending_value(path);
        }
        // An editor mounts only where write-through can land: the
        // owning cell must not be external.
        let edit = writable_at(sources, &path)
            .then(|| match path.split_last() {
                // An unnamed cell mounts an EMPTY name editor: typing
                // names it.
                Some((Step::Name, parent)) => sources
                    .resolve(parent)
                    .and_then(Value::as_cell)
                    .map(|cell| line_edit(sources.name(cell).unwrap_or(""), NAME_COLOR)),
                _ => sources
                    .resolve(&path)
                    .and_then(|value| value.as_str().map(|s| line_edit(s, STRING_COLOR))),
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

/// The index of the path's last Follow step: the identity crossing
/// every write below it lands through. The link before it names the
/// owning cell; everything after it is a value spine.
fn last_follow(path: &[Step]) -> Option<usize> {
    path.iter().rposition(|step| matches!(step, Step::Follow))
}

/// Whether a write at `path` can land: the owning cell — the one the
/// path's last Follow crosses into, or for a Name step the named
/// cell itself — must not be external. A path with no Follow is the
/// document's own root spine and always writable.
fn writable_at(sources: &Sources, path: &[Step]) -> bool {
    match path.split_last() {
        Some((Step::Name, parent)) => sources
            .resolve(parent)
            .and_then(Value::as_cell)
            .is_some_and(|cell| sources.writable(cell)),
        _ => match last_follow(path) {
            Some(index) => sources
                .resolve(&path[..index])
                .and_then(Value::as_cell)
                .is_some_and(|cell| sources.writable(cell)),
            None => true,
        },
    }
}

/// Deletes the value at `path`. A field or element step rebuilds the
/// owning value without it — unlinking; anything the dropped value
/// linked stays in the table for the orphan pool. A trailing Follow
/// removes the cell's own value: bare again, the symmetric partner
/// of authoring a value into one. The empty path empties the
/// document's root; paths that no longer resolve decline.
pub fn delete_edge(doc: &mut Document, library: &Cells, path: &[Step]) -> bool {
    match path.split_last() {
        None => doc.root.take().is_some(),
        Some((Step::Follow, parent)) => {
            let cell = {
                let sources = Sources { doc: &*doc, library };
                sources
                    .resolve(parent)
                    .and_then(Value::as_cell)
                    .filter(|cell| sources.writable(*cell))
                    .filter(|cell| doc.cells.value(*cell).is_some())
            };
            match cell {
                Some(cell) => {
                    doc.cells.clear_value(cell);
                    true
                }
                None => false,
            }
        }
        // Names are not edges: nothing to detach. Un-naming is
        // emptying the name editor — the empty string being
        // no-name's one spelling.
        Some((Step::Name, _)) => false,
        Some((Step::Key(_) | Step::Element(_), _)) => {
            let write = {
                let sources = Sources { doc: &*doc, library };
                match last_follow(path) {
                    Some(index) => sources
                        .resolve(&path[..index])
                        .and_then(Value::as_cell)
                        .filter(|cell| sources.writable(*cell))
                        .and_then(|cell| {
                            spine::without(sources.value(cell)?, &path[index + 1..])
                                .map(|rebuilt| (Some(cell), rebuilt))
                        }),
                    None => sources
                        .root()
                        .and_then(|root| spine::without(root, path))
                        .map(|rebuilt| (None, rebuilt)),
                }
            };
            match write {
                Some((Some(cell), rebuilt)) => {
                    doc.cells.set_value(cell, rebuilt);
                    true
                }
                Some((None, rebuilt)) => {
                    doc.root = Some(rebuilt);
                    true
                }
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

/// A value-stage pending: the location named by `path` does not
/// exist, and its value is being authored.
pub fn pending_value(path: Path) -> Selection {
    Selection::Pending {
        path,
        query: line_edit("", QUERY_COLOR),
        choice: 0,
    }
}

/// A new field on the record at `parent` — inline, or a link's cell
/// value, normalized through Follow so the pending lands where the
/// field will live. Only records take fields, by type. EXTERNAL
/// cells — the library the authority — decline: a lone document
/// value would shadow the library's whole statement (per-cell
/// fallback), silently de-naming the conventions. A document that
/// owns the cell (a fork, copy/paste's job) authors freely.
pub fn pending_edge(sources: &Sources, parent: Path) -> Option<Selection> {
    let value = sources.resolve(&parent)?;
    let parent = match value {
        Value::Record(_) => parent,
        Value::Atom(atom) => {
            let cell = atom.as_cell()?;
            sources.value(cell)?.as_record()?;
            let mut followed = parent;
            followed.push(Step::Follow);
            followed
        }
        Value::List(_) => return None,
    };
    writable_at(sources, &parent).then_some(())?;
    Some(Selection::PendingEdge {
        parent,
        query: line_edit("", QUERY_COLOR),
        choice: 0,
    })
}

/// A valueless cell's value being authored: the within-gesture's
/// meaning on a cell with nothing held yet (bare, or named bare —
/// the red link being filled in).
pub fn pending_follow(sources: &Sources, path: &[Step]) -> Option<Selection> {
    let cell = sources.resolve(path)?.as_cell()?;
    sources.value(cell).is_none().then_some(())?;
    sources.writable(cell).then_some(())?;
    let mut followed = path.to_vec();
    followed.push(Step::Follow);
    Some(pending_value(followed))
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
    // Stated, not incidental: a list under an external cell takes no
    // minted siblings (the write would decline anyway, but a pending
    // that opens and cannot commit is an affordance lie).
    writable_at(sources, parent_path).then_some(())?;
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

/// A pending element inside the list at `path` — inline, or a link's
/// cell value, normalized through Follow — appended at the end or
/// prepended at the front. Only lists take elements, by type, and
/// the owning cell must be writable, as in [`pending_edge`].
fn pending_into_at(sources: &Sources, path: &[Step], end: bool) -> Option<Selection> {
    let value = sources.resolve(path)?;
    let (list_path, elements) = match value {
        Value::List(elements) => (path.to_vec(), elements),
        Value::Atom(atom) => {
            let cell = atom.as_cell()?;
            let elements = sources.value(cell)?.as_list()?;
            let mut followed = path.to_vec();
            followed.push(Step::Follow);
            (followed, elements)
        }
        Value::Record(_) => return None,
    };
    writable_at(sources, &list_path).then_some(())?;
    let positions: Vec<&Position> = elements.keys().collect();
    let fresh = if end {
        position::between(positions.last().copied(), None)?
    } else {
        position::between(None, positions.first().copied())?
    };
    let mut fresh_path = list_path;
    fresh_path.push(Step::Element(fresh));
    Some(pending_value(fresh_path))
}

/// Appends: "add to this list" goes at the end — the within chord's
/// meaning on a list, where fields don't exist.
pub fn pending_into(sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_into_at(sources, path, true)
}

pub fn pending_into_first(sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_into_at(sources, path, false)
}

/// Plain Enter: a new peer BESIDE the selection — continue the
/// enumeration you are in. An element pends a sibling (before with
/// shift); a field value pends a new field on its parent; the root
/// has nothing beside it and falls within — a field on a record, an
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

/// The command chord: author WITHIN the selection — a new field on
/// the selected record or cell, an element appended into a list, or
/// a bare cell's first value. With shift, the front instead —
/// prepend. Atoms other than links have no within and decline.
pub fn pending_insert(sources: &Sources, path: &[Step], front: bool) -> Option<Selection> {
    if front {
        pending_into_first(sources, path)
    } else {
        pending_edge(sources, path.to_vec())
            .or_else(|| pending_into(sources, path))
            .or_else(|| pending_follow(sources, path))
    }
}

/// A pending root for an empty document.
pub fn pending_root(sources: &Sources) -> Option<Selection> {
    sources.root().is_none().then(|| pending_value(Vec::new()))
}

/// The bytes a `0x` query denotes: hex digits, any case (the value
/// is the bytes; lowercase is the canonical spelling), whole bytes
/// only.
fn parse_blob(text: &str) -> Option<Vec<u8>> {
    let hex = text.strip_prefix("0x")?;
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    hex.len().is_multiple_of(2).then_some(())?;
    hex.as_bytes()
        .chunks(2)
        .map(|pair| Some(digit(pair[0])? << 4 | digit(pair[1])?))
        .collect()
}

/// The value a pending query resolves to: a leading quote forces a
/// string (the closing quote optional, so string mode holds while
/// typing), `0x` hex reads as a blob, anything else is the string as
/// typed.
pub fn resolve_query(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.strip_prefix('"') {
        Some(inner) => Value::from(inner.strip_suffix('"').unwrap_or(inner)),
        None => parse_blob(trimmed)
            .map(Value::from)
            .unwrap_or_else(|| Value::from(text)),
    }
}

/// The clipboard spelling of a value — SHALLOW by design: one value,
/// a link being its identity alone, no cell values traveling (deep
/// copy waits on the projection-boundary design; see docs/model.md).
/// Strings and blobs spell as the query language — "quoted" strings,
/// `0x` hex — so they read in other apps and [`from_clipboard`] reads
/// them back; links, lists, and records spell as Value JSON.
pub fn to_clipboard(value: &Value) -> String {
    match value {
        Value::Atom(Atom::String(_) | Atom::Blob(_)) => value.to_string(),
        _ => serde_json::to_string(value).expect("values serialize"),
    }
}

/// The value a clipboard text denotes: Value JSON when it parses,
/// else the query reading — quoted strings, `0x` blobs, bare text —
/// so text copied anywhere pastes sensibly.
pub fn from_clipboard(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| resolve_query(text))
}

/// A completion offer on a pending. The display styles itself by the
/// action's kind at draw time.
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
    /// Mint a cell and commit a link to it. Named, the cell's table
    /// entry starts as the name alone — the red link; unnamed, it
    /// starts fully bare, nothing said at all.
    NewCell { name: Option<String> },
    /// Commit an empty list value.
    NewList,
    /// Commit an empty inline record value — anonymous structure, no
    /// cell minted.
    NewRecord,
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
/// by the fuzzy tiers), and a fresh cell — named after the query when
/// there is one, the create-on-reference of the floating-definitions
/// design. The label stage (`labels`) offers only what can label:
/// strings and cell references — no blobs, and "new list"/"new
/// record" stay value offers.
fn completion_entries(
    sources: &Sources,
    names: &Names,
    raw: bool,
    labels: bool,
    query: &str,
) -> Vec<Entry> {
    let trimmed = query.trim();
    let quoted = trimmed.trim_start().starts_with('"');
    let blob = (!labels).then(|| parse_blob(trimmed)).flatten();
    let atom = match (&blob, labels) {
        (Some(bytes), _) => Value::from(bytes.clone()),
        (None, false) => resolve_query(query),
        // A label is a name: quotes strip, everything else is the
        // text as typed — never a blob.
        (None, true) => match trimmed.strip_prefix('"') {
            Some(inner) => Value::from(inner.strip_suffix('"').unwrap_or(inner)),
            None => Value::from(query),
        },
    };
    // Quotes and `0x` state atom intent, so the atom leads; otherwise
    // a confident (non-fuzzy) reference match is likelier the intent
    // than a new literal — typing a visible name or short id should
    // default to the reference, and quoting always forces the string.
    let atom_leads = quoted || blob.is_some();
    // The typed text is always insertable as itself: a blob query
    // offers its string form right below the blob (a quote already
    // states string intent, so quoted queries stay string-only).
    let string_entry = blob.is_some().then(|| Entry {
        display: format!("\"{query}\""),
        detail: None,
        matches: Vec::new(),
        action: EntryAction::Value(Value::from(query)),
    });
    let atom_entry = Entry {
        display: match &atom {
            Value::Atom(Atom::String(s)) => format!("\"{s}\""),
            other => other.to_string(),
        },
        detail: None,
        matches: Vec::new(),
        action: EntryAction::Value(atom),
    };
    // Every cell the document contains is referenceable: named ones
    // by name, unnamed ones by the short id they render as — what
    // you see is what you can type. Unnamed keys start with the
    // ellipsis, which sorts after names, so they trail on an empty
    // query. "new list" and "new record" rank among them under their
    // own display text: type toward one and it surfaces, type away
    // and it leaves.
    let mut references_pool: Vec<(String, EntryAction)> = document_cells(sources)
        .into_iter()
        .map(|cell| {
            let key = crate::conventions::display_name(sources, names, raw, cell)
                .unwrap_or_else(|| short_id(cell));
            (key, EntryAction::Value(Value::from(cell)))
        })
        .collect();
    references_pool.sort_by(|a, b| a.0.cmp(&b.0));
    if !labels {
        references_pool.push(("new list".to_string(), EntryAction::NewList));
        references_pool.push(("new record".to_string(), EntryAction::NewRecord));
    }
    let references: Vec<(Entry, bool)> = filter::rank(references_pool, |(key, _)| key, query)
        .into_iter()
        .take(8)
        .map(|ranked| {
            let fuzzy = ranked.fuzzy();
            let matches = ranked.matches;
            let (display, action) = ranked.item;
            let detail = match &action {
                EntryAction::Value(value) => value
                    .as_cell()
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
    let name_text = trimmed
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(trimmed);
    let name = (!name_text.is_empty()).then(|| name_text.to_string());
    entries.push(Entry {
        display: match &name {
            Some(name) => format!("new cell \"{name}\""),
            None => "new cell".to_string(),
        },
        detail: None,
        matches: Vec::new(),
        action: EntryAction::NewCell { name },
    });
    entries
}

/// The cells a value links, walked structurally — lists and records
/// are values, so their contents are right here; record labels
/// reference too.
fn value_cells(value: &Value, cells: &mut Vec<CellId>) {
    match value {
        Value::Atom(atom) => cells.extend(atom.as_cell()),
        Value::List(elements) => {
            for element in elements.values() {
                value_cells(element, cells);
            }
        }
        Value::Record(fields) => {
            for (label, field) in fields {
                cells.extend(label.as_cell());
                value_cells(field, cells);
            }
        }
    }
}

/// Every cell the document or its library mentions — table entries,
/// links inside values, cells used as labels, and the root's own
/// links. Bare cells referenced anywhere are included: still
/// referenceable. Library cells are offered so the conventions are
/// typeable from keystroke one. Sorted for a deterministic offer
/// order.
fn document_cells(sources: &Sources) -> Vec<CellId> {
    let mut cells = Vec::new();
    for cell in sources.cells() {
        cells.push(*cell);
        if let Some(value) = sources.value(*cell) {
            value_cells(value, &mut cells);
        }
    }
    if let Some(root) = sources.root() {
        value_cells(root, &mut cells);
    }
    cells.sort();
    cells.dedup();
    cells
}

/// Resolves a chosen entry to the value it denotes, minting (and
/// naming) for a new cell. Labels and values resolve alike — the
/// label stage never offers a non-label action.
pub fn resolve_entry(doc: &mut Document, action: &EntryAction) -> Value {
    match action {
        EntryAction::Value(value) => value.clone(),
        EntryAction::NewCell { name } => {
            let cell = new_cell_id();
            if let Some(name) = name {
                doc.cells.set_name(cell, name);
            }
            Value::from(cell)
        }
        EntryAction::NewList => Value::list([]),
        EntryAction::NewRecord => Value::record([]),
    }
}

/// Commits a pending from a chosen entry: resolves the action to a
/// value and writes it.
pub fn commit_pending(
    doc: &mut Document,
    library: &Cells,
    path: &[Step],
    action: &EntryAction,
) -> bool {
    let value = resolve_entry(doc, action);
    set_value(doc, library, path, value)
}

/// Writes `value` at `path` — the empty path writes the document
/// root. The single write every edit reduces to: the path's last
/// Follow names the owning, authority-gated cell; the steps below it
/// are a value spine, rebuilt around the new leaf through the lens.
/// A bare cell takes its first value through the empty spine.
pub fn set_value(doc: &mut Document, library: &Cells, path: &[Step], value: Value) -> bool {
    let write = {
        let sources = Sources { doc: &*doc, library };
        match last_follow(path) {
            Some(index) => sources
                .resolve(&path[..index])
                .and_then(Value::as_cell)
                .filter(|cell| sources.writable(*cell))
                .and_then(|cell| {
                    spine::set(sources.value(cell), &path[index + 1..], value)
                        .map(|rebuilt| (Some(cell), rebuilt))
                }),
            None => spine::set(sources.root(), path, value).map(|rebuilt| (None, rebuilt)),
        }
    };
    match write {
        Some((Some(cell), rebuilt)) => {
            doc.cells.set_value(cell, rebuilt);
            true
        }
        Some((None, rebuilt)) => {
            doc.root = Some(rebuilt);
            true
        }
        None => false,
    }
}

/// Writes the name of the cell the path's trailing Name step
/// addresses — the empty string un-names, its canonical spelling.
/// Names are identity metadata, so the write targets the named cell
/// directly, gated on its own authority; clearing a name that isn't
/// there declines, keeping no-ops distinguishable.
pub fn set_name(doc: &mut Document, library: &Cells, path: &[Step], name: &str) -> bool {
    let cell = {
        let sources = Sources { doc: &*doc, library };
        match path.split_last() {
            Some((Step::Name, parent)) => sources
                .resolve(parent)
                .and_then(Value::as_cell)
                .filter(|cell| sources.writable(*cell))
                .filter(|cell| !name.is_empty() || sources.name(*cell).is_some()),
            _ => None,
        }
    };
    match cell {
        Some(cell) => {
            doc.cells.set_name(cell, name);
            true
        }
        None => false,
    }
}

/// Toggle the collapse override for the value at `path`, staying
/// sparse: an override matching the default (collapsed inside a
/// cycle, expanded otherwise) is removed rather than stored. Declines
/// unless there is something to collapse — a cell with a value, or a
/// nonempty list or record.
pub fn toggle_collapse(sources: &Sources, collapse: &mut Collapse, path: &[Step]) -> bool {
    sources
        .resolve(path)
        .filter(|value| match value {
            Value::Atom(atom) => atom
                .as_cell()
                .and_then(|cell| sources.value(cell))
                .is_some(),
            Value::List(elements) => !elements.is_empty(),
            Value::Record(fields) => !fields.is_empty(),
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
/// every handled event — the graph is the source of truth. The
/// edited kind follows the current value: only strings mount
/// editors, and they write every keystroke. Everything funnels
/// through [`set_value`], so an element edit rebuilds its list at
/// the owning cell and a location that no longer takes the write
/// drops it silently — the malformed-graph rule at the mutation
/// boundary. Returns whether this write OPENED an undo step: true
/// exactly on the first write of the mounted editor's life, so a
/// typing run is one step and history stays a dumb stack.
pub fn write_through(doc: &mut Document, library: &Cells, selection: &mut Selection) -> bool {
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
    let wrote = match path.split_last() {
        // A Name step edits the cell's name — the same run shape,
        // through the name write instead of the value write. The
        // empty string spells no name: an empty editor over an
        // unnamed cell writes nothing, and emptying a name un-names
        // live.
        Some((Step::Name, parent)) => {
            let current = {
                let sources = Sources { doc: &*doc, library };
                sources
                    .resolve(parent)
                    .and_then(Value::as_cell)
                    .and_then(|cell| sources.name(cell))
                    .map(str::to_owned)
                    .unwrap_or_default()
            };
            current != text && set_name(doc, library, path, &text)
        }
        _ => {
            let (current, next) = {
                let sources = Sources { doc: &*doc, library };
                let current = sources.resolve(path);
                let next = match current {
                    Some(Value::Atom(Atom::String(_))) => Some(Value::from(text)),
                    _ => None,
                };
                (current.cloned(), next)
            };
            match next {
                Some(next) => {
                    current.as_ref() != Some(&next) && set_value(doc, library, path, next)
                }
                None => false,
            }
        }
    };
    if wrote {
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

/// Marks `child` as the projection of the value at `path`. On placement
/// it draws the highlight when this is the selected path, registers a
/// click that selects it (innermost wins by handler precedence) — or,
/// with the command modifier and a pending open, picks `value` into it
/// — and records itself for keyboard navigation.
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

/// The value marked as the secondary selection: the one at the
/// selected path. A value can project in many places — links, but
/// equally strings, blobs, and equal lists — and the marks make that
/// sameness visible. Inline records are structure, not identity: no
/// marks. A selected name marks as its string.
fn secondary_of(sources: &Sources, selection: Option<&Selection>) -> Option<Value> {
    match selection? {
        Selection::Edge { path, .. } => match path.split_last() {
            Some((Step::Name, parent)) => sources
                .resolve(parent)
                .and_then(Value::as_cell)
                .and_then(|cell| sources.name(cell))
                .map(Value::from),
            _ => sources
                .resolve(path)
                .filter(|value| !matches!(value, Value::Record(_)))
                .cloned(),
        },
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
        // The graph view's selected cell is a secondary here too:
        // its projections are the same value.
        secondary: secondary_of(sources, selection).or_else(|| graph_node.cloned()),
    };
    // The Raw view derives from the one bit: names answer None and
    // nothing else changes — lists and records render as themselves
    // there too, since kind is data, not convention. An empty
    // document is a selectable placeholder at the root path.
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

/// A link rendered as its cell: PARENS are the cell's syntax — `(`
/// name-or-short-id value `)` — completing the delimiter family
/// (brackets say list, braces say record). The name, when there is
/// one, is the identity's own metadata projected at the Name step —
/// selectable, editable, two-stage. Leaf and inline values sit
/// between head and close paren; a record or list value is the SAME
/// record or list view, held here through [`Held`] — one rendering
/// per container kind, whatever holds it. A valueless cell — bare,
/// or the named red link — renders the pending placeholder in the
/// value's place (the empty-slot rule in [`Selection::edge`] makes
/// selecting it begin the first value). Clicks on the parens and
/// gaps fall through to the cell's own descend. Collapsed (by
/// default in a cycle, or forced by an override), containers show
/// their usual summary inside the parens and an atom elides
/// entirely.
fn cell_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    cell: CellId,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let name = cx.name(cell);
    let head = row(
        2.0 * scale,
        vec![
            text(tcx, "(", &cx.styles.dim),
            head_view(cx, tcx, path, cell, &name, hooks),
        ],
    );
    let mut followed = path.to_vec();
    followed.push(Step::Follow);
    let Some(value) = cx.sources.value(cell).cloned() else {
        return row(
            4.0 * scale,
            vec![
                head,
                row(
                    2.0 * scale,
                    vec![
                        pending_view(cx, tcx, followed.clone(), hooks),
                        text(tcx, ")", &cx.styles.dim),
                    ],
                ),
            ],
        );
    };
    let mut inner = ancestors.clone();
    inner.insert(cell);
    // A pending inside the value forces the cell open.
    let pending_inside = cx.pending_child_of(&followed).is_some()
        || cx.pending_edge_under(&followed).is_some();
    let collapsed =
        !pending_inside && cx.collapse.collapsed(path, ancestors.contains(&cell));
    let held = Held {
        head,
        path: path.to_vec(),
        collapsed,
    };
    match &value {
        Value::Atom(_) if collapsed => row(
            4.0 * scale,
            vec![
                held.head,
                disclosure(path.to_vec(), true, hooks, cx.styles),
                text(tcx, ")", &cx.styles.dim),
            ],
        ),
        // Leaf-only records and lists (pendings included) read
        // inline between the parens, as do atoms and links.
        Value::Atom(_) => inline_cell(cx, tcx, held.head, &followed, &inner, &value, hooks),
        Value::Record(fields)
            if !collapsed
                && fields.values().all(leaf_atom)
                && cx.pending_edge_under(&followed).is_none() =>
        {
            inline_cell(cx, tcx, held.head, &followed, &inner, &value, hooks)
        }
        Value::List(elements) if !collapsed && elements.values().all(leaf_atom) => {
            inline_cell(cx, tcx, held.head, &followed, &inner, &value, hooks)
        }
        Value::Record(fields) => {
            record_view(cx, tcx, &followed, &inner, fields, Some(held), hooks)
        }
        Value::List(elements) => {
            list_view(cx, tcx, &followed, &inner, elements, Some(held), hooks)
        }
    }
}

/// A container view held by a cell: the parenthesized head joins the
/// container's delimiter line, the close paren its closer (`})`,
/// `])`), and collapse — decided by the cell, cycle default and
/// pending-forcing included — keys its disclosure at the cell's own
/// path.
struct Held<P> {
    head: Node<P>,
    path: Path,
    collapsed: bool,
}

/// Leaf atoms — strings and blobs — read inline; anything with
/// interior structure or identity blocks.
fn leaf_atom(value: &Value) -> bool {
    value.as_str().is_some() || value.as_blob().is_some()
}

/// A cell whose value reads on one line: `(head value)`.
fn inline_cell<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    head: Node<P>,
    followed: &[Step],
    ancestors: &HashSet<CellId>,
    value: &Value,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    row(
        4.0 * scale,
        vec![
            head,
            row(
                2.0 * scale,
                vec![
                    value_view(cx, tcx, followed, ancestors, value, hooks),
                    text(tcx, ")", &cx.styles.dim),
                ],
            ),
        ],
    )
}

/// Registers a held value at its own (Follow) path — keyboard-
/// reachable and primary-highlighted, no pointer target: clicks
/// belong to the rows and the cell.
fn held_body<P: Canvas + HasDescends>(cx: &Cx, path: Path, body: Node<P>) -> Node<P> {
    let selected = cx.selected(&path);
    let scale = cx.styles.scale;
    decorate(body, move |p: &mut P, rect| {
        if selected {
            primary_highlight(scale, p, rect);
        }
        p.descends().push(Descend {
            path: path.clone(),
            rect,
        });
    })
}

/// A cell's head: the name — identity metadata at the Name step —
/// projected as the header text, or the short id when unnamed. It
/// selects, edits, marks, and deletes at the Name step; deleting it
/// un-names the cell, and an unnamed cell's id engages as an EMPTY
/// name editor, so typing names it — click to select, click again
/// to (re)name, the Finder pattern either way.
///
/// The head text stands for the CELL until the cell is selected: a
/// cold click falls through to the block's own target and selects the
/// cell, and only then does the text engage as a target. The pass
/// decides from current state; single-shot dispatch means the second
/// click always sees the engaged successor. Cold, the head stays
/// keyboard-reachable (and markable, when named), just not a pointer
/// target.
fn head_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    cell: CellId,
    name: &Option<String>,
    hooks: &Hooks<C>,
) -> Node<P> {
    let mut edge = path.to_vec();
    edge.push(Step::Name);
    let editing = cx
        .selection
        .filter(|selection| selection.path() == edge.as_slice())
        .and_then(Selection::edit);
    let short = short_id(cell);
    let fallback = match name {
        Some(name) => text(tcx, name, &cx.styles.name),
        None => text(tcx, &short, &cx.styles.id),
    };
    // While the name buffer is empty, the short id ghosts in place —
    // the field keeps its width and shows what an empty name falls
    // back to.
    let content = atom_content(
        editing,
        fallback,
        Some((&short, &cx.styles.id)),
        tcx,
        cx.styles,
        hooks,
    );
    // The name string is the identity the head marks and picks; an
    // unnamed head stands in for the cell itself and marks nothing.
    let mark = name.as_ref().map(|name| Value::from(name.as_str()));
    let target = mark.clone().unwrap_or_else(|| Value::from(cell));
    if cx.selected(path) || cx.selected(&edge) {
        let content = cursor_target(edge.clone(), target.clone(), hooks, content);
        let content = match &mark {
            Some(value) if !cx.selected(&edge) => secondary_mark(cx, value, content),
            _ => content,
        };
        descend(cx, edge, Some(target), hooks, content)
    } else {
        let content = match &mark {
            Some(value) => secondary_mark(cx, value, content),
            None => content,
        };
        decorate(content, move |p: &mut P, rect| {
            p.descends().push(Descend { path: edge, rect });
        })
    }
}

/// One record field row: the label-and-arrow head, then the value (or
/// its pending query). `parent` is the record's own path — a cell's
/// followed path or an inline record's. A real field's label and
/// arrow select the field, like its value — grouped so one target
/// spans both and the gap between. A pending row's plain click falls
/// through (the not-yet-field can't be selected), but command still
/// picks its label's identity.
fn field_row<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    parent: &[Step],
    ancestors: &HashSet<CellId>,
    key: Label,
    value: Option<Value>,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let mut child = parent.to_vec();
    child.push(Step::Key(key.clone()));
    let head = row(6.0 * scale, vec![label_view(cx, tcx, &key), arrow(cx.styles)]);
    let head = match &value {
        Some(_) => select_target(
            child.clone(),
            Value::Atom(Atom::from(key.clone())),
            hooks,
            head,
        ),
        None => pick_target(key.clone(), hooks, head),
    };
    let content = match value {
        Some(value) => value_view(cx, tcx, &child, ancestors, &value, hooks),
        None => pending_view(cx, tcx, child, hooks),
    };
    row(6.0 * scale, vec![head, content])
}

/// The label-query row of a new field being authored on a record. The
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
/// session identity, not information; order carries it. A leaf-atom
/// list reads as an inline literal; collapsed shows the element
/// count. Lists have no identity, so there is no head of their own
/// and no cycle through them — only linked cells can recurse — but a
/// cell HOLDING a list frames this same view through [`Held`].
fn list_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    elements: &OrdMap<Position, Value>,
    held: Option<Held<P>>,
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

    // A leaf-atom list reads as a literal: `["a", "b"]` on one line,
    // dim punctuation, each element still an ordinary descend (click,
    // edit, secondary-mark) — a pending one included. Links, records,
    // and nested lists take the block form; width-aware grouping
    // (Wadler) waits, so a long list will run wide for now. A held
    // call is block by dispatch; the guard keeps the frame from being
    // dropped if that ever drifts.
    let inline = held.is_none()
        && items
            .iter()
            .all(|(_, value)| value.as_ref().is_none_or(leaf_atom));
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

    // Standalone, a pending child forces the list open and only an
    // override collapses it (no identity to be an ancestor); held,
    // the cell decided.
    let framed = held.is_some();
    let (delta_path, collapsed, close) = match &held {
        Some(held) => (held.path.clone(), held.collapsed, "])"),
        None => (
            path.to_vec(),
            items.iter().all(|(_, value)| value.is_some())
                && cx.collapse.collapsed(path, false),
            "]",
        ),
    };
    let mut header: Vec<Node<P>> = held.map(|held| held.head).into_iter().collect();
    header.push(text(tcx, "[", &cx.styles.dim));
    header.push(disclosure(delta_path, collapsed, hooks, cx.styles));
    if collapsed {
        header.push(text(tcx, &count_text(items.len(), "element"), &cx.styles.dim));
        header.push(text(tcx, close, &cx.styles.dim));
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
    let body = col(HAlign::Start, 0, 4.0 * scale, rows);
    let body = if framed {
        held_body(cx, path.to_vec(), body)
    } else {
        body
    };
    // The block form closes its bracket at the header's indent.
    col(
        HAlign::Start,
        0,
        4.0 * scale,
        vec![
            row(4.0 * scale, header),
            pad(Insets::new(26.0 * scale, 0.0, 0.0, 0.0), body),
            text(tcx, close, &cx.styles.dim),
        ],
    )
}

/// A record value: an anonymous content-compared value, BRACED —
/// braces mark records the way parens mark cells. Field rows at the
/// record's own path. An all-leaf-atom record reads as an inline
/// literal `{x: "1", y: "2"}`; standalone collapse is override-only,
/// since a value has no identity to recur through; a cell HOLDING a
/// record frames this same view through [`Held`].
fn record_view<C: 'static, P: Canvas + HasHandler<C> + HasDescends + HasPopup>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    fields: &OrdMap<Label, Value>,
    held: Option<Held<P>>,
    hooks: &Hooks<C>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let mut items: Vec<(Label, Option<Value>)> = fields
        .iter()
        .map(|(key, value)| (key.clone(), Some(value.clone())))
        .collect();
    if let Some(Step::Key(key)) = cx.pending_child_of(path) {
        items.push((key, None));
        items.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let pending_edge = cx.pending_edge_under(path).is_some();

    let inline = held.is_none()
        && !pending_edge
        && items
            .iter()
            .all(|(_, value)| value.as_ref().is_none_or(leaf_atom));
    if inline {
        let mut cells: Vec<Node<P>> = vec![text(tcx, "{", &cx.styles.dim)];
        for (index, (key, value)) in items.into_iter().enumerate() {
            if index > 0 {
                cells.push(text(tcx, ", ", &cx.styles.dim));
            }
            cells.push(label_view(cx, tcx, &key));
            cells.push(text(tcx, ": ", &cx.styles.dim));
            let mut child = path.to_vec();
            child.push(Step::Key(key));
            cells.push(match value {
                Some(value) => value_view(cx, tcx, &child, ancestors, &value, hooks),
                None => pending_view(cx, tcx, child, hooks),
            });
        }
        cells.push(text(tcx, "}", &cx.styles.dim));
        return row(0.0, cells);
    }

    let framed = held.is_some();
    let (delta_path, collapsed, close) = match &held {
        Some(held) => (held.path.clone(), held.collapsed, "})"),
        None => (
            path.to_vec(),
            !pending_edge
                && items.iter().all(|(_, value)| value.is_some())
                && cx.collapse.collapsed(path, false),
            "}",
        ),
    };
    let mut header: Vec<Node<P>> = held.map(|held| held.head).into_iter().collect();
    header.push(text(tcx, "{", &cx.styles.dim));
    header.push(disclosure(delta_path, collapsed, hooks, cx.styles));
    if collapsed {
        header.push(text(tcx, &count_text(items.len(), "field"), &cx.styles.dim));
        header.push(text(tcx, close, &cx.styles.dim));
        return row(4.0 * scale, header);
    }
    let mut rows: Vec<Node<P>> = items
        .into_iter()
        .map(|(key, value)| field_row(cx, tcx, path, ancestors, key, value, hooks))
        .collect();
    // A new field being authored: the label query, unsorted until it
    // has a label to sort by.
    if let Some((query, choice)) = cx.pending_edge_under(path) {
        rows.push(pending_edge_row(cx, tcx, query, choice, hooks));
    }
    let body = col(HAlign::Start, 0, 4.0 * scale, rows);
    let body = if framed {
        held_body(cx, path.to_vec(), body)
    } else {
        body
    };
    col(
        HAlign::Start,
        0,
        4.0 * scale,
        vec![
            row(4.0 * scale, header),
            pad(Insets::new(26.0 * scale, 0.0, 0.0, 0.0), body),
            text(tcx, close, &cx.styles.dim),
        ],
    )
}

fn count_text(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// Git-style short form of a cell id: an ellipsis and the last five
/// hex digits, fixed length even where fewer would disambiguate.
/// A collision within a document is unlikely (about 0.5% somewhere in
/// a hundred-cell document) and the display can grow if it ever
/// matters.
pub fn short_id(id: CellId) -> String {
    let hex = id.simple().to_string();
    format!("…{}", &hex[hex.len() - 5..])
}

/// A blob's display: `0x` and its bytes, truncated past sixteen —
/// the mini hex editor is a later projection; this is the floor.
fn blob_text(bytes: &[u8]) -> String {
    if bytes.len() <= 16 {
        format!("0x{}", hex_string(bytes))
    } else {
        format!("0x{}… ({} bytes)", hex_string(&bytes[..8]), bytes.len())
    }
}

fn label_view<P: Canvas>(cx: &Cx, tcx: &mut TextCtx, key: &Label) -> Node<P> {
    let inner = match key {
        // A string label wears its quotes: it IS a string, and the
        // quotes are what distinguish it from a cell label read by
        // name (the open styling question, answered 2026-07-20).
        Label::String(s) => text(tcx, &format!("\"{s}\""), &cx.styles.label),
        // A named cell used as a label reads by its name, through the
        // editor's one name policy.
        Label::Cell(cell) => match cx.name(*cell) {
            Some(name) => text(tcx, &name, &cx.styles.label),
            None => text(tcx, &short_id(*cell), &cx.styles.id),
        },
    };
    secondary_mark(cx, &Value::Atom(Atom::from(key.clone())), inner)
}

/// A cell projection's ground, painted only at authority
/// TRANSITIONS: an external cell under document authority takes the
/// dark tint — no lock, just "from elsewhere" — and a
/// document-authority cell under an external one takes its light
/// ground back (opaque, since an alpha wash can't be undone by
/// another wash). Runs of the same authority draw nothing, so
/// nesting never stacks tints. The enclosing authority is the owning
/// cell at the path's last Follow, so a cell inside a list carries
/// its list's owner as context. Wraps outside the descend so the
/// cell's own selection highlight draws over its ground.
fn ground<P: Canvas>(cx: &Cx, path: &[Step], value: &Value, content: Node<P>) -> Node<P> {
    let Some(cell) = value.as_cell() else {
        return content;
    };
    let external = cx.sources.external(cell);
    let parent_external = last_follow(path)
        .and_then(|index| cx.sources.resolve(&path[..index]))
        .and_then(Value::as_cell)
        .is_some_and(|cell| cx.sources.external(cell));
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
/// projection of the selected value — an expanded block, a collapsed
/// handle, or a label. The primary selection's geometry at lower
/// strength, so the two read as one family.
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

/// The disclosure delta to the right of a handle: down when expanded,
/// right when collapsed. Clicking it reports a toggle for this path
/// without selecting; the extent spans the header height so the
/// target is comfortable, and living outside the child indent it
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
/// separates a field's key from its target.
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
    ancestors: &HashSet<CellId>,
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
            let content = atom_content(editing, fallback, None, tcx, cx.styles, hooks);
            row(0.0, vec![
                text(tcx, "\"", &cx.styles.string),
                cursor_target(path.to_vec(), value.clone(), hooks, content),
                text(tcx, "\"", &cx.styles.string),
            ])
        }
        Value::Atom(Atom::Blob(bytes)) => text(tcx, &blob_text(bytes), &cx.styles.id),
        // The hardcoded projection chain, decided per value: links
        // render as their cells, lists and records as themselves —
        // in the Raw view too, kind being data. A registry waits for
        // user-defined projections.
        Value::Atom(Atom::Cell(cell)) => cell_view(cx, tcx, path, ancestors, *cell, hooks),
        Value::List(elements) => list_view(cx, tcx, path, ancestors, elements, None, hooks),
        Value::Record(fields) => record_view(cx, tcx, path, ancestors, fields, None, hooks),
    };
    // Other projections of the selected value carry the secondary
    // mark; the selected one has the primary highlight.
    let inner = if cx.selected(path) {
        inner
    } else {
        secondary_mark(cx, value, inner)
    };
    let placed = descend(cx, path.to_vec(), Some(value.clone()), hooks, inner);
    ground(cx, path, value, placed)
}

/// A nonexistent location the selection is authoring: the completion
/// query's focused editor, wrapped as an ordinary descend so it
/// highlights, clicks, and navigates like the value it may become.
/// Its placement emits the completion popup for the shell to draw
/// over the body.
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
/// stages — a value and a new field's label (`labels` narrows the
/// offers there).
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
    let content = atom_content(Some(query), fallback, None, tcx, cx.styles, hooks);
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
                EntryAction::Value(value) if value.as_blob().is_some() => &styles.id,
                EntryAction::Value(_) => &styles.label,
                EntryAction::NewCell { .. } | EntryAction::NewList | EntryAction::NewRecord => {
                    &styles.dim
                }
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
/// this atom is being edited — with `placeholder` as its ghost while
/// empty — its static text otherwise.
fn atom_content<C: 'static, P: Canvas + HasHandler<C> + HasDescends>(
    editing: Option<&LineEditState>,
    fallback: Node<P>,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    hooks: &Hooks<C>,
) -> Node<P> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            text_edit(line, true, &styles.edit, placeholder, tcx, move |c| edit_ctx(c))
        }
        None => fallback,
    }
}

/// A command-click pick target with no plain-click behavior — for
/// parts like a pending row's label, whose plain click deliberately
/// falls through.
fn pick_target<C: 'static, P: Canvas + HasHandler<C>>(
    key: Label,
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
                && pick(ctx, Value::Atom(Atom::from(key.clone())))
        });
    })
}

/// A plain click-to-select target for `path` — for parts like labels
/// and the cell star that select without carrying an editor click.
/// With the command modifier and a pending open, picks `value` — the
/// identity the part displays — into it instead.
fn select_target<C: 'static, P: Canvas + HasHandler<C>>(
    path: Path,
    value: Value,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    decorate(content, move |p, rect| {
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let picked = command(&event.state.modifiers)
                        && pick(ctx, value.clone());
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
    use ui_events::keyboard::{KeyState, Modifiers};

    fn src<'a>(doc: &'a Document, library: &'a Cells) -> Sources<'a> {
        Sources { doc, library }
    }

    fn key(s: &str) -> Step {
        Step::Key(Label::from(s))
    }

    /// A one-cell document: the root links a cell holding `fields`.
    fn doc_of(fields: Vec<(Label, Value)>) -> (Document, CellId) {
        let mut cells = Cells::new();
        let cell = new_cell_id();
        cells.set_value(cell, Value::record(fields));
        (
            Document {
                root: Some(Value::from(cell)),
                cells,
            },
            cell,
        )
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
    fn selecting_a_string_brings_an_editor() {
        let lib = Cells::new();
        let (mut doc, cell) = doc_of(vec![
            (Label::from("name"), Value::from("old")),
            (Label::from("x"), Value::from("1.5")),
        ]);
        let at = |doc: &Document, path: Vec<Step>| Selection::edge(&src(doc, &lib), path);
        assert!(at(&doc, vec![Step::Follow, key("name")]).edit().is_some());
        assert!(at(&doc, vec![Step::Follow, key("x")]).edit().is_some());
        // Missing fields, links, and blobs carry no editor.
        assert!(at(&doc, vec![Step::Follow, key("missing")]).edit().is_none());
        assert!(at(&doc, vec![]).edit().is_none());
        doc.cells
            .set_value(cell, Value::record([(Label::from("b"), Value::from(vec![0xff_u8]))]));
        assert!(at(&doc, vec![Step::Follow, key("b")]).edit().is_none());
        // A cell holding a string edits at its Follow path.
        doc.cells.set_value(cell, Value::from("held"));
        assert!(at(&doc, vec![Step::Follow]).edit().is_some());
        // A named cell's name edits at its Name path.
        doc.cells.set_name(cell, "roof");
        assert!(at(&doc, vec![Step::Name]).edit().is_some());
        assert!(at(&doc, vec![Step::Follow, Step::Name]).edit().is_none());
    }

    #[test]
    fn edits_write_through_to_the_field() {
        let lib = Cells::new();
        let (mut doc, _) = doc_of(vec![(Label::from("name"), Value::from("old"))]);
        let path = vec![Step::Follow, key("name")];
        let mut selection = Selection::edge(&src(&doc, &lib), path.clone());
        selection.edit_mut().unwrap().set_text("new");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(src(&doc, &lib).resolve(&path), Some(&Value::from("new")));
        // A selection without an editor writes nothing.
        let mut plain = Selection::edge(&src(&doc, &lib), vec![Step::Follow, key("missing")]);
        assert!(!write_through(&mut doc, &lib, &mut plain));
        assert_eq!(src(&doc, &lib).resolve(&path), Some(&Value::from("new")));
    }

    #[test]
    fn element_edits_rebuild_the_list_at_the_owning_cell() {
        let lib = Cells::new();
        let (mut doc, _) = doc_of(vec![(
            Label::from("dash"),
            Value::list([Value::from("2"), Value::from("3")]),
        )]);
        let list_path = vec![Step::Follow, key("dash")];
        let ps = positions(src(&doc, &lib).resolve(&list_path).unwrap());
        let element = vec![Step::Follow, key("dash"), Step::Element(ps[1].clone())];

        // Editing an element writes the whole rebuilt list at the
        // owning cell; the sibling keeps its position and value.
        let mut selection = Selection::edge(&src(&doc, &lib), element.clone());
        selection.edit_mut().unwrap().set_text("9");
        assert!(write_through(&mut doc, &lib, &mut selection));
        assert_eq!(src(&doc, &lib).resolve(&element), Some(&Value::from("9")));
        assert_eq!(
            src(&doc, &lib).resolve(&list_path),
            Some(&Value::list([Value::from("2"), Value::from("9")]))
        );
        assert_eq!(positions(src(&doc, &lib).resolve(&list_path).unwrap()), ps);
    }

    #[test]
    fn set_value_writes_fields_elements_roots_and_bare_cells() {
        let lib = Cells::new();
        let (mut doc, cell) = doc_of(vec![(Label::from("x"), Value::from("1"))]);
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("x")],
            Value::from("2")
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow, key("x")]),
            Some(&Value::from("2"))
        );

        // A fresh Key step INSERTS a field; a deep spine rebuilds
        // through nested records and lists.
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("at")],
            Value::record([(Label::from("row"), Value::from("top"))]),
        ));
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("at"), key("row")],
            Value::from("bottom")
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow, key("at"), key("row")]),
            Some(&Value::from("bottom"))
        );

        // The whole cell value is addressable at Follow: conversion
        // is one set.
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow],
            Value::list([Value::from("a")])
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow]),
            Some(&Value::list([Value::from("a")]))
        );

        // A bare cell takes its first value through the empty spine;
        // deeper steps into nothing decline.
        let bare = new_cell_id();
        doc.root = Some(Value::from(bare));
        assert!(!set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("x")],
            Value::from("v")
        ));
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow],
            Value::record([(Label::from("x"), Value::from("v"))])
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow, key("x")]),
            Some(&Value::from("v"))
        );

        // An inline record at the root writes on the root spine — no
        // cell involved.
        doc.root = Some(Value::record([(Label::from("shape"), Value::from(cell))]));
        assert!(set_value(
            &mut doc,
            &lib,
            &[key("title")],
            Value::from("scene")
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[key("title")]),
            Some(&Value::from("scene"))
        );
        assert!(set_value(&mut doc, &lib, &[], Value::from("root")));
        assert_eq!(doc.root, Some(Value::from("root")));
        // A parent that is not a record declines.
        assert!(!set_value(&mut doc, &lib, &[key("x")], Value::from("0")));
    }

    #[test]
    fn external_cells_decline_writes_and_bare_cells_accept() {
        let mut lib = Cells::new();
        let lib_cell = new_cell_id();
        lib.set_name(lib_cell, "convention");
        lib.set_value(lib_cell, Value::record([(Label::from("a"), Value::from("1"))]));
        let mut doc = Document {
            root: Some(Value::from(lib_cell)),
            cells: Cells::new(),
        };
        // The library's cell declines writes wholesale — fields,
        // value, and name alike.
        assert!(!set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("a")],
            Value::from("2")
        ));
        assert!(!set_name(&mut doc, &lib, &[Step::Name], "mine"));
        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow, key("a")]));
        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow]));
        // Forking — the document taking the cell over — writes.
        doc.cells
            .set_value(lib_cell, Value::record([(Label::from("a"), Value::from("1"))]));
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("a")],
            Value::from("2")
        ));
    }

    #[test]
    fn write_through_opens_one_step_per_editor_life() {
        let lib = Cells::new();
        let (mut doc, _) = doc_of(vec![(Label::from("name"), Value::from("a"))]);
        let path = vec![Step::Follow, key("name")];
        let mut selection = Selection::edge(&src(&doc, &lib), path);

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
        let mut fresh =
            Selection::edge(&src(&doc, &lib), vec![Step::Follow, key("name")]);
        fresh.edit_mut().unwrap().set_text("x");
        assert!(write_through(&mut doc, &lib, &mut fresh));
    }

    #[test]
    fn delete_unlinks_fields_and_elements_and_bares_cells() {
        let lib = Cells::new();
        let child = new_cell_id();
        let (mut doc, cell) = doc_of(vec![
            (Label::from("child"), Value::from(child)),
            (
                Label::from("dash"),
                Value::list([Value::from("2"), Value::from("3")]),
            ),
        ]);
        doc.cells.set_name(child, "c");

        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow, key("missing")]));

        // Unlinking a field drops the link; the linked cell floats in
        // the table for the orphan pool.
        assert!(delete_edge(&mut doc, &lib, &[Step::Follow, key("child")]));
        assert_eq!(src(&doc, &lib).resolve(&[Step::Follow, key("child")]), None);
        assert!(doc.cells.entry(child).is_some());

        // An element step rebuilds the list without it.
        let dash = vec![Step::Follow, key("dash")];
        let ps = positions(src(&doc, &lib).resolve(&dash).unwrap());
        assert!(delete_edge(
            &mut doc,
            &lib,
            &[Step::Follow, key("dash"), Step::Element(ps[0].clone())]
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&dash),
            Some(&Value::list([Value::from("3")]))
        );

        // A trailing Follow removes the cell's value: valueless
        // again, and a second delete declines.
        assert!(delete_edge(&mut doc, &lib, &[Step::Follow]));
        assert!(doc.cells.value(cell).is_none());
        assert!(src(&doc, &lib).resolve(&[]).is_some());
        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow]));

        // The empty path empties the document.
        assert!(delete_edge(&mut doc, &lib, &[]));
        assert!(doc.root.is_none());
        assert!(!delete_edge(&mut doc, &lib, &[]));
    }

    #[test]
    fn pendings_normalize_through_links_and_gate_on_authority() {
        let mut lib = Cells::new();
        let lib_cell = new_cell_id();
        lib.set_value(lib_cell, Value::record([(Label::from("a"), Value::from("1"))]));
        let bare = new_cell_id();
        let (mut doc, _) = doc_of(vec![
            (Label::from("at"), Value::record([])),
            (Label::from("tags"), Value::list([Value::from("x")])),
            (Label::from("lib"), Value::from(lib_cell)),
            (Label::from("material"), Value::from(bare)),
            (Label::from("s"), Value::from("leaf")),
        ]);
        doc.root = doc.root.clone();
        let sources = src(&doc, &lib);

        // A link to a record cell pends its field under Follow; an
        // inline record pends at its own path.
        let on_cell = pending_edge(&sources, vec![]).unwrap();
        assert_eq!(on_cell.path(), &[Step::Follow]);
        let inline = pending_edge(&sources, vec![Step::Follow, key("at")]).unwrap();
        assert_eq!(inline.path(), &[Step::Follow, key("at")]);

        // Lists, leaf atoms, external cells, and bare cells decline
        // fields.
        assert!(pending_edge(&sources, vec![Step::Follow, key("tags")]).is_none());
        assert!(pending_edge(&sources, vec![Step::Follow, key("s")]).is_none());
        assert!(pending_edge(&sources, vec![Step::Follow, key("lib")]).is_none());
        assert!(pending_edge(&sources, vec![Step::Follow, key("material")]).is_none());

        // A bare cell pends its first value at Follow — the
        // within-gesture's meaning there.
        let filling = pending_follow(&sources, &[Step::Follow, key("material")]).unwrap();
        assert_eq!(
            filling.path(),
            &[Step::Follow, key("material"), Step::Follow]
        );
        assert!(pending_follow(&sources, &[Step::Follow, key("lib")]).is_none());
        assert!(pending_follow(&sources, &[Step::Follow, key("at")]).is_none());

        // Into a list through its link, appended at the end.
        let into = pending_into(&sources, &[Step::Follow, key("tags")]).unwrap();
        assert!(matches!(
            into.path().last(),
            Some(Step::Element(_))
        ));
        assert_eq!(into.path().len(), 3);

        // The within chord: fields on records, elements into lists,
        // first values into bare cells.
        assert!(pending_insert(&sources, &[], false).is_some());
        assert!(pending_insert(&sources, &[Step::Follow, key("tags")], false).is_some());
        assert!(pending_insert(&sources, &[Step::Follow, key("material")], false).is_some());
        assert!(pending_insert(&sources, &[Step::Follow, key("s")], false).is_none());
    }

    #[test]
    fn queries_resolve_strings_and_blobs() {
        assert_eq!(resolve_query("hello"), Value::from("hello"));
        assert_eq!(resolve_query("\"quoted\""), Value::from("quoted"));
        assert_eq!(resolve_query("\"open"), Value::from("open"));
        assert_eq!(resolve_query("\"0xff\""), Value::from("0xff"));
        assert_eq!(resolve_query("0xff00"), Value::from(vec![0xff, 0x00]));
        // Input is case-tolerant — the value is the bytes, lowercase
        // just the canonical spelling — and whole bytes only.
        assert_eq!(resolve_query("0xDEad"), Value::from(vec![0xde, 0xad]));
        assert_eq!(resolve_query("0xf"), Value::from("0xf"));
        assert_eq!(resolve_query("0x"), Value::from(vec![]));
    }

    #[test]
    fn clipboard_spellings_round_trip() {
        let cell = new_cell_id();
        let cases = [
            Value::from("plain"),
            Value::from("\"tricky\""),
            Value::from(vec![0xde, 0xad]),
            Value::from(cell),
            Value::list([Value::from("a"), Value::from(cell)]),
            Value::record([(Label::from("x"), Value::from("1"))]),
            Value::record([(Label::Cell(cell), Value::from(vec![0x00_u8]))]),
        ];
        for value in cases {
            assert_eq!(from_clipboard(&to_clipboard(&value)), value);
        }
        // Atoms read in other apps; alien text pastes sensibly.
        assert_eq!(to_clipboard(&Value::from("hi")), "\"hi\"");
        assert_eq!(to_clipboard(&Value::from(vec![0xff_u8])), "0xff");
        assert_eq!(from_clipboard("loose text"), Value::from("loose text"));
    }

    #[test]
    fn completion_offers_follow_the_stage() {
        let lib = crate::conventions::library();
        let (mut doc, cell) = doc_of(vec![(Label::from("kind"), Value::from("building"))]);
        doc.cells.set_name(cell, "roof");
        let sources = src(&doc, &lib);
        let names = Names::table();
        let displays = |labels: bool, query: &str| -> Vec<String> {
            completion_entries(&sources, &names, false, labels, query)
                .into_iter()
                .map(|entry| entry.display)
                .collect()
        };

        // The value stage offers the string, references, the value
        // constructors, and the mint.
        let value_stage = displays(false, "");
        assert!(value_stage.iter().any(|d| d == "roof"));
        assert!(value_stage.iter().any(|d| d == "new list"));
        assert!(value_stage.iter().any(|d| d == "new record"));
        assert!(value_stage.iter().any(|d| d == "new cell"));

        // The label stage narrows to what can label: no list, record,
        // or blob offers.
        let label_stage = displays(true, "");
        assert!(label_stage.iter().all(|d| d != "new list"));
        assert!(label_stage.iter().all(|d| d != "new record"));
        assert!(label_stage.iter().any(|d| d == "new cell"));
        let label_blob = completion_entries(&sources, &names, false, true, "0xff");
        assert!(matches!(
            &label_blob[0].action,
            EntryAction::Value(value) if value.as_str() == Some("0xff")
        ));

        // A blob query leads with the blob, its string form below.
        let value_blob = displays(false, "0xff");
        assert_eq!(value_blob[0], "0xff");
        assert_eq!(value_blob[1], "\"0xff\"");

        // Reference commits are links.
        let roof = completion_entries(&sources, &names, false, false, "roof");
        assert!(matches!(
            &roof[0].action,
            EntryAction::Value(value) if value.as_cell() == Some(cell)
        ));
    }

    #[test]
    fn minting_seeds_names_and_bare_cells() {
        let mut doc = Document {
            root: None,
            cells: Cells::new(),
        };
        // A named mint is the red link: a name and nothing else.
        let named = resolve_entry(
            &mut doc,
            &EntryAction::NewCell {
                name: Some("roof".to_string()),
            },
        );
        let named_cell = named.as_cell().unwrap();
        assert_eq!(doc.cells.name(named_cell), Some("roof"));
        assert!(doc.cells.value(named_cell).is_none());
        // An unnamed mint is fully bare: a link with nothing said at
        // all.
        let bare = resolve_entry(&mut doc, &EntryAction::NewCell { name: None });
        assert!(doc.cells.entry(bare.as_cell().unwrap()).is_none());
        // The value constructors commit pure values — nothing minted.
        assert_eq!(
            resolve_entry(&mut doc, &EntryAction::NewList),
            Value::list([])
        );
        assert_eq!(
            resolve_entry(&mut doc, &EntryAction::NewRecord),
            Value::record([])
        );
    }

    #[test]
    fn the_sample_document_shows_the_constructs() {
        let doc = sample_document();
        let lib = crate::conventions::library();
        let sources = src(&doc, &lib);
        // The root is an inline record of roles.
        assert!(doc.root.as_ref().unwrap().as_record().is_some());
        let roof = sources.resolve(&[key("shape")]).unwrap().as_cell().unwrap();
        assert_eq!(sources.name(roof), Some("roof"));
        // The material cell is referenced and fully bare.
        let material = sources
            .resolve(&[key("shape"), Step::Follow, key("material")])
            .unwrap()
            .as_cell()
            .unwrap();
        assert!(sources.entry(material).is_none());
        // The stroke cell is the named bare floater, referenced only
        // as a label.
        let stroke = sources
            .value(roof)
            .unwrap()
            .as_record()
            .unwrap()
            .keys()
            .find_map(Label::as_cell)
            .unwrap();
        assert_eq!(sources.name(stroke), Some("stroke"));
        assert!(sources.value(stroke).is_none());
        // The style cell is shared by the root and the roof.
        assert_eq!(
            sources.resolve(&[key("style")]),
            sources.resolve(&[key("shape"), Step::Follow, key("style")])
        );
        // Points hold inline records; the swatch is a blob.
        let points = sources
            .resolve(&[key("shape"), Step::Follow, key("points")])
            .unwrap();
        let origin = points.as_list().unwrap().values().next().unwrap().as_cell().unwrap();
        assert!(matches!(
            sources
                .value(origin)
                .and_then(|value| value.as_record())
                .and_then(|fields| fields.get(&Label::from("at"))),
            Some(Value::Record(_))
        ));
        assert!(sources
            .resolve(&[key("style"), Step::Follow, key("swatch")])
            .unwrap()
            .as_blob()
            .is_some());
        // Documents round trip, names included.
        let json = serde_json::to_string(&doc).unwrap();
        let loaded: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.root, doc.root);
        assert_eq!(loaded.cells.name(roof), Some("roof"));
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
    }

    #[test]
    fn selecting_an_empty_value_slot_pends() {
        let mut lib = Cells::new();
        let bare = new_cell_id();
        let mut doc = Document {
            root: Some(Value::from(bare)),
            cells: Cells::new(),
        };
        // A writable valueless cell's Follow slot is already
        // authoring: selecting it (the rendered placeholder) pends.
        assert!(matches!(
            Selection::edge(&src(&doc, &lib), vec![Step::Follow]),
            Selection::Pending { .. }
        ));
        // Valued, it selects normally.
        doc.cells.set_value(bare, Value::from("v"));
        assert!(matches!(
            Selection::edge(&src(&doc, &lib), vec![Step::Follow]),
            Selection::Edge { .. }
        ));
        // An EXTERNAL valueless cell declines the pend — a pending
        // that cannot commit is an affordance lie.
        let lib_bare = new_cell_id();
        lib.set_name(lib_bare, "convention");
        doc.root = Some(Value::from(lib_bare));
        assert!(matches!(
            Selection::edge(&src(&doc, &lib), vec![Step::Follow]),
            Selection::Edge { edit: None, .. }
        ));
        // The empty document's root is the same rule.
        let empty = Document {
            root: None,
            cells: Cells::new(),
        };
        assert!(matches!(
            Selection::edge(&src(&empty, &lib), vec![]),
            Selection::Pending { .. }
        ));
    }

    #[test]
    fn renaming_writes_and_deleting_unnames_at_the_name_step() {
        let lib = Cells::new();
        let (mut doc, cell) = doc_of(vec![(Label::from("x"), Value::from("1"))]);
        doc.cells.set_name(cell, "old");
        let path = vec![Step::Name];

        // The name edits like any string, through the same run shape.
        let mut selection = Selection::edge(&src(&doc, &lib), path.clone());
        selection.edit_mut().unwrap().set_text("new");
        assert!(write_through(&mut doc, &lib, &mut selection));
        assert_eq!(doc.cells.name(cell), Some("new"));
        assert!(!write_through(&mut doc, &lib, &mut selection));

        // Emptying the buffer un-names live — the empty string is
        // the canonical spelling of no name — and typing again
        // re-names.
        selection.edit_mut().unwrap().set_text("");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(doc.cells.name(cell), None);
        selection.edit_mut().unwrap().set_text("back");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(doc.cells.name(cell), Some("back"));

        // Names are not edges: delete declines — un-naming is the
        // empty write above; the value is untouched throughout.
        assert!(!delete_edge(&mut doc, &lib, &path));
        assert_eq!(doc.cells.name(cell), Some("back"));
        selection.edit_mut().unwrap().set_text("");
        write_through(&mut doc, &lib, &mut selection);
        assert!(doc.cells.value(cell).is_some());

        // An unnamed cell mounts an EMPTY name editor: idle it
        // writes nothing, typing names the cell.
        let mut naming = Selection::edge(&src(&doc, &lib), path.clone());
        assert_eq!(naming.edit().unwrap().text(), "");
        assert!(!write_through(&mut doc, &lib, &mut naming));
        assert_eq!(doc.cells.name(cell), None);
        naming.edit_mut().unwrap().set_text("fresh");
        assert!(write_through(&mut doc, &lib, &mut naming));
        assert_eq!(doc.cells.name(cell), Some("fresh"));
    }
}
