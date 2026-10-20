//! Selection and collapse. Selection paths name projected occurrences; their
//! conject locates document data for mutations.

use crate::display::widget::view::Root;
use crate::editing::{Read, Scope};
use crate::location::Location;
use crate::sources::Sources;
use ::grap::text;
use gid::{Path, Position, Resolution, Step, Value, position};
use puri::edit::LineEditState;
use std::rc::Rc;

/// Editing state beside the selection: the live line editor
/// (caret, anchor, IME preedit, drag — and the text in motion), plus
/// whether that state is a query and its undo grouping.
pub(crate) struct Editor {
    pub(crate) line: LineEditState,
    query: bool,
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
    root: Root,
    /// Where: the value's path for edge and pending stages, the
    /// parent record's for a label stage.
    path: Path,
    scope: Scope,
    payload: Value,
    editor: Option<Editor>,
    /// Where a picker opened from a key was opened, for stepping back to.
    origin: Option<Path>,
}

/// The selection's role. Without an explicit query mode, its location
/// determines whether it selects a value or an empty slot.
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
    /// Reify a payload at a host-owned path, read through `scope`: the
    /// mutation boundary Grap capabilities use after decoding the path.
    /// Editor fields are decoded once into their live owner, then removed
    /// from the stored payload.
    pub(crate) fn from_payload(
        root: &Root,
        sources: &Sources,
        scope: Scope,
        path: Path,
        payload: Value,
    ) -> Self {
        let mut selection = match stage_of(&payload) {
            Stage::Pending | Stage::Label => query_selection(root, scope, path, payload),
            Stage::Edge => {
                let read = scope.view(*sources);
                let mounted = payload::editor_text(&payload)
                    .filter(|_| read.writable(&path))
                    .map(|text| mount(&payload, text, read.resolve_path(&path).is_none()));
                let (payload, editor) = match mounted {
                    Some((payload, editor)) => (payload, Some(editor)),
                    None => (payload, None),
                };
                Self {
                    root: root.clone(),
                    path,
                    scope,
                    payload,
                    editor,
                    origin: None,
                }
            }
        };
        selection.reset_completion_for_query();
        selection
    }

    pub(crate) fn edge(root: &Root, scope: Scope, path: Path) -> Self {
        Self {
            root: root.clone(),
            path,
            scope,
            payload: payload::edge(),
            editor: None,
            origin: None,
        }
    }

    pub fn path(&self) -> &[Step] {
        &self.path
    }

    pub(crate) fn scope(&self) -> &Scope {
        &self.scope
    }

    /// This selection selected again, read through `scope`: its line's
    /// caret moves to the end.
    pub(crate) fn reselected(mut self, scope: Scope) -> Self {
        if let Some(line) = self.edit_mut() {
            line.cursor_to_end();
        }
        self.scope = scope;
        self
    }

    pub(crate) fn origin(&self) -> Option<&[Step]> {
        self.origin.as_deref()
    }

    pub(crate) fn with_origin(mut self, origin: Path) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Where the selected data is stored, as this selection's scope names it.
    pub(crate) fn location(&self) -> Option<Location<'_>> {
        self.scope.location(&self.path)
    }

    pub(crate) fn value<'a>(&self, sources: &Sources<'a>) -> Option<&'a Value> {
        self.scope.view(*sources).resolve_path(&self.path)
    }

    pub(crate) fn writable(&self, sources: &Sources<'_>) -> bool {
        self.scope.view(*sources).writable(&self.path)
    }

    pub fn root(&self) -> &Root {
        &self.root
    }

    pub(crate) fn relocate(&mut self, root: Root, path: Path) {
        self.root = root;
        self.path = path;
        self.set_recorded(false);
    }

    /// The selection as data — what a projection at this path receives.
    pub fn payload(&self) -> Value {
        match &self.editor {
            Some(editor) => payload::with_editor(&self.payload, &editor.line, editor.query),
            None => self.payload.clone(),
        }
    }

    pub(crate) fn explicit_stage(&self) -> Stage {
        stage_of(&self.payload)
    }

    pub fn stage(&self, sources: &Sources) -> Stage {
        match self.explicit_stage() {
            Stage::Edge if self.value(sources).is_none() && self.writable(sources) => {
                Stage::Pending
            }
            stage => stage,
        }
    }

    pub(crate) fn history_path(&self) -> Option<&[Step]> {
        (self.explicit_stage() == Stage::Edge
            && self.editor.as_ref().is_none_or(|editor| !editor.query))
        .then_some(&self.path)
    }

    /// The chosen completion row, clamped against the current card at use.
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

    pub fn set_completion_view(&mut self, scroll: f64, choice: usize) {
        self.reset_completion_for_query();
        self.payload = payload::with_completion_view(&self.payload, scroll, choice);
    }

    /// Whether the mounted editor's write-through run has recorded
    /// its undo step — deleting through the value is one gesture.
    pub(crate) fn recorded(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| editor.recorded)
    }

    /// Sets whether the mounted editor's write-through run has recorded
    /// its undo step: its first write does, a payload replacement at the
    /// same place carries the run over, and anything that ends it clears it.
    pub(crate) fn set_recorded(&mut self, recorded: bool) {
        if let Some(editor) = &mut self.editor {
            editor.recorded = recorded;
        }
    }

    pub fn edit(&self) -> Option<&LineEditState> {
        self.editor.as_ref().map(|editor| &editor.line)
    }

    pub(crate) fn value_edit(&self) -> Option<&LineEditState> {
        self.editor
            .as_ref()
            .filter(|editor| !editor.query)
            .map(|editor| &editor.line)
    }

    pub fn edit_mut(&mut self) -> Option<&mut LineEditState> {
        self.editor.as_mut().map(|editor| &mut editor.line)
    }

    pub(crate) fn clear_editor(&mut self) {
        self.editor = None;
    }

    pub(crate) fn initial_line(&self, text: &str) -> LineEditState {
        payload::editor_line(&self.payload, text)
    }

    pub(crate) fn initial_query(&self) -> LineEditState {
        self.initial_line(payload::query(&self.payload).unwrap_or(""))
    }

    pub(crate) fn edit_query(
        &mut self,
        operation: impl FnOnce(&mut LineEditState) -> bool,
    ) -> bool {
        let handled = operation(self.edit_query_mut());
        self.reset_completion_for_query();
        handled
    }

    fn edit_query_mut(&mut self) -> &mut LineEditState {
        let payload = &mut self.payload;
        &mut self
            .editor
            .get_or_insert_with(|| {
                let (rest, editor) = mount(payload, payload::query(payload).unwrap_or(""), true);
                *payload = rest;
                editor
            })
            .line
    }

    pub(crate) fn edit_line_mut(&mut self, spelling: &str) -> &mut LineEditState {
        let payload = &mut self.payload;
        &mut self
            .editor
            .get_or_insert_with(|| {
                let (rest, editor) = mount(payload, spelling, false);
                *payload = rest;
                editor
            })
            .line
    }

    fn query_changed(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| {
            editor.query && payload::query(&self.payload).unwrap_or("") != editor.line.text()
        })
    }

    fn reset_completion_for_query(&mut self) {
        if self.query_changed()
            && let Some(editor) = &self.editor
        {
            self.payload = payload::with_completion_query(&self.payload, editor.line.text());
        }
    }
}

/// The stage `payload` names, an edge's unless it names another.
fn stage_of(payload: &Value) -> Stage {
    match payload::stage(payload) {
        Some(stage) if stage == payload::vocabulary::PENDING => Stage::Pending,
        Some(stage) if stage == payload::vocabulary::LABEL => Stage::Label,
        _ => Stage::Edge,
    }
}

/// The live editor `payload` describes over `text`, a query's or a value's,
/// and the payload without that description: the editor owns it once
/// mounted.
fn mount(payload: &Value, text: &str, query: bool) -> (Value, Editor) {
    (
        payload::without_editor(payload),
        Editor {
            line: payload::editor_line(payload, text),
            query,
            recorded: false,
        },
    )
}

/// A value-stage pending: the location named by `path` does not
/// exist, and its value is being authored.
pub(crate) fn pending_value(root: &Root, scope: Scope, path: Path) -> Selection {
    pending_with_query(root, scope, path, "")
}

/// A value pending with `seed` already typed as its query, as typing over a
/// selected value opens one.
pub(crate) fn pending_with_query(root: &Root, scope: Scope, path: Path, seed: &str) -> Selection {
    query_selection(root, scope, path, payload::pending(seed))
}

/// Decode an incoming pending selection. The caret defaults to the
/// end of the query when no offset was supplied.
fn query_selection(root: &Root, scope: Scope, path: Path, payload: Value) -> Selection {
    let (payload, editor) = mount(&payload, payload::query(&payload).unwrap_or(""), true);
    Selection {
        root: root.clone(),
        path,
        scope,
        payload,
        editor: Some(editor),
        origin: None,
    }
}

/// A new field on a writable record, following a cell if needed.
pub(crate) fn pending_edge(root: &Root, read: &Read, parent: Path) -> Option<Selection> {
    let value = read.resolve_path(&parent)?;
    let parent = match value {
        Value::Record(_) => parent,
        Value::Cell(cell) => {
            let value = read.sources.resolve(*cell)?;
            value.value.as_record()?;
            let mut followed = parent;
            followed.push(Step::Follow(value.resolution));
            followed
        }
        Value::Blob(_) | Value::List(_) => return None,
    };
    read.writable(&parent).then_some(())?;
    Some(query_selection(
        root,
        read.scope.clone(),
        parent,
        payload::label(""),
    ))
}

/// A bare cell's value being authored: the within-gesture's meaning
/// on a referenced identity with no value yet.
pub(crate) fn pending_follow(root: &Root, read: &Read, path: &[Step]) -> Option<Selection> {
    let cell = read.resolve_path(path)?.as_cell()?;
    read.sources.resolve(cell).is_none().then_some(())?;
    let mut followed = path.to_vec();
    followed.push(Step::Follow(Resolution::Document));
    read.writable(&followed).then_some(())?;
    Some(pending_value(root, read.scope.clone(), followed))
}

/// A pending sibling next to the element at `path` (which must sit at
/// an element step), minted between it and its neighbor. The list
/// projection's gesture.
fn pending_beside(root: &Root, read: &Read, path: &[Step], after: bool) -> Option<Selection> {
    let (parent_path, position) = match path {
        [parent_path @ .., Step::Element(position)] => Some((parent_path, position)),
        _ => None,
    }?;
    let elements = read.resolve_path(parent_path)?.as_list()?;
    // Stated, not incidental: a list under an external cell takes no
    // minted siblings (the write would decline anyway, but a pending
    // that opens and cannot commit is an affordance lie).
    read.writable(parent_path).then_some(())?;
    let positions: Vec<&Position> = elements.keys().collect();
    let index = positions.iter().position(|p| *p == position)?;
    let fresh = if after {
        position::between(Some(position), positions.get(index + 1).copied())?
    } else {
        position::between(index.checked_sub(1).map(|i| positions[i]), Some(position))?
    };
    let mut fresh_path = parent_path.to_vec();
    fresh_path.push(Step::Element(fresh));
    Some(pending_value(root, read.scope.clone(), fresh_path))
}

/// A pending element inside the list at `path` — inline, or a link's
/// cell value, normalized through Follow — appended at the `end`, as
/// "add to this list" does, or else prepended at the front. Only lists
/// take elements, by type, and the owning cell must be writable, as in
/// [`pending_edge`].
pub(crate) fn pending_into(
    root: &Root,
    read: &Read,
    path: &[Step],
    end: bool,
) -> Option<Selection> {
    let value = read.resolve_path(path)?;
    let (list_path, elements) = match value {
        Value::List(elements) => (path.to_vec(), elements),
        Value::Cell(cell) => {
            let value = read.sources.resolve(*cell)?;
            let elements = value.value.as_list()?;
            let mut followed = path.to_vec();
            followed.push(Step::Follow(value.resolution));
            (followed, elements)
        }
        Value::Blob(_) | Value::Record(_) => return None,
    };
    read.writable(&list_path).then_some(())?;
    let positions: Vec<&Position> = elements.keys().collect();
    let fresh = if end {
        position::between(positions.last().copied(), None)?
    } else {
        position::between(None, positions.first().copied())?
    };
    let mut fresh_path = list_path;
    fresh_path.push(Step::Element(fresh));
    Some(pending_value(root, read.scope.clone(), fresh_path))
}

/// Plain Enter: a new peer BESIDE the selection — continue the
/// enumeration you are in. An element pends a sibling (before with
/// shift); a field value pends a new field on its parent; the root
/// has nothing beside it and falls within — a field on a record, an
/// appended element on a list. A cell's whole value is drawn where its
/// reference is, so it continues the reference's enumeration.
pub(crate) fn pending_enter(
    root: &Root,
    read: &Read,
    path: &[Step],
    before: bool,
) -> Option<Selection> {
    match path {
        [reference @ .., Step::Follow(_)] => pending_enter(root, read, reference, before),
        _ => pending_beside(root, read, path, !before)
            .or_else(|| {
                path.split_last()
                    .and_then(|(_, parent)| pending_edge(root, read, parent.to_vec()))
            })
            .or_else(|| pending_edge(root, read, path.to_vec()))
            .or_else(|| pending_into(root, read, path, true)),
    }
}

/// The command chord: author WITHIN the selection — a new field on
/// the selected record or cell, an element appended into a list, or
/// a bare cell's first value. With shift, the front instead —
/// prepend. Atoms other than links have no within and decline.
pub(crate) fn pending_insert(
    root: &Root,
    read: &Read,
    path: &[Step],
    front: bool,
) -> Option<Selection> {
    if front {
        pending_into(root, read, path, false)
    } else {
        pending_edge(root, read, path.to_vec())
            .or_else(|| pending_into(root, read, path, true))
            .or_else(|| pending_follow(root, read, path))
    }
}

/// A pending root for an empty document.
pub fn pending_root(root: &Root, sources: &Sources) -> Option<Selection> {
    sources
        .root()
        .is_none()
        .then(|| pending_value(root, Scope::default(), Vec::new()))
}

/// The value a pending query resolves to: a leading quote forces
/// text (the closing quote optional, so text mode holds while
/// typing), `0x` hex reads as a blob, anything else is text as typed.
pub fn resolve_query(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.strip_prefix('"') {
        Some(inner) => text::value(inner.strip_suffix('"').unwrap_or(inner)),
        None => trimmed
            .strip_prefix("0x")
            .and_then(gid::read_hex)
            .map(Value::from)
            .unwrap_or_else(|| text::value(text)),
    }
}

/// After a commit, select `path` below the committing site, with `payload`.
pub fn at(path: &[Step], payload: Value) -> crate::site::Continuation {
    let path = path.to_vec();
    Rc::new(move |_, site, staged| {
        staged.select(site.iter().chain(&path).cloned().collect(), payload.clone());
        true
    })
}

/// After a commit, open a pending at `path` below the committing site.
pub fn pending_at(path: &[Step]) -> crate::site::Continuation {
    at(path, payload::empty_pending())
}

/// The selection as data: the payload projections receive when their
/// path is the selected one. Stage is a named cell; the query rides
/// the text convention and the choice the f64 convention. Editor
/// gesture internals (text in motion, caret, anchor, preedit, drag)
/// are encoded when requested and decoded when a capability supplies
/// a replacement. They are never mirrored in the stored payload.
pub mod payload {
    use ::grap::f64 as f64_convention;
    use ::grap::text;
    use gid::{CellId, Value};
    use kurbo::Point;
    use puri::edit::LineEditState;

    pub mod vocabulary {
        use gid::CellId;

        /// A selection's stage: an edge, or a pending value or label being
        /// typed. The editor library names these.
        pub const STAGE: CellId = CellId::from_u128(0x6a1fd3082b9c47e5f60d21a8c45e9b37);
        pub const EDGE: CellId = CellId::from_u128(0x2f74c8a1936e05bd4c17e2b98d60a5f4);
        pub const PENDING: CellId = CellId::from_u128(0x91d5e60b3a8f27c4058b39f6d2c471ea);
        pub const LABEL: CellId = CellId::from_u128(0x7be29f4680d1c5a3f2496e07b85d13c2);
        pub const QUERY: CellId = CellId::from_u128(0xc25e80f7d1934ab6270c8f5e13b6d4a9);
        pub const CHOICE: CellId = CellId::from_u128(0x48b7a92c05e1d6f3891a4d20e7c53f6b);
        pub const COMPLETION_SCROLL: CellId = CellId::from_u128(0x151767a413a8bc5f579465dd67f18263);

        /// Selection byte offsets; FOCUS may precede ANCHOR.
        pub const ANCHOR: CellId = CellId::from_u128(0x5d38a1c7f24e9b60d15c7a02e83f46b9);
        pub const FOCUS: CellId = CellId::from_u128(0xa906e35d21c84f7bc3d05e918b62fa47);
        /// The in-flight IME composition: a text value with optional
        /// START/END cursor fields overlaid.
        pub const PREEDIT: CellId = CellId::from_u128(0x1c84f0b6d97325ea40d6b18c53e29f74);
        pub const START: CellId = CellId::from_u128(0xf27b950e13a8d64c26f9e30a71d45b8c);
        pub const END: CellId = CellId::from_u128(0x60d3e94a852f17bd39c2a45f08e61d73);
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

    /// A pending with nothing typed yet.
    pub fn empty_pending() -> Value {
        Value::record([(vocabulary::STAGE, Value::Cell(vocabulary::PENDING))])
    }

    /// A pending value with `query` typed, its first offer chosen.
    pub fn pending(query: &str) -> Value {
        Value::record([
            (vocabulary::STAGE, Value::Cell(vocabulary::PENDING)),
            (vocabulary::QUERY, text::value(query)),
            (vocabulary::CHOICE, f64_convention::value(0.0)),
        ])
    }

    /// A new field's label with `query` typed, its first offer chosen.
    pub fn label(query: &str) -> Value {
        Value::record([
            (vocabulary::STAGE, Value::Cell(vocabulary::LABEL)),
            (vocabulary::QUERY, text::value(query)),
            (vocabulary::CHOICE, f64_convention::value(0.0)),
        ])
    }

    pub fn stage(payload: &Value) -> Option<CellId> {
        payload.as_record()?.get(&vocabulary::STAGE)?.as_cell()
    }

    pub fn query(payload: &Value) -> Option<&str> {
        text::read(payload.as_record()?.get(&vocabulary::QUERY)?)
    }

    pub fn choice(payload: &Value) -> Option<usize> {
        index(payload.as_record()?.get(&vocabulary::CHOICE)?)
    }

    /// The whole, non-negative number `value` spells, as an index or offset.
    fn index(value: &Value) -> Option<usize> {
        f64_convention::read(value).and_then(f64_convention::to_usize)
    }

    pub fn completion_scroll(payload: &Value) -> Option<f64> {
        f64_convention::read(payload.as_record()?.get(&vocabulary::COMPLETION_SCROLL)?)
            .filter(|scroll| scroll.is_finite() && *scroll >= 0.0)
    }

    pub fn with_completion_query(payload: &Value, query: &str) -> Value {
        if self::query(payload) == Some(query) {
            payload.clone()
        } else {
            with_field(
                &with_completion_view(payload, 0.0, 0),
                vocabulary::QUERY,
                text::value(query),
            )
        }
    }

    pub fn with_completion_view(payload: &Value, scroll: f64, choice: usize) -> Value {
        let fields = payload.as_record().cloned().unwrap_or_default();
        Value::Record(
            fields
                .update(vocabulary::COMPLETION_SCROLL, f64_convention::value(scroll))
                .update(vocabulary::CHOICE, f64_convention::value(choice as f64)),
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
        let field_index = |key: CellId| index(payload.as_record()?.get(&key)?);
        let anchor = field_index(vocabulary::ANCHOR).unwrap_or(text.len());
        let focus = field_index(vocabulary::FOCUS).unwrap_or(text.len());
        let preedit = payload
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::PREEDIT))
            .and_then(|composed| {
                let cursor = composed.as_record().and_then(|fields| {
                    let offset = |key: CellId| index(fields.get(&key)?);
                    Some((offset(vocabulary::START)?, offset(vocabulary::END)?))
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
    mod tests;
}

#[cfg(test)]
mod tests;
