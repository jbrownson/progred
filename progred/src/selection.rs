//! Selection and collapse. Selection paths name projected occurrences, and
//! say where the data a mutation writes is.

use crate::editing::Read;
use crate::location::Location;
use crate::occurrence::{Path, Step};
use crate::sources::Sources;
use crate::workspace::View;
use ::grap::text;
use gid::{Position, Value, position};
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
    /// The view the path is in.
    view: Rc<View>,
    /// Where: the value's path for edge and pending stages, the
    /// parent record's for a label stage.
    path: Path,
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
    /// Reify a payload at a host-owned path: the mutation boundary Grap
    /// capabilities use after decoding the path.
    /// Editor fields are decoded once into their live owner, then removed
    /// from the stored payload.
    pub(crate) fn from_payload(
        sources: &Sources,
        view: &Rc<View>,
        path: Path,
        payload: Value,
    ) -> Self {
        let mut selection = match stage_of(&payload) {
            Stage::Pending | Stage::Label => query_selection(view, path, payload),
            Stage::Edge => {
                let read = Read { sources: *sources };
                let mounted = payload::editor_text(&payload)
                    .filter(|_| read.writable(&path))
                    .map(|text| mount(&payload, text, read.resolve_path(&path).is_none()));
                let (payload, editor) = match mounted {
                    Some((payload, editor)) => (payload, Some(editor)),
                    None => (payload, None),
                };
                Self {
                    view: view.clone(),
                    path,
                    payload,
                    editor,
                    origin: None,
                }
            }
        };
        selection.reset_completion_for_query();
        selection
    }

    /// The value at `path` in `view`.
    pub(crate) fn edge(view: &Rc<View>, path: Path) -> Self {
        Self {
            view: view.clone(),
            path,
            payload: payload::edge(),
            editor: None,
            origin: None,
        }
    }

    pub fn path(&self) -> &[Step] {
        &self.path
    }

    /// This selection selected again: its line's caret moves to the end.
    pub(crate) fn reselected(mut self) -> Self {
        if let Some(line) = self.edit_mut() {
            line.cursor_to_end();
        }
        self
    }

    pub(crate) fn origin(&self) -> Option<&[Step]> {
        self.origin.as_deref()
    }

    pub(crate) fn with_origin(mut self, origin: Path) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Where the selected data is, as its path says.
    pub(crate) fn location(&self) -> Option<Location<'static>> {
        crate::occurrence::location(&self.path)
    }

    pub(crate) fn value<'a>(&self, sources: &Sources<'a>) -> Option<&'a Value> {
        crate::occurrence::read(sources, &self.path)
    }

    pub(crate) fn writable(&self, sources: &Sources<'_>) -> bool {
        crate::occurrence::writable(sources, &self.path)
    }

    /// The view the selection is in.
    pub fn view(&self) -> &Rc<View> {
        &self.view
    }

    pub(crate) fn relocate(&mut self, path: Path) {
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

    /// This pending, its value just written in its place from what its
    /// query typed: the value selected at its edge, typing going on in its
    /// line from the query's caret. That write opened the run's undo step.
    pub(crate) fn typing_on(&mut self) {
        self.payload = payload::edge();
        self.origin = None;
        if let Some(editor) = &mut self.editor {
            editor.query = false;
            editor.recorded = true;
        }
    }

    /// The line being typed here, mounted over `spelling` if none is. A
    /// pending's query becomes the line's own when a line drawn where the
    /// value is missing types into it.
    pub(crate) fn edit_line_mut(&mut self, spelling: &str) -> &mut LineEditState {
        let payload = &mut self.payload;
        let editor = self.editor.get_or_insert_with(|| {
            let (rest, editor) = mount(payload, spelling, false);
            *payload = rest;
            editor
        });
        editor.query = false;
        &mut editor.line
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
pub(crate) fn pending_value(view: &Rc<View>, path: Path) -> Selection {
    pending_with_query(view, path, "")
}

/// A value pending with `seed` already typed as its query, as typing over a
/// selected value opens one.
pub(crate) fn pending_with_query(view: &Rc<View>, path: Path, seed: &str) -> Selection {
    query_selection(view, path, payload::pending(seed))
}

/// Decode an incoming pending selection. The caret defaults to the
/// end of the query when no offset was supplied.
fn query_selection(view: &Rc<View>, path: Path, payload: Value) -> Selection {
    let (payload, editor) = mount(&payload, payload::query(&payload).unwrap_or(""), true);
    Selection {
        view: view.clone(),
        path,
        payload,
        editor: Some(editor),
        origin: None,
    }
}

/// A new field on a writable record, following a cell if needed.
pub(crate) fn pending_edge(read: &Read<'_>, view: &Rc<View>, parent: Path) -> Option<Selection> {
    let value = read.resolve_path(&parent)?;
    let parent = match value {
        Value::Record(_) => parent,
        Value::Cell(cell) => {
            read.sources.value(*cell)?.as_record()?;
            let mut followed = parent;
            followed.push(Step::Follow);
            followed
        }
        Value::Blob(_) | Value::List(_) => return None,
    };
    read.writable(&parent).then_some(())?;
    Some(query_selection(view, parent, payload::label("")))
}

/// A bare cell's value being authored: the within-gesture's meaning
/// on a referenced identity with no value yet.
pub(crate) fn pending_follow(read: &Read<'_>, view: &Rc<View>, path: &[Step]) -> Option<Selection> {
    let cell = read.resolve_path(path)?.as_cell()?;
    read.sources.resolve(cell).is_none().then_some(())?;
    let mut followed = path.to_vec();
    followed.push(Step::Follow);
    read.writable(&followed).then_some(())?;
    Some(pending_value(view, followed))
}

/// A pending sibling next to the element at `path` (which must sit at
/// an element step), minted between it and its neighbor. The list
/// projection's gesture.
fn pending_beside(
    read: &Read<'_>,
    view: &Rc<View>,
    path: &[Step],
    after: bool,
) -> Option<Selection> {
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
    Some(pending_value(view, fresh_path))
}

/// A pending element inside the list at `path` — inline, or a link's
/// cell value, normalized through Follow — appended at the `end`, as
/// "add to this list" does, or else prepended at the front. Only lists
/// take elements, by type, and the owning cell must be writable, as in
/// [`pending_edge`].
pub(crate) fn pending_into(
    read: &Read<'_>,
    view: &Rc<View>,
    path: &[Step],
    end: bool,
) -> Option<Selection> {
    let value = read.resolve_path(path)?;
    let (list_path, elements) = match value {
        Value::List(elements) => (path.to_vec(), elements),
        Value::Cell(cell) => {
            let elements = read.sources.value(*cell)?.as_list()?;
            let mut followed = path.to_vec();
            followed.push(Step::Follow);
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
    Some(pending_value(view, fresh_path))
}

/// Plain Enter: a new peer BESIDE the selection — continue the
/// enumeration you are in. An element pends a sibling (before with
/// shift); a field value pends a new field on its parent; where the
/// selection's area `starts`, and where a jump or computed value starts,
/// nothing drawn is beside it, so it falls within — a field on a record, an
/// appended element on a list. A cell's whole value is drawn where its
/// reference is, so it continues the reference's enumeration.
pub(crate) fn pending_enter(
    read: &Read<'_>,
    view: &Rc<View>,
    path: &[Step],
    starts: &[Step],
    before: bool,
) -> Option<Selection> {
    match path {
        _ if path == starts => pending_insert(read, view, path, before),
        [reference @ .., Step::Follow] => pending_enter(read, view, reference, starts, before),
        [.., Step::Jump(_) | Step::Computed] => pending_insert(read, view, path, before),
        _ => pending_beside(read, view, path, !before)
            .or_else(|| {
                path.split_last()
                    .and_then(|(_, parent)| pending_edge(read, view, parent.to_vec()))
            })
            .or_else(|| pending_edge(read, view, path.to_vec()))
            .or_else(|| pending_into(read, view, path, true)),
    }
}

/// The command chord: author WITHIN the selection — a new field on
/// the selected record or cell, an element appended into a list, or
/// a bare cell's first value. With shift, the front instead —
/// prepend. Atoms other than links have no within and decline.
pub(crate) fn pending_insert(
    read: &Read<'_>,
    view: &Rc<View>,
    path: &[Step],
    front: bool,
) -> Option<Selection> {
    if front {
        pending_into(read, view, path, false)
    } else {
        pending_edge(read, view, path.to_vec())
            .or_else(|| pending_into(read, view, path, true))
            .or_else(|| pending_follow(read, view, path))
    }
}

/// A pending root for an empty document, in its view, `document`.
pub fn pending_root(sources: &Sources, document: &Rc<View>) -> Option<Selection> {
    sources
        .root()
        .is_none()
        .then(|| pending_value(document, Vec::new()))
}

/// The value a pending query resolves to: a leading quote forces
/// text (the closing quote optional, so text mode holds while
/// typing), `0x` hex reads as a blob, anything else is text as typed.
pub fn resolve_query(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.strip_prefix('"') {
        Some(inner) => text::value(inner.strip_suffix('"').unwrap_or(inner)),
        None => gid::Blob::read(trimmed)
            .map(Value::Blob)
            .unwrap_or_else(|| text::value(text)),
    }
}

/// After a commit, select `path` below the committing site, with `payload`.
pub fn at(path: &[gid::Step], payload: Value) -> crate::site::Continuation {
    let path = path.to_vec();
    Rc::new(move |_, site, staged| {
        staged.select(crate::occurrence::below(site, &path), payload.clone());
        true
    })
}

/// After a commit, open a pending at `path` below the committing site.
pub fn pending_at(path: &[gid::Step]) -> crate::site::Continuation {
    at(path, payload::empty_pending())
}

/// The selection as data: the payload projections receive when their
/// path is the selected one. Stage is a named cell; the query rides
/// the text convention and the choice the f64 convention. Editor
/// gesture internals (text in motion, caret, anchor, preedit, drag)
/// are encoded when requested and decoded when a capability supplies
/// a replacement. They are never mirrored in the stored payload.
pub mod payload;

#[cfg(test)]
#[path = "tests/selection.rs"]
mod tests;
