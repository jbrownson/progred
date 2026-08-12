//! Selection, collapse, and the writes they drive. Paths name
//! locations in a [`Document`]; this module owns what is selected
//! there and how authoring and mutation land.

use crate::document::{Document, Path, short_id};
use crate::sources::Sources;
use crate::projection;
use progred_graph::{CellId, Cells, Position, Step, Value, position, spine};
use puri::edit::LineEditState;
use std::collections::HashMap;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

/// Per-path collapse overrides. An absent entry means "use the
/// default", which is collapsed inside a cycle and expanded otherwise;
/// a present entry forces it the other way. Sparse: only overrides are
/// stored.
#[derive(Default)]
pub struct Collapse {
    pub(crate) overrides: HashMap<Path, bool>,
}

impl Collapse {
    pub fn collapsed(&self, path: &[Step], in_cycle: bool) -> bool {
        self.overrides.get(path).copied().unwrap_or(in_cycle)
    }
}

pub(crate) struct ValueEditState {
    pub(crate) line: LineEditState,
    pub(crate) handler: crate::display::EditHandler,
}

/// What is selected: the value at a path, or a nonexistent field
/// being authored. A selected editable atom carries its live editor state —
/// projected text and f64 values are text editors focused by selection, and the
/// graph is written through as they edit. A pending selection carries the
/// completion query instead; the query resolves to the value that
/// commits, and until then the graph is untouched — deselecting
/// discards the pending entirely.
pub enum Selection {
    Edge {
        path: Path,
        edit: Option<ValueEditState>,
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
    /// selects the existing field if the label is taken). With
    /// `replacing`, an EXISTING field's label re-opened: commit
    /// re-keys the field whole, its value carried — values write
    /// through, addresses stage (a label is a key in a shared map,
    /// so intermediate spellings must never land).
    PendingEdge {
        parent: Path,
        query: LineEditState,
        choice: usize,
        replacing: Option<CellId>,
    },
}

impl Selection {
    /// Select the value at `path`; a compact text value brings a focused editor (the root included —
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
                .is_some_and(|cell| sources.value(cell).is_none() && sources.writable(cell)),
            _ => false,
        };
        if empty_slot {
            return pending_value(path);
        }
        // An editor mounts only where write-through can land: the
        // owning cell must not be external.
        let edit = writable_at(sources, &path)
            .then(|| sources.resolve(&path).and_then(projection::editor))
            .flatten();
        Selection::Edge {
            path,
            edit: edit.map(|editor| ValueEditState {
                line: line_edit(&editor.text),
                handler: editor.handler,
            }),
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
            Selection::Edge { edit, .. } => edit.as_ref().map(|edit| &edit.line),
            Selection::Pending { query, .. } | Selection::PendingEdge { query, .. } => Some(query),
        }
    }

    pub fn edit_mut(&mut self) -> Option<&mut LineEditState> {
        match self {
            Selection::Edge { edit, .. } => edit.as_mut().map(|edit| &mut edit.line),
            Selection::Pending { query, .. } | Selection::PendingEdge { query, .. } => Some(query),
        }
    }
}

// Seeded with the caret at the end: an editor mounted without a
// click — a keyboard landing, Cmd+L — starts appending (a select-all
// trial read as dangerous), and a mounting click's caret placement
// overrides it (`select`, `rename`). The one exception is a LEFTWARD
// keyboard landing, which seeds the start (`selected_by_arrow`).
pub(crate) fn line_edit(text: &str) -> LineEditState {
    LineEditState::new(text).with_cursor_at_end()
}

/// The selection an arrow step lands on: the caret seeds the side the
/// travel direction exits from, so the next same-direction press
/// crosses projected text in one press. The end-seeded default already IS
/// the rightward case; a leftward landing seeds the START instead of
/// grinding back through every character.
pub fn selected_by_arrow(sources: &Sources, path: Path, event: &KeyboardEvent) -> Selection {
    let mut selection = Selection::edge(sources, path);
    if matches!(&event.key, Key::Named(NamedKey::ArrowLeft))
        && let Some(edit) = selection.edit_mut()
    {
        edit.cursor_to_start();
    }
    selection
}

/// The index of the path's last Follow step: the identity crossing
/// every write below it lands through. The link before it names the
/// owning cell; everything after it is a value spine.
pub(crate) fn last_follow(path: &[Step]) -> Option<usize> {
    path.iter().rposition(|step| matches!(step, Step::Follow))
}

/// Whether a write at `path` can land: the owning cell — the one the
/// path's last Follow crosses into — must not be external. A path
/// with no Follow is the document's own root spine and always
/// writable.
pub(crate) fn writable_at(sources: &Sources, path: &[Step]) -> bool {
    match last_follow(path) {
        Some(index) => sources
            .resolve(&path[..index])
            .and_then(Value::as_cell)
            .is_some_and(|cell| sources.writable(cell)),
        None => true,
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
                let sources = Sources {
                    doc: &*doc,
                    library,
                };
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
        Some((Step::Key(_) | Step::Element(_), _)) => {
            let write = {
                let sources = Sources {
                    doc: &*doc,
                    library,
                };
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

/// A value-stage pending: the location named by `path` does not
/// exist, and its value is being authored.
pub fn pending_value(path: Path) -> Selection {
    Selection::Pending {
        path,
        query: line_edit(""),
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
        Value::Cell(cell) => {
            sources.value(*cell)?.as_record()?;
            let mut followed = parent;
            followed.push(Step::Follow);
            followed
        }
        Value::Blob(_) | Value::List(_) => return None,
    };
    writable_at(sources, &parent).then_some(())?;
    Some(Selection::PendingEdge {
        parent,
        query: line_edit(""),
        choice: 0,
        replacing: None,
    })
}

/// An existing field's label re-opened as a pending edge, the query
/// seeded with the cell label's current display name or short id.
/// Another cell may share the name; the seed is presentation, not
/// identity. Committing a taken label navigates to its field, the
/// new-field rule.
pub fn pending_rename(sources: &Sources, path: &[Step]) -> Option<Selection> {
    let (step, parent) = path.split_last()?;
    let Step::Key(key) = step else { return None };
    sources.resolve(path)?;
    writable_at(sources, parent).then_some(())?;
    let seed = sources
        .value(*key)
        .and_then(progred_name::read)
        .map(str::to_owned)
        .unwrap_or_else(|| short_id(*key));
    Some(Selection::PendingEdge {
        parent: parent.to_vec(),
        query: line_edit(&seed),
        choice: 0,
        replacing: Some(*key),
    })
}

/// A bare cell's value being authored: the within-gesture's meaning
/// on a referenced identity with no value yet.
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
        Value::Cell(cell) => {
            let elements = sources.value(*cell)?.as_list()?;
            let mut followed = path.to_vec();
            followed.push(Step::Follow);
            (followed, elements)
        }
        Value::Blob(_) | Value::Record(_) => return None,
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
pub(crate) fn parse_blob(text: &str) -> Option<Vec<u8>> {
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

/// The value a pending query resolves to: a leading quote forces
/// text (the closing quote optional, so text mode holds while
/// typing), `0x` hex reads as a blob, anything else is text as typed.
pub fn resolve_query(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.strip_prefix('"') {
        Some(inner) => progred_text::value(inner.strip_suffix('"').unwrap_or(inner)),
        None => parse_blob(trimmed)
            .map(Value::from)
            .unwrap_or_else(|| progred_text::value(text)),
    }
}

/// The clipboard spelling of a value, and whether it is STRUCTURE.
/// Values carry their own inline structure, and cell copies are
/// ALWAYS SHALLOW — a link is its identity alone, no cell values
/// travel: the value/cell boundary IS the copy boundary. Compact text
/// and blobs spell as the query language — quoted text and `0x` hex —
/// and are not structure: their text is their faithful form. Links,
/// lists, and records spell as Value JSON and ARE: the shell writes
/// that spelling under the private clipboard format too, whose
/// presence is what says "structure" — never the text's shape, so
/// text that happens to spell Value JSON stays text.
pub fn to_clipboard(value: &Value) -> (String, bool) {
    match (projection::whole_text(value), value.as_blob()) {
        (Some(text), _) => (format!("\"{text}\""), false),
        (_, Some(_)) => (value.to_string(), false),
        _ => (
            serde_json::to_string(value).expect("values serialize"),
            true,
        ),
    }
}

/// The value clipboard TEXT denotes — always the query reading:
/// quoted text, `0x` blobs, and bare text. Text is never structure;
/// structure rides the private format, read by [`from_structure`].
pub fn from_clipboard(text: &str) -> Value {
    resolve_query(text)
}

/// The value the private clipboard format's bytes denote.
pub fn from_structure(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

/// Writes `value` at `path` — the empty path writes the document
/// root. The single write every edit reduces to: the path's last
/// Follow names the owning, authority-gated cell; the steps below it
/// are a value spine, rebuilt around the new leaf through the lens.
/// A bare cell takes its first value through the empty spine.
pub fn set_value(doc: &mut Document, library: &Cells, path: &[Step], value: Value) -> bool {
    let write = {
        let sources = Sources {
            doc: &*doc,
            library,
        };
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

/// Re-keys the field `old` on the record at `parent` to `label`, the
/// value carried — one write through [`set_value`]. Declines when
/// the record or field is missing or the label is taken: a rename
/// never destroys a sibling (the caller navigates to it instead).
pub fn rename_field(
    doc: &mut Document,
    library: &Cells,
    parent: &[Step],
    old: &CellId,
    label: CellId,
) -> bool {
    let rekeyed = {
        let sources = Sources {
            doc: &*doc,
            library,
        };
        sources
            .resolve(parent)
            .and_then(Value::as_record)
            .and_then(|fields| {
                (!fields.contains_key(&label)).then_some(())?;
                let value = fields.get(old)?.clone();
                Some(Value::Record(fields.without(old).update(label, value)))
            })
    };
    match rekeyed {
        Some(record) => set_value(doc, library, parent, record),
        None => false,
    }
}

/// Toggle the collapse override for the value at `path`. Declines
/// unless there is something to collapse — a cell with a value, or a
/// nonempty list or record.
pub fn toggle_collapse(sources: &Sources, collapse: &mut Collapse, path: &[Step]) -> bool {
    match collapse_default(sources, path) {
        Some(default) => {
            let next = !collapse.collapsed(path, default);
            store_collapse(collapse, path, default, next);
            true
        }
        None => false,
    }
}

/// The directional twin: close or open the value at `path` — the fold
/// axis of keyboard navigation. Returns whether the state changed.
pub fn set_collapse(
    sources: &Sources,
    collapse: &mut Collapse,
    path: &[Step],
    closed: bool,
) -> bool {
    match collapse_default(sources, path) {
        Some(default) if collapse.collapsed(path, default) != closed => {
            store_collapse(collapse, path, default, closed);
            true
        }
        _ => false,
    }
}

/// The default collapse for the value at `path` — collapsed inside a
/// cycle, expanded otherwise — or `None` when there is nothing to
/// collapse.
fn collapse_default(sources: &Sources, path: &[Step]) -> Option<bool> {
    sources
        .resolve(path)
        // Compact atom projections are leaves. Once another field
        // enriches either convention, the visible record is collapsible.
        .filter(|value| !projection::editable(value))
        .filter(|value| match value {
            Value::Cell(cell) => sources.value(*cell).is_some(),
            Value::Blob(_) => false,
            Value::List(elements) => !elements.is_empty(),
            Value::Record(fields) => !fields.is_empty(),
        })
        .map(|value| {
            (0..path.len())
                .filter_map(|end| sources.resolve(&path[..end]))
                .any(|ancestor| ancestor == value)
        })
}

/// Stays sparse: an override matching the default is removed rather
/// than stored.
fn store_collapse(collapse: &mut Collapse, path: &[Step], default: bool, next: bool) {
    if next == default {
        collapse.overrides.remove(path);
    } else {
        collapse.overrides.insert(path.to_vec(), next);
    }
}

/// Writes the selection's editor text through to its location after
/// every handled event — the graph is the source of truth.
/// The projection that mounted the editor supplies its text-to-value
/// handler, and valid intermediate values write every keystroke. Everything funnels
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
    let text = edit.line.text().to_string();
    let wrote = {
        let (current, next) = {
            let sources = Sources {
                doc: &*doc,
                library,
            };
            let current = sources.resolve(path);
            let next = current.and_then(|current| edit.handler.apply(current, &text));
            (current.cloned(), next)
        };
        match next {
            Some(next) => current.as_ref() != Some(&next) && set_value(doc, library, path, next),
            None => false,
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
