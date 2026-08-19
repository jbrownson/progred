//! Selection, collapse, and the writes they drive. Paths name
//! locations in a [`Document`]; this module owns what is selected
//! there and how authoring and mutation land.

use crate::annotations::{self, Annotations};
use crate::identity::short_id;
use crate::projection::Projection;
use crate::sources::Sources;
use crate::spine;
use gid::{CellId, Cells, Document, Path, Position, Step, Value, position};
use progred_libraries::{name, text};
use puri::edit::LineEditState;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

/// Tier-2 editing state beside the selection: the live line editor
/// (caret, anchor, IME preedit, drag — and the text in motion), plus
/// the write-through wiring only edge editors have.
pub(crate) struct Editor {
    pub(crate) line: LineEditState,
    update: Option<fn(&Value, &str) -> Option<Value>>,
    /// Whether this editor's write-through run has recorded its undo
    /// step: the run is the editor's lifetime, so the first write
    /// records and the rest coalesce by staying silent.
    recorded: bool,
}

/// What is selected, stored as data plus tier-2 editing state: the
/// payload is a GID value — stage, query, choice, replacing; what a
/// projection at the selected path receives — while the live editor
/// stays Rust beside it, its text writing through to the payload at
/// the same per-event point the document takes its writes. A pending
/// stage's query resolves to the value that commits; until then the
/// graph is untouched, and deselecting discards the pending entirely.
pub struct Selection {
    /// Where: the value's path for edge and pending stages, the
    /// parent record's for a label stage.
    path: Path,
    payload: Value,
    editor: Option<Editor>,
}

/// The payload's stage, decoded for matching. Junk stages read as a
/// plain edge — the malformed rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Edge,
    /// A nonexistent location's value being authored (the root and a
    /// bare cell's value included).
    Pending,
    /// A field label being authored on the record at the path — new,
    /// or, with `replacing`, an existing one re-opened: commit re-keys
    /// the field whole, its value carried — values write through,
    /// addresses stage (a label is a key in a shared map, so
    /// intermediate spellings must never land).
    Label,
}

impl Selection {
    /// Select the value at `path`; a compact text value brings a focused editor (the root included —
    /// its commits target the document's root field). Selecting an
    /// EMPTY VALUE SLOT is already authoring it — there is nothing
    /// there to select, only something to begin, so it pends
    /// immediately: the empty document's root, and a valueless
    /// writable cell's Follow slot (its rendered placeholder).
    pub fn edge<World>(sources: &Sources, projection: &Projection<World>, path: Path) -> Self {
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
            .then(|| {
                sources
                    .resolve(&path)
                    .and_then(|value| projection.line(value))
            })
            .flatten();
        Selection {
            path,
            payload: payload::edge(),
            editor: edit.map(line_editing),
        }
    }

    /// A click on an editable line: the projection already named the
    /// line, so the selection does not look the value up again.
    pub fn from_line(sources: &Sources, path: Path, line: &crate::render::LineEdit) -> Self {
        let editor = writable_at(sources, &path).then(|| line_editing(line.clone()));
        Selection {
            path,
            payload: payload::edge(),
            editor,
        }
    }

    pub fn path(&self) -> &[Step] {
        &self.path
    }

    /// The selection as data — what a projection at this path receives.
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    pub fn stage(&self) -> Stage {
        match payload::stage(&self.payload) {
            Some(stage) if stage == payload::vocabulary::PENDING => Stage::Pending,
            Some(stage) if stage == payload::vocabulary::LABEL => Stage::Label,
            _ => Stage::Edge,
        }
    }

    /// Which completion entry commits; clamped against the frame's
    /// recomputed entries at use.
    pub fn choice(&self) -> usize {
        payload::choice(&self.payload).unwrap_or(0)
    }

    pub fn set_choice(&mut self, choice: usize) {
        self.payload = payload::with_choice(&self.payload, choice);
    }

    pub fn replacing(&self) -> Option<CellId> {
        payload::replacing(&self.payload)
    }

    /// Whether the mounted editor's write-through run has recorded
    /// its undo step — deleting through the value is one gesture.
    pub(crate) fn recorded(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| editor.recorded)
    }

    pub fn edit(&self) -> Option<&LineEditState> {
        self.editor.as_ref().map(|editor| &editor.line)
    }

    pub fn edit_mut(&mut self) -> Option<&mut LineEditState> {
        self.editor.as_mut().map(|editor| &mut editor.line)
    }

    /// Reseed a pending's query — the test paths.
    #[cfg(test)]
    pub(crate) fn with_query(mut self, text: &str) -> Selection {
        if let Some(editor) = &mut self.editor {
            editor.line = line_edit(text);
        }
        self.sync_payload();
        self
    }

    /// The live editor, written through to the payload whole — the
    /// same discipline, and the same per-event point, as the document
    /// write below. The payload is canonical at event boundaries; the
    /// working copy is its decode between them.
    fn sync_payload(&mut self) {
        let Some(editor) = &self.editor else { return };
        let next = payload::with_editor(&self.payload, &editor.line, self.stage() != Stage::Edge);
        if next != self.payload {
            self.payload = next;
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

fn line_editing(line: crate::render::LineEdit) -> Editor {
    Editor {
        line: line_edit(&line.text),
        update: Some(line.update),
        recorded: false,
    }
}

/// The selection an arrow step lands on: the caret seeds the side the
/// travel direction exits from, so the next same-direction press
/// crosses projected text in one press. The end-seeded default already IS
/// the rightward case; a leftward landing seeds the START instead of
/// grinding back through every character.
pub fn selected_by_arrow<World>(
    sources: &Sources,
    projection: &Projection<World>,
    path: Path,
    event: &KeyboardEvent,
) -> Selection {
    let mut selection = Selection::edge(sources, projection, path);
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
    pending_with_query(path, "")
}

/// An edge with no editor mounted — the test paths' plain selection.
#[cfg(test)]
pub(crate) fn bare_edge(path: Path) -> Selection {
    Selection {
        path,
        payload: payload::edge(),
        editor: None,
    }
}

/// A value pending with a seeded query — the clipboard and test paths.
pub(crate) fn pending_with_query(path: Path, seed: &str) -> Selection {
    query_selection(path, payload::pending(seed, 0))
}

/// A pending selection from its payload: the working editor is the
/// payload's decode, so the value is the state and the caret defaults
/// to the end of the seed.
fn query_selection(path: Path, payload: Value) -> Selection {
    let line = payload::editor_line(&payload, payload::query(&payload).unwrap_or(""));
    Selection {
        path,
        payload,
        editor: Some(Editor {
            line,
            update: None,
            recorded: false,
        }),
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
    Some(query_selection(parent, payload::label("", 0, None)))
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
        .and_then(name::read)
        .map(str::to_owned)
        .unwrap_or_else(|| short_id(*key));
    Some(query_selection(
        parent.to_vec(),
        payload::label(&seed, 0, Some(*key)),
    ))
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
        Some(inner) => text::value(inner.strip_suffix('"').unwrap_or(inner)),
        None => parse_blob(trimmed)
            .map(Value::from)
            .unwrap_or_else(|| text::value(text)),
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
    match (text::read(value), value.as_blob()) {
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
pub fn toggle_collapse<World>(
    sources: &Sources,
    projection: &Projection<World>,
    annotations: &mut Annotations,
    path: &[Step],
) -> bool {
    match collapse_default(sources, projection, path) {
        Some(default) => {
            let next = !annotations::collapsed(annotations, path, default);
            annotations::set_collapsed(annotations, path, default, next);
            true
        }
        None => false,
    }
}

/// The directional twin: close or open the value at `path` — the fold
/// axis of keyboard navigation. Returns whether the state changed.
pub fn set_collapse<World>(
    sources: &Sources,
    projection: &Projection<World>,
    annotations: &mut Annotations,
    path: &[Step],
    closed: bool,
) -> bool {
    match collapse_default(sources, projection, path) {
        Some(default) if annotations::collapsed(annotations, path, default) != closed => {
            annotations::set_collapsed(annotations, path, default, closed);
            true
        }
        _ => false,
    }
}

/// The default collapse for the value at `path` — collapsed inside a
/// cycle, expanded otherwise — or `None` when there is nothing to
/// collapse.
fn collapse_default<World>(
    sources: &Sources,
    projection: &Projection<World>,
    path: &[Step],
) -> Option<bool> {
    sources
        .resolve(path)
        // Compact atom projections are leaves. Once another field
        // enriches either convention, the visible record is collapsible.
        .filter(|value| projection.line(value).is_none())
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

/// Writes the selection's editor text through to its location after
/// every handled event — the graph is the source of truth.
/// The projection that mounted the line supplies its update, and
/// valid intermediate values write every keystroke. Everything funnels
/// through [`set_value`], so an element edit rebuilds its list at
/// the owning cell and a location that no longer takes the write
/// drops it silently — the malformed-graph rule at the mutation
/// boundary. Returns whether this write OPENED an undo step: true
/// exactly on the first write of the mounted editor's life, so a
/// typing run is one step and history stays a dumb stack.
pub fn write_through(doc: &mut Document, library: &Cells, selection: &mut Selection) -> bool {
    selection.sync_payload();
    let Selection { path, editor, .. } = selection;
    let Some(editor) = editor else {
        return false;
    };
    let Some(update) = editor.update else {
        return false;
    };
    let text = editor.line.text().to_string();
    let wrote = {
        let (current, next) = {
            let sources = Sources {
                doc: &*doc,
                library,
            };
            let current = sources.resolve(path);
            let next = current.and_then(|current| update(current, &text));
            (current.cloned(), next)
        };
        match next {
            Some(next) => current.as_ref() != Some(&next) && set_value(doc, library, path, next),
            None => false,
        }
    };
    if wrote {
        let first = !editor.recorded;
        editor.recorded = true;
        return first;
    }
    false
}

/// Breaks the open edit run: the next write records a fresh undo
/// step. Called after a save, so a run never straddles the mark.
pub fn break_edit_run(selection: Option<&mut Selection>) {
    if let Some(editor) = selection.and_then(|selection| selection.editor.as_mut()) {
        editor.recorded = false;
    }
}

/// The selection as data: the payload projections receive when their
/// path is the selected one. Stage is a named cell; the query rides
/// the text convention and the choice the f64 convention. Editor
/// gesture internals (caret, anchor, preedit, drag) are tier-2 Rust
/// and never encode; the live editor text writes through to the
/// payload at the same per-event point the document does.
pub mod payload {
    use gid::{CellId, Value};
    use progred_libraries::{f64 as f64_convention, text};
    use puri::edit::LineEditState;
    use vello::kurbo::Point;

    pub mod vocabulary {
        use gid::CellId;

        pub const STAGE: CellId = CellId::from_u128(0x6a1fd3082b9c47e5f60d21a8c45e9b37);
        pub const QUERY: CellId = CellId::from_u128(0xc25e80f7d1934ab6270c8f5e13b6d4a9);
        pub const CHOICE: CellId = CellId::from_u128(0x48b7a92c05e1d6f3891a4d20e7c53f6b);
        pub const REPLACING: CellId = CellId::from_u128(0xe3906b5d78a2c4f10b358d96a1f42c7d);

        /// A value's edge is selected; editing state, if any, is tier-2.
        pub const EDGE: CellId = CellId::from_u128(0x2f74c8a1936e05bd4c17e2b98d60a5f4);
        /// A value pending: the query authors the value at the path.
        pub const PENDING: CellId = CellId::from_u128(0x91d5e60b3a8f27c4058b39f6d2c471ea);
        /// A label pending on the record at the path: a new field's
        /// label, or with REPLACING, an existing one re-opened.
        pub const LABEL: CellId = CellId::from_u128(0x7be29f4680d1c5a3f2496e07b85d13c2);

        /// Selection byte offsets; FOCUS may precede ANCHOR.
        pub const ANCHOR: CellId = CellId::from_u128(0x5d38a1c7f24e9b60d15c7a02e83f46b9);
        pub const FOCUS: CellId = CellId::from_u128(0xa906e35d21c84f7bc3d05e918b62fa47);
        /// The in-flight IME composition: a text value with optional
        /// START/END cursor fields overlaid.
        pub const PREEDIT: CellId = CellId::from_u128(0x1c84f0b6d97325ea40d6b18c53e29f74);
        pub const START: CellId = CellId::from_u128(0xf27b950e13a8d64c26f9e30a71d45b8c);
        pub const END: CellId = CellId::from_u128(0x60d3e94a852f17bd39c2a45f08e61d73);
        /// The in-progress drag-selection: window origin and click count.
        pub const DRAG: CellId = CellId::from_u128(0xb49c26e1075df3a8e5017d29c46b83f5);
        pub const X: CellId = CellId::from_u128(0x39e50d7ac1846f2b7a2384b06d95c1ef);
        pub const Y: CellId = CellId::from_u128(0x8e17b3f4692a05dc90e5f6c2374a18db);
        pub const COUNT: CellId = CellId::from_u128(0x4dab72e9508c31f6cb490271f8ea56d0);
    }

    pub fn edge() -> Value {
        Value::record([(vocabulary::STAGE, Value::Cell(vocabulary::EDGE))])
    }

    pub fn pending(query: &str, choice: usize) -> Value {
        Value::record([
            (vocabulary::STAGE, Value::Cell(vocabulary::PENDING)),
            (vocabulary::QUERY, text::value(query)),
            (vocabulary::CHOICE, f64_convention::value(choice as f64)),
        ])
    }

    pub fn label(query: &str, choice: usize, replacing: Option<CellId>) -> Value {
        let mut fields = vec![
            (vocabulary::STAGE, Value::Cell(vocabulary::LABEL)),
            (vocabulary::QUERY, text::value(query)),
            (vocabulary::CHOICE, f64_convention::value(choice as f64)),
        ];
        if let Some(replacing) = replacing {
            fields.push((vocabulary::REPLACING, Value::Cell(replacing)));
        }
        Value::record(fields)
    }

    pub fn stage(payload: &Value) -> Option<CellId> {
        payload.as_record()?.get(&vocabulary::STAGE)?.as_cell()
    }

    pub fn query(payload: &Value) -> Option<&str> {
        text::read(payload.as_record()?.get(&vocabulary::QUERY)?)
    }

    pub fn choice(payload: &Value) -> Option<usize> {
        let choice = f64_convention::read(payload.as_record()?.get(&vocabulary::CHOICE)?)?;
        (choice >= 0.0 && choice.fract() == 0.0).then_some(choice as usize)
    }

    pub fn replacing(payload: &Value) -> Option<CellId> {
        payload.as_record()?.get(&vocabulary::REPLACING)?.as_cell()
    }

    pub fn with_choice(payload: &Value, choice: usize) -> Value {
        with_field(payload, vocabulary::CHOICE, f64_convention::value(choice as f64))
    }

    fn with_field(payload: &Value, key: CellId, value: Value) -> Value {
        let fields = payload.as_record().cloned().unwrap_or_default();
        Value::Record(fields.update(key, value))
    }

    /// Encode the whole live editor into the payload: the query text
    /// (pending stages own their text; an edge's text lives in the
    /// document), the selection offsets, and any in-flight IME
    /// composition or drag.
    pub fn with_editor(payload: &Value, line: &LineEditState, own_text: bool) -> Value {
        let mut fields = payload.as_record().cloned().unwrap_or_default();
        if own_text {
            fields.insert(vocabulary::QUERY, text::value(line.text()));
        }
        let (anchor, focus) = line.selection_offsets();
        fields.insert(vocabulary::ANCHOR, f64_convention::value(anchor as f64));
        fields.insert(vocabulary::FOCUS, f64_convention::value(focus as f64));
        match line.preedit_parts() {
            Some((preedit, cursor)) => {
                let mut composed = text::value(preedit).as_record().cloned().unwrap_or_default();
                if let Some((start, end)) = cursor {
                    composed.insert(vocabulary::START, f64_convention::value(start as f64));
                    composed.insert(vocabulary::END, f64_convention::value(end as f64));
                }
                fields.insert(vocabulary::PREEDIT, Value::Record(composed));
            }
            None => {
                fields.remove(&vocabulary::PREEDIT);
            }
        }
        match line.drag_parts() {
            Some((origin, count)) => {
                fields.insert(
                    vocabulary::DRAG,
                    Value::record([
                        (vocabulary::X, f64_convention::value(origin.x)),
                        (vocabulary::Y, f64_convention::value(origin.y)),
                        (vocabulary::COUNT, f64_convention::value(f64::from(count))),
                    ]),
                );
            }
            None => {
                fields.remove(&vocabulary::DRAG);
            }
        }
        Value::Record(fields)
    }

    /// Decode the live editor from the payload over `text` — the
    /// query for pending stages, the document's line for an edge.
    /// Absent offsets land the caret at the end (the mount default);
    /// junk clamps, per [`LineEditState::from_parts`].
    pub fn editor_line(payload: &Value, text: &str) -> LineEditState {
        let field_index = |key: CellId| {
            let index = f64_convention::read(payload.as_record()?.get(&key)?)?;
            (index >= 0.0 && index.fract() == 0.0).then_some(index as usize)
        };
        let anchor = field_index(vocabulary::ANCHOR).unwrap_or(text.len());
        let focus = field_index(vocabulary::FOCUS).unwrap_or(text.len());
        let preedit = payload
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::PREEDIT))
            .and_then(|composed| {
                let cursor = composed.as_record().and_then(|fields| {
                    let index = |key: CellId| {
                        let index = f64_convention::read(fields.get(&key)?)?;
                        (index >= 0.0 && index.fract() == 0.0).then_some(index as usize)
                    };
                    Some((index(vocabulary::START)?, index(vocabulary::END)?))
                });
                Some((text::read(composed)?.to_string(), cursor))
            });
        let drag = payload
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::DRAG))
            .and_then(Value::as_record)
            .and_then(|drag| {
                let coordinate = |key: CellId| f64_convention::read(drag.get(&key)?);
                Some((
                    Point::new(coordinate(vocabulary::X)?, coordinate(vocabulary::Y)?),
                    coordinate(vocabulary::COUNT)? as u8,
                ))
            });
        LineEditState::from_parts(text, anchor, focus, preedit, drag)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn payloads_round_trip_by_stage() {
            let edge = edge();
            assert_eq!(stage(&edge), Some(vocabulary::EDGE));
            assert_eq!(query(&edge), None);

            let pending = pending("asd", 2);
            assert_eq!(stage(&pending), Some(vocabulary::PENDING));
            assert_eq!(query(&pending), Some("asd"));
            assert_eq!(choice(&pending), Some(2));
            assert_eq!(replacing(&pending), None);

            let key = gid::new_cell_id();
            let rename = label("nm", 0, Some(key));
            assert_eq!(stage(&rename), Some(vocabulary::LABEL));
            assert_eq!(replacing(&rename), Some(key));
            assert_eq!(label("nm", 0, None).as_record().unwrap().len(), 3);
        }

        #[test]
        fn the_whole_editor_round_trips_through_the_payload() {
            let mut line = LineEditState::from_parts(
                "hëllo",
                2,
                4,
                Some(("ab".to_string(), Some((0, 2)))),
                Some((Point::new(4.5, 6.0), 2)),
            );
            // 2 is inside ë (bytes 1..3): clamps back to its boundary.
            assert_eq!(line.selection_offsets(), (1, 4));
            let encoded = with_editor(&pending("", 0), &line, true);
            let decoded = editor_line(&encoded, query(&encoded).unwrap_or(""));
            assert_eq!(decoded.text(), "hëllo");
            assert_eq!(decoded.selection_offsets(), line.selection_offsets());
            assert_eq!(decoded.preedit_parts(), line.preedit_parts());
            assert_eq!(decoded.drag_parts(), line.drag_parts());

            // Ending the composition and drag removes their fields.
            line = LineEditState::from_parts("hëllo", 1, 4, None, None);
            let settled = with_editor(&encoded, &line, true);
            let fields = settled.as_record().unwrap();
            assert!(!fields.contains_key(&vocabulary::PREEDIT));
            assert!(!fields.contains_key(&vocabulary::DRAG));
        }

        #[test]
        fn junk_reads_none() {
            assert_eq!(stage(&Value::record([])), None);
            let junk = Value::record([(
                vocabulary::CHOICE,
                super::f64_convention::value(-1.5),
            )]);
            assert_eq!(choice(&junk), None);
        }
    }
}
