//! Selection, collapse, and the writes they drive. Paths name
//! locations in a [`Document`]; this module owns what is selected
//! there and how authoring and mutation land.

use crate::annotations::{self, Annotations};
use crate::sources::Sources;
use crate::spine;
use crate::workspace;
use gid::{CellId, Document, Path, Position, Resolution, Step, Value, position};
use progred_libraries::{Libraries, absent, f64 as f64_convention, text};
use puri::edit::LineEditState;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

/// Tier-2 editing state beside the selection: the live line editor
/// (caret, anchor, IME preedit, drag — and the text in motion), plus
/// the write-through wiring only edge editors have.
pub(crate) struct Editor {
    pub(crate) line: LineEditState,
    /// The write-back rule as data: a grap callable, evaluated with
    /// the stack's foreign functions at the write-through point.
    update: Option<Value>,
    /// Whether this editor's write-through run has recorded its undo
    /// step: the run is the editor's lifetime, so the first write
    /// records and the rest coalesce by staying silent.
    recorded: bool,
}

/// One selection with caller-owned editor state. The live editor owns
/// text, caret, IME, and drag; its GID description is derived on demand.
/// Other projection state stays in the payload. QUERY there identifies
/// the completion view's query, not the editor's current text.
pub struct Selection {
    /// Which transient projection root owns this occurrence. Paths
    /// may coincide across panes, but selection never does.
    root: workspace::Root,
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
    /// A new field label being authored on the record at the path.
    /// Values write through, addresses stage: a label is a key in a
    /// shared map, so intermediate spellings must never land.
    Label,
}

impl Selection {
    /// Reify a payload at a host-owned path. This is the mutation
    /// boundary used by Grap capabilities after decoding the path.
    /// Editor fields are decoded once into
    /// their live owner, then removed from the stored payload.
    pub(crate) fn from_payload(
        root: &workspace::Root,
        sources: &Sources,
        path: Path,
        payload: Value,
    ) -> Self {
        match payload::stage(&payload) {
            Some(stage) if stage == payload::vocabulary::PENDING => {
                query_selection(root, path, payload)
            }
            Some(stage) if stage == payload::vocabulary::LABEL => {
                query_selection(root, path, payload)
            }
            _ => {
                let editor = payload
                    .as_record()
                    .and_then(|fields| fields.get(&payload::vocabulary::UPDATE))
                    .cloned()
                    .filter(|_| writable_at(sources, &path))
                    .and_then(|update| {
                        let fallback = payload::editor_text(&payload)?.to_owned();
                        Some(Editor {
                            line: payload::editor_line(&payload, &fallback),
                            update: Some(update),
                            recorded: false,
                        })
                    });
                Self {
                    root: root.clone(),
                    path,
                    payload: if editor.is_some() {
                        payload::without_editor(&payload)
                    } else {
                        payload
                    },
                    editor,
                }
            }
        }
    }

    /// Select the structural edge at `path`. Projected controls may
    /// install a richer transition on their navigation landmark; the
    /// stock line control uses that to mount its exact editor. An
    /// EMPTY VALUE SLOT is already authoring it — there is nothing
    /// there to select, only something to begin, so it pends
    /// immediately: the empty document's root, and a valueless
    /// writable cell's Follow slot (its rendered placeholder).
    pub fn edge(root: &workspace::Root, sources: &Sources, path: Path) -> Self {
        let empty_slot = match path.split_last() {
            None => sources.root().is_none(),
            Some((Step::Follow(resolution), parent)) => sources
                .resolve_path(parent)
                .and_then(Value::as_cell)
                .is_some_and(|cell| {
                    sources.value(cell, resolution).is_none() && sources.writable(cell, resolution)
                }),
            _ => false,
        };
        if empty_slot {
            return pending_value(root, path);
        }
        edge_selection(root, path, None)
    }

    /// Mount a line control already chosen by the current frame. This
    /// keeps pointer selection and caret placement on the exact
    /// projection the user clicked, without re-projecting after the
    /// selection transition.
    pub(crate) fn from_line(
        root: &workspace::Root,
        sources: &Sources,
        path: Path,
        line: progred_display::LineEdit,
    ) -> Self {
        let editor = writable_at(sources, &path).then(|| line_editing(line));
        edge_selection(root, path, editor)
    }

    pub fn path(&self) -> &[Step] {
        &self.path
    }

    pub fn root(&self) -> &workspace::Root {
        &self.root
    }

    pub(crate) fn relocate(&mut self, root: workspace::Root, path: Path) {
        self.root = root;
        self.path = path;
        self.preserve_recorded(false);
    }

    /// The selection as data — what a projection at this path receives.
    pub fn payload(&self) -> Value {
        match &self.editor {
            Some(editor) => {
                let payload =
                    payload::with_editor(&self.payload, &editor.line, self.stage() != Stage::Edge);
                match &editor.update {
                    Some(update) => payload::with_update(&payload, update),
                    None => payload,
                }
            }
            None => self.payload.clone(),
        }
    }

    pub fn stage(&self) -> Stage {
        match payload::stage(&self.payload) {
            Some(stage) if stage == payload::vocabulary::PENDING => Stage::Pending,
            Some(stage) if stage == payload::vocabulary::LABEL => Stage::Label,
            _ => Stage::Edge,
        }
    }

    /// The chosen completion row, including the expansion affordance;
    /// clamped against the current card at use.
    pub fn choice(&self) -> usize {
        if self.query_changed() {
            0
        } else {
            payload::choice(&self.payload).unwrap_or(0)
        }
    }

    pub fn completion_scroll(&self) -> f64 {
        if self.query_changed() {
            0.0
        } else {
            payload::completion_scroll(&self.payload).unwrap_or(0.0)
        }
    }

    pub fn completion_everything(&self) -> bool {
        !self.query_changed() && payload::completion_everything(&self.payload)
    }

    pub fn set_completion_view(&mut self, scroll: f64, choice: usize, everything: bool) {
        self.reset_completion_for_query();
        self.payload = payload::with_completion_view(&self.payload, scroll, choice, everything);
    }

    /// Whether the mounted editor's write-through run has recorded
    /// its undo step — deleting through the value is one gesture.
    pub(crate) fn recorded(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| editor.recorded)
    }

    /// Carry an open write-through run across a data-driven selection
    /// payload replacement at the same path.
    pub(crate) fn preserve_recorded(&mut self, recorded: bool) {
        if let Some(editor) = &mut self.editor {
            editor.recorded = recorded;
        }
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
        self.reset_completion_for_query();
        self
    }

    fn query_changed(&self) -> bool {
        self.stage() != Stage::Edge
            && self
                .editor
                .as_ref()
                .is_some_and(|editor| payload::query(&self.payload) != Some(editor.line.text()))
    }

    fn reset_completion_for_query(&mut self) {
        if self.query_changed()
            && let Some(editor) = &self.editor
        {
            self.payload = payload::with_completion_query(&self.payload, editor.line.text());
        }
    }
}

fn edge_selection(root: &workspace::Root, path: Path, editor: Option<Editor>) -> Selection {
    Selection {
        root: root.clone(),
        path,
        payload: payload::edge(),
        editor,
    }
}

// Seeded with the caret at the end: an editor mounted without a
// pointer — notably a keyboard landing — starts appending (a
// select-all trial read as dangerous), and a mounting click's caret
// placement overrides it (`select`). A leftward keyboard
// landing reseeds the start through `seed_from_arrow`.
pub(crate) fn line_edit(text: &str) -> LineEditState {
    LineEditState::new(text).with_cursor_at_end()
}

fn line_editing(line: progred_display::LineEdit) -> Editor {
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
pub fn seed_from_arrow(selection: &mut Selection, event: &KeyboardEvent) {
    if matches!(&event.key, Key::Named(NamedKey::ArrowLeft))
        && let Some(line) = selection.edit_mut()
    {
        line.cursor_to_start();
    }
}

/// The index of the path's last Follow step: the identity crossing
/// every write below it lands through. The link before it names the
/// owning cell; everything after it is a value spine.
pub(crate) fn last_follow(path: &[Step]) -> Option<usize> {
    path.iter()
        .rposition(|step| matches!(step, Step::Follow(_)))
}

/// Whether a write at `path` can land: the owning cell — the one the
/// path's last Follow crosses into — must not be external. A path
/// with no Follow is the document's own root spine and always
/// writable.
pub(crate) fn writable_at(sources: &Sources, path: &[Step]) -> bool {
    match last_follow(path) {
        Some(index) => match path[index] {
            Step::Follow(resolution) => sources
                .resolve_path(&path[..index])
                .and_then(Value::as_cell)
                .is_some_and(|cell| sources.writable(cell, &resolution)),
            Step::Key(_) | Step::Element(_) => false,
        },
        None => true,
    }
}

/// Deletes the value at `path`. A field or element step rebuilds the
/// owning value without it — unlinking; anything the dropped value
/// linked stays in the table for the orphan pool. A trailing Follow
/// removes the cell's own value: bare again, the symmetric partner
/// of authoring a value into one. The empty path empties the
/// document's root; paths that no longer resolve decline.
pub fn delete_edge(doc: &mut Document, libraries: &Libraries, path: &[Step]) -> bool {
    match path.split_last() {
        None => doc.root.take().is_some(),
        Some((Step::Follow(resolution), parent)) => {
            let cell = {
                let sources = Sources {
                    doc: &*doc,
                    libraries,
                };
                sources
                    .resolve_path(parent)
                    .and_then(Value::as_cell)
                    .filter(|cell| sources.writable(*cell, resolution))
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
                    libraries,
                };
                match last_follow(path) {
                    Some(index) => match path[index] {
                        Step::Follow(resolution) => sources
                            .resolve_path(&path[..index])
                            .and_then(Value::as_cell)
                            .filter(|cell| sources.writable(*cell, &resolution))
                            .and_then(|cell| {
                                spine::without(
                                    sources.value(cell, &resolution)?,
                                    &path[index + 1..],
                                )
                                .map(|rebuilt| (Some(cell), rebuilt))
                            }),
                        Step::Key(_) | Step::Element(_) => None,
                    },
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
pub fn pending_value(root: &workspace::Root, path: Path) -> Selection {
    pending_with_query(root, path, "")
}

/// An edge with no editor mounted — the test paths' plain selection.
#[cfg(test)]
pub(crate) fn bare_edge(root: &workspace::Root, path: Path) -> Selection {
    Selection {
        root: root.clone(),
        path,
        payload: payload::edge(),
        editor: None,
    }
}

/// A value pending with a seeded query — the clipboard and test paths.
pub(crate) fn pending_with_query(root: &workspace::Root, path: Path, seed: &str) -> Selection {
    query_selection(root, path, payload::pending(seed, 0))
}

/// Decode an incoming pending selection. The caret defaults to the
/// end of the query when no offset was supplied.
fn query_selection(root: &workspace::Root, path: Path, payload: Value) -> Selection {
    let line = payload::editor_line(&payload, payload::query(&payload).unwrap_or(""));
    Selection {
        root: root.clone(),
        path,
        payload: payload::without_editor(&payload),
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
/// value would introduce a new document definition. A document that
/// already owns the traversed path authors freely.
fn sole_value<'a>(sources: &Sources<'a>, cell: CellId) -> Option<crate::sources::LocatedValue<'a>> {
    let mut values = sources.values(cell);
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

pub fn pending_edge(root: &workspace::Root, sources: &Sources, parent: Path) -> Option<Selection> {
    let value = sources.resolve_path(&parent)?;
    let parent = match value {
        Value::Record(_) => parent,
        Value::Cell(cell) => {
            let value = sole_value(sources, *cell)?;
            value.value.as_record()?;
            let mut followed = parent;
            followed.push(Step::Follow(value.source));
            followed
        }
        Value::Blob(_) | Value::List(_) => return None,
    };
    writable_at(sources, &parent).then_some(())?;
    Some(query_selection(root, parent, payload::label("", 0)))
}

/// A bare cell's value being authored: the within-gesture's meaning
/// on a referenced identity with no value yet.
pub fn pending_follow(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
) -> Option<Selection> {
    let cell = sources.resolve_path(path)?.as_cell()?;
    sources.values(cell).next().is_none().then_some(())?;
    sources
        .writable(cell, &Resolution::Document)
        .then_some(())?;
    let mut followed = path.to_vec();
    followed.push(Step::Follow(Resolution::Document));
    Some(pending_value(root, followed))
}

/// A pending sibling next to the element at `path` (which must sit at
/// an element step), minted between it and its neighbor. The list
/// projection's gesture.
fn pending_beside(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
    after: bool,
) -> Option<Selection> {
    let (step, parent_path) = path.split_last()?;
    let Step::Element(position) = step else {
        return None;
    };
    let elements = sources.resolve_path(parent_path)?.as_list()?;
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
    Some(pending_value(root, fresh_path))
}

pub fn pending_after(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
) -> Option<Selection> {
    pending_beside(root, sources, path, true)
}

pub fn pending_before(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
) -> Option<Selection> {
    pending_beside(root, sources, path, false)
}

/// A pending element inside the list at `path` — inline, or a link's
/// cell value, normalized through Follow — appended at the end or
/// prepended at the front. Only lists take elements, by type, and
/// the owning cell must be writable, as in [`pending_edge`].
fn pending_into_at(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
    end: bool,
) -> Option<Selection> {
    let value = sources.resolve_path(path)?;
    let (list_path, elements) = match value {
        Value::List(elements) => (path.to_vec(), elements),
        Value::Cell(cell) => {
            let value = sole_value(sources, *cell)?;
            let elements = value.value.as_list()?;
            let mut followed = path.to_vec();
            followed.push(Step::Follow(value.source));
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
    Some(pending_value(root, fresh_path))
}

/// Appends: "add to this list" goes at the end — the within chord's
/// meaning on a list, where fields don't exist.
pub fn pending_into(root: &workspace::Root, sources: &Sources, path: &[Step]) -> Option<Selection> {
    pending_into_at(root, sources, path, true)
}

pub fn pending_into_first(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
) -> Option<Selection> {
    pending_into_at(root, sources, path, false)
}

/// Plain Enter: a new peer BESIDE the selection — continue the
/// enumeration you are in. An element pends a sibling (before with
/// shift); a field value pends a new field on its parent; the root
/// has nothing beside it and falls within — a field on a record, an
/// appended element on a list.
pub fn pending_enter(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
    before: bool,
) -> Option<Selection> {
    let beside = if before {
        pending_before(root, sources, path)
    } else {
        pending_after(root, sources, path)
    };
    beside
        .or_else(|| {
            path.split_last()
                .and_then(|(_, parent)| pending_edge(root, sources, parent.to_vec()))
        })
        .or_else(|| pending_edge(root, sources, path.to_vec()))
        .or_else(|| pending_into(root, sources, path))
}

/// The command chord: author WITHIN the selection — a new field on
/// the selected record or cell, an element appended into a list, or
/// a bare cell's first value. With shift, the front instead —
/// prepend. Atoms other than links have no within and decline.
pub fn pending_insert(
    root: &workspace::Root,
    sources: &Sources,
    path: &[Step],
    front: bool,
) -> Option<Selection> {
    if front {
        pending_into_first(root, sources, path)
    } else {
        pending_edge(root, sources, path.to_vec())
            .or_else(|| pending_into(root, sources, path))
            .or_else(|| pending_follow(root, sources, path))
    }
}

/// A pending root for an empty document.
pub fn pending_root(root: &workspace::Root, sources: &Sources) -> Option<Selection> {
    sources
        .root()
        .is_none()
        .then(|| pending_value(root, Vec::new()))
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
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
pub fn from_structure(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

/// Writes `value` at `path` — the empty path writes the document
/// root. The single write every edit reduces to: the path's last
/// Follow names the owning, authority-gated cell; the steps below it
/// are a value spine, rebuilt around the new leaf through the lens.
/// A bare cell takes its first value through the empty spine.
pub fn set_value(doc: &mut Document, libraries: &Libraries, path: &[Step], value: Value) -> bool {
    let write = {
        let sources = Sources {
            doc: &*doc,
            libraries,
        };
        match last_follow(path) {
            Some(index) => match path[index] {
                Step::Follow(resolution) => sources
                    .resolve_path(&path[..index])
                    .and_then(Value::as_cell)
                    .filter(|cell| sources.writable(*cell, &resolution))
                    .and_then(|cell| {
                        spine::set(sources.value(cell, &resolution), &path[index + 1..], value)
                            .map(|rebuilt| (Some(cell), rebuilt))
                    }),
                Step::Key(_) | Step::Element(_) => None,
            },
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

/// Toggle the collapse override for the value at `path`. Declines
/// unless there is something to collapse — a cell with a value, or a
/// nonempty list or record.
pub fn toggle_collapse(sources: &Sources, annotations: &mut Annotations, path: &[Step]) -> bool {
    match collapse_default(sources, path) {
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
pub fn set_collapse(
    sources: &Sources,
    annotations: &mut Annotations,
    path: &[Step],
    closed: bool,
) -> bool {
    match collapse_default(sources, path) {
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
pub(crate) fn collapse_default(sources: &Sources, path: &[Step]) -> Option<bool> {
    let value = sources.resolve_path(path)?;
    let in_cycle = value.as_cell().is_some_and(|cell| {
        (0..path.len())
            .filter(|end| matches!(path[*end], Step::Follow(_)))
            .filter_map(|end| sources.resolve_path(&path[..end]).and_then(Value::as_cell))
            .any(|ancestor| ancestor == cell)
    });
    collapse_default_for_value(sources, value, in_cycle)
}

/// The collapse class of an already-resolved value. Projection has
/// the ancestor cells in hand as it walks, while editor commands
/// recover the same `in_cycle` answer from their one-off path.
pub(crate) fn collapse_default_for_value(
    sources: &Sources,
    value: &Value,
    in_cycle: bool,
) -> Option<bool> {
    Some(value)
        .filter(|value| text::read(value).is_none() && f64_convention::read(value).is_none())
        .filter(|value| match value {
            Value::Cell(cell) => sources.values(*cell).next().is_some(),
            Value::Blob(_) => false,
            Value::List(elements) => !elements.is_empty(),
            Value::Record(fields) => !fields.is_empty(),
        })
        .map(|_| in_cycle)
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
pub fn write_through(doc: &mut Document, libraries: &Libraries, selection: &mut Selection) -> bool {
    selection.reset_completion_for_query();
    let Selection { path, editor, .. } = selection;
    let Some(editor) = editor else {
        return false;
    };
    let Some(update) = editor.update.clone() else {
        return false;
    };
    let typed = editor.line.text().to_string();
    let wrote = {
        let (current, next) = {
            let sources = Sources {
                doc: &*doc,
                libraries,
            };
            let current = sources.resolve_path(path).cloned();
            let next = {
                let arguments =
                    std::iter::once((
                        progred_libraries::line_edit::vocabulary::INPUT,
                        text::value(&typed),
                    ))
                    .chain(current.clone().map(|current| {
                        (progred_libraries::line_edit::vocabulary::CURRENT, current)
                    }));
                // `apply`, not `call` + `evaluate`: the current value
                // is data even when it is code-shaped.
                let evaluation = grap::apply(
                    &update,
                    arguments,
                    |cell| sources.grap_definitions(cell),
                    grap::DEFAULT_FUEL,
                );
                (!absent::is_absent(&evaluation.result)).then_some(evaluation.result)
            };
            (current, next)
        };
        match next {
            Some(next) => current.as_ref() != Some(&next) && set_value(doc, libraries, path, next),
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
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
pub fn break_edit_run(selection: Option<&mut Selection>) {
    if let Some(editor) = selection.and_then(|selection| selection.editor.as_mut()) {
        editor.recorded = false;
    }
}

/// The selection as data: the payload projections receive when their
/// path is the selected one. Stage is a named cell; the query rides
/// the text convention and the choice the f64 convention. Editor
/// gesture internals (text in motion, caret, anchor, preedit, drag)
/// are encoded when requested and decoded when a capability supplies
/// a replacement. They are never mirrored in the stored payload.
pub mod payload {
    use gid::{CellId, Value};
    use kurbo::Point;
    use progred_libraries::{f64 as f64_convention, logic, text};
    use puri::edit::LineEditState;

    pub mod vocabulary {
        use gid::CellId;
        pub use progred_libraries::selection::vocabulary::{STAGE, EDGE, PENDING, LABEL};
        pub const QUERY: CellId = CellId::from_u128(0xc25e80f7d1934ab6270c8f5e13b6d4a9);
        pub const CHOICE: CellId = CellId::from_u128(0x48b7a92c05e1d6f3891a4d20e7c53f6b);
        pub const COMPLETION_SCROLL: CellId = CellId::from_u128(0x151767a413a8bc5f579465dd67f18263);
        pub const COMPLETION_EVERYTHING: CellId =
            CellId::from_u128(0xedfa139b72d468afe1d926e811477456);

        /// Selection byte offsets; FOCUS may precede ANCHOR.
        pub const ANCHOR: CellId = CellId::from_u128(0x5d38a1c7f24e9b60d15c7a02e83f46b9);
        pub const FOCUS: CellId = CellId::from_u128(0xa906e35d21c84f7bc3d05e918b62fa47);
        /// The in-flight IME composition: a text value with optional
        /// START/END cursor fields overlaid.
        pub const PREEDIT: CellId = CellId::from_u128(0x1c84f0b6d97325ea40d6b18c53e29f74);
        pub const START: CellId = CellId::from_u128(0xf27b950e13a8d64c26f9e30a71d45b8c);
        pub const END: CellId = CellId::from_u128(0x60d3e94a852f17bd39c2a45f08e61d73);
        /// The edge editor's write-back rule: a grap callable.
        pub const UPDATE: CellId = CellId::from_u128(0xcd06f18e4a72359bd6084c3f92e17ab4);
        /// The editor's in-motion text. For a pending this mirrors
        /// QUERY; for an edge it preserves text until write-through.
        pub const EDITOR_TEXT: CellId = CellId::from_u128(0x3b3544bd8a2fc3a83a08edb6766fad4a);
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

    pub fn label(query: &str, choice: usize) -> Value {
        Value::record([
            (vocabulary::STAGE, Value::Cell(vocabulary::LABEL)),
            (vocabulary::QUERY, text::value(query)),
            (vocabulary::CHOICE, f64_convention::value(choice as f64)),
        ])
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

    pub fn completion_scroll(payload: &Value) -> Option<f64> {
        f64_convention::read(payload.as_record()?.get(&vocabulary::COMPLETION_SCROLL)?)
            .filter(|scroll| scroll.is_finite() && *scroll >= 0.0)
    }

    pub fn completion_everything(payload: &Value) -> bool {
        payload
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::COMPLETION_EVERYTHING))
            .and_then(Value::as_cell)
            == Some(logic::vocabulary::TRUE)
    }

    pub fn with_update(payload: &Value, update: &Value) -> Value {
        with_field(payload, vocabulary::UPDATE, update.clone())
    }

    pub fn with_completion_query(payload: &Value, query: &str) -> Value {
        if self::query(payload) == Some(query) {
            payload.clone()
        } else {
            with_field(
                &with_completion_view(payload, 0.0, 0, false),
                vocabulary::QUERY,
                text::value(query),
            )
        }
    }

    pub fn with_completion_view(
        payload: &Value,
        scroll: f64,
        choice: usize,
        everything: bool,
    ) -> Value {
        let fields = payload.as_record().cloned().unwrap_or_default();
        Value::Record(
            fields
                .update(vocabulary::COMPLETION_SCROLL, f64_convention::value(scroll))
                .update(vocabulary::CHOICE, f64_convention::value(choice as f64))
                .update(vocabulary::COMPLETION_EVERYTHING, logic::value(everything)),
        )
    }

    pub fn without_editor(payload: &Value) -> Value {
        let mut fields = payload.as_record().cloned().unwrap_or_default();
        for field in [
            vocabulary::EDITOR_TEXT,
            vocabulary::ANCHOR,
            vocabulary::FOCUS,
            vocabulary::PREEDIT,
            vocabulary::DRAG,
            vocabulary::UPDATE,
        ] {
            fields.remove(&field);
        }
        Value::Record(fields)
    }

    fn with_field(payload: &Value, key: CellId, value: Value) -> Value {
        let fields = payload.as_record().cloned().unwrap_or_default();
        Value::Record(fields.update(key, value))
    }

    /// Encode the whole live editor into the payload: its in-motion
    /// text, the query text when the stage owns it, selection offsets,
    /// and any in-flight IME composition or drag.
    pub fn with_editor(payload: &Value, line: &LineEditState, own_text: bool) -> Value {
        let payload = if own_text {
            with_completion_query(payload, line.text())
        } else {
            payload.clone()
        };
        let mut fields = payload.as_record().cloned().unwrap_or_default();
        fields.insert(vocabulary::EDITOR_TEXT, text::value(line.text()));
        let (anchor, focus) = line.selection_offsets();
        fields.insert(vocabulary::ANCHOR, f64_convention::value(anchor as f64));
        fields.insert(vocabulary::FOCUS, f64_convention::value(focus as f64));
        match line.preedit_parts() {
            Some((preedit, cursor)) => {
                let mut composed = text::value(preedit)
                    .as_record()
                    .cloned()
                    .unwrap_or_default();
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

    pub fn editor_text(payload: &Value) -> Option<&str> {
        text::read(payload.as_record()?.get(&vocabulary::EDITOR_TEXT)?)
    }

    /// Decode the live editor from the payload over fallback `text`.
    /// An encoded in-motion spelling wins when present.
    /// Absent offsets land the caret at the end (the mount default);
    /// junk clamps, per [`LineEditState::from_parts`].
    pub fn editor_line(payload: &Value, text: &str) -> LineEditState {
        let text = editor_text(payload).unwrap_or(text);
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

            let label = label("nm", 0);
            assert_eq!(stage(&label), Some(vocabulary::LABEL));
            assert_eq!(label.as_record().unwrap().len(), 3);
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
        fn changing_an_owned_query_resets_its_completion_choice() {
            let changed = with_editor(
                &with_completion_view(&pending("old", 2), 24.0, 2, true),
                &LineEditState::from_parts("new", 3, 3, None, None),
                true,
            );
            assert_eq!(query(&changed), Some("new"));
            assert_eq!(choice(&changed), Some(0));
            assert_eq!(completion_scroll(&changed), Some(0.0));
            assert!(!completion_everything(&changed));

            let unchanged = with_editor(
                &with_completion_view(&pending("same", 2), 24.0, 2, true),
                &LineEditState::from_parts("same", 4, 4, None, None),
                true,
            );
            assert_eq!(choice(&unchanged), Some(2));
            assert_eq!(completion_scroll(&unchanged), Some(24.0));
            assert!(completion_everything(&unchanged));
        }

        #[test]
        fn junk_reads_none() {
            assert_eq!(stage(&Value::record([])), None);
            let junk = Value::record([(vocabulary::CHOICE, super::f64_convention::value(-1.5))]);
            assert_eq!(choice(&junk), None);
        }
    }
}
