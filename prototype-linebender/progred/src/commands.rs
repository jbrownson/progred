//! Editor commands: insert, delete, clipboard, rename, collapse.

use crate::completion;
use crate::graph_view;
use crate::model::Selected;
use crate::navigate;
use crate::projection;
use crate::selection;
use crate::sources;
use crate::{App, CLIPBOARD_FORMAT, plain};
use gid::{CellId, Path, Step, Value};
use puri::edit::LineEditState;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

impl App {
    /// Graph-view keys: Delete detaches the selected node — the
    /// cell's whole entry removed and every link to it unlinked, or
    /// the root emptied. The one selection slot means this and the
    /// document delete below can never both match; Escape falls
    /// through to the universal clear in `insert_key`.
    pub(crate) fn graph_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && plain(event)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && match self.model.graph_selection() {
                Some(selection) => {
                    let selection = *selection;
                    let before = self.model.doc.clone();
                    if graph_view::delete_selection(&mut self.model.doc, &selection) {
                        self.model.history.record(before, None);
                        self.refresh_title();
                    }
                    self.model.selection = None;
                    true
                }
                None => false,
            }
    }

    /// Backspace or Delete removes the selected edge — a focused atom
    /// editor claims the keys while it has text and declines on an
    /// empty buffer, so emptying a string then backspacing again
    /// deletes the element. Selection lands on the next sibling, else
    /// the previous, else the parent.
    pub(crate) fn delete_key(
        &mut self,
        descends: &[navigate::Descend],
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && plain(event)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && self.delete_selected_edge(descends)
    }

    /// Deletes the selected edge and lands the selection on a
    /// survivor — Backspace/Delete's action, and cut's second half.
    pub(crate) fn delete_selected_edge(&mut self, descends: &[navigate::Descend]) -> bool {
        match &self.model.selection {
            // Only a real edge deletes; a pending's Backspace is its
            // cancel, handled by insert_key.
            Some(Selected::Tree(current))
                if current.stage() == selection::Stage::Edge =>
            {
                let path = current.path().to_vec();
                // Backspacing through the value and once more to
                // delete the edge is one gesture: when this edge has
                // the open run, its frame (pre-run document, edge
                // intact) already covers the deletion.
                let covered = current.recorded();
                let before = self.model.doc.clone();
                selection::delete_edge(&mut self.model.doc, &self.stack.library, &path) && {
                    if !covered {
                        self.model.history.record(before, Some(path.clone()));
                        self.refresh_title();
                    }
                    let next = navigate::selection_after_delete(descends, &path);
                    self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                        &self.sources(),
                        &self.stack.projection,
                        next,
                    )));
                    true
                }
            }
            _ => false,
        }
    }

    /// The chosen entry's action — from the frame's popup, else the
    /// query's inferred atom.
    pub(crate) fn chosen_action(
        popup: &Option<completion::Popup>,
        query: &LineEditState,
        choice: usize,
        labels: bool,
    ) -> completion::EntryAction {
        popup
            .as_ref()
            .and_then(|p| p.entries.get(choice.min(p.entries.len().saturating_sub(1))))
            .map(|entry| entry.action.clone())
            .unwrap_or_else(|| {
                if labels {
                    completion::EntryAction::NewLabel(query.text().to_string())
                } else {
                    completion::EntryAction::Value(selection::resolve_query(query.text()))
                }
            })
    }

    /// Commits a pointed-at value into the open pending — the
    /// command-click gesture. A value-stage pending commits and
    /// selects the edge; a label stage advances to its value stage.
    /// False when nothing is pending — or when the picked value
    /// cannot label (a list, a record, a blob) at the label stage —
    /// so the click falls through rather than spending the pending.
    pub(crate) fn pick_identity(&mut self, id: Value) -> bool {
        if matches!(
            &self.model.selection,
            Some(Selected::Tree(current)) if current.stage() == selection::Stage::Label
        ) && id.as_cell().is_none()
        {
            return false;
        }
        match self.model.selection.take() {
            Some(Selected::Tree(current)) => match current.stage() {
                selection::Stage::Pending => {
                    self.commit_value(
                        current.path().to_vec(),
                        &completion::EntryAction::Value(id),
                    );
                    true
                }
                selection::Stage::Label => {
                    self.commit_label(
                        current.path().to_vec(),
                        current.replacing(),
                        &completion::EntryAction::Value(id),
                    );
                    true
                }
                selection::Stage::Edge => {
                    self.model.selection = Some(Selected::Tree(current));
                    false
                }
            },
            selection => {
                self.model.selection = selection;
                false
            }
        }
    }

    /// Commits the pending value stage — one undo step — and selects
    /// the edge it wrote.
    pub(crate) fn commit_value(&mut self, path: Path, action: &completion::EntryAction) {
        let before = self.model.doc.clone();
        if completion::commit_pending(&mut self.model.doc, &self.stack.library, &path, action) {
            self.model.history.record(before, None);
            self.refresh_title();
        }
        self.model.selection = Some(Selected::Tree(selection::Selection::edge(
            &self.sources(),
            &self.stack.projection,
            path,
        )));
    }

    /// A resolved label advances the pending edge to its value stage —
    /// or selects the existing field when the label is taken (rename
    /// included: a taken label never clobbers its field, selection
    /// communicates it, and replacing it means deleting it first).
    /// A free-text label persists its newly named cell before the
    /// value stage; a bare-cell choice has nothing to persist. A
    /// rename re-keys the field and creates its label cell in one
    /// history step, the value carried.
    pub(crate) fn commit_label(
        &mut self,
        parent: Path,
        replacing: Option<CellId>,
        action: &completion::EntryAction,
    ) {
        let Some((label, created)) = completion::resolve_label(action) else {
            return;
        };
        let mut path = parent.clone();
        path.push(Step::Key(label));
        if self.sources().resolve(&path).is_some() {
            self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                &self.sources(),
                &self.stack.projection,
                path,
            )));
            return;
        }
        match replacing {
            Some(old) => {
                let before = self.model.doc.clone();
                if let Some((cell, value)) = &created {
                    self.model.doc.cells.set_value(*cell, value.clone());
                }
                let renamed = selection::rename_field(
                    &mut self.model.doc,
                    &self.stack.library,
                    &parent,
                    &old,
                    label,
                );
                if renamed {
                    self.model.history.record(before, None);
                    self.refresh_title();
                } else {
                    if let Some((cell, _)) = created {
                        self.model.doc.cells.clear_value(cell);
                    }
                    // The rename could not land; back to the field.
                    path = parent;
                    path.push(Step::Key(old));
                }
                self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                    &self.sources(),
                    &self.stack.projection,
                    path,
                )));
            }
            None => {
                if let Some((cell, value)) = created {
                    let before = self.model.doc.clone();
                    self.model.doc.cells.set_value(cell, value);
                    self.model.history.record(before, None);
                    self.refresh_title();
                }
                self.model.selection = Some(Selected::Tree(selection::pending_value(path)));
            }
        }
    }

    /// Structural copy/paste, the shell's fallback: a focused text
    /// editor's own clipboard handling wins by dispatch order, so
    /// these fire on cell, list, and graph selections. Deliberately
    /// NOT menu items — muda accelerators intercept ahead of key
    /// dispatch, which would take Cmd+C/V away from text editing.
    pub(crate) fn clipboard_key(
        &mut self,
        descends: &[navigate::Descend],
        event: &KeyboardEvent,
    ) -> bool {
        if !event.state.is_down() || !projection::command(&event.modifiers) {
            return false;
        }
        let Key::Character(c) = &event.key else {
            return false;
        };
        match c.to_lowercase().as_str() {
            "c" => self.copy_selection(),
            "x" => self.copy_selection() && self.delete_selected_edge(descends),
            "v" => self.paste_clipboard(),
            _ => false,
        }
    }

    /// Copies the selected value — SHALLOW: a link is its identity
    /// alone, no cell values travel; the value carries its own inline
    /// structure. Graph selections copy their node's value.
    pub(crate) fn copy_selection(&self) -> bool {
        use clipboard_rs::{Clipboard, ClipboardContext};
        let sources = self.sources();
        let value = match &self.model.selection {
            Some(Selected::Tree(selection)) => sources.resolve(selection.path()).cloned(),
            Some(Selected::Graph(graph_view::GraphSelection::Node(node))) => {
                graph_view::node_value(&self.model.doc, node)
            }
            None => None,
        };
        let Some(value) = value else {
            return false;
        };
        let (text, structural) = selection::to_clipboard(&value);
        ClipboardContext::new()
            .and_then(|cb| {
                if structural {
                    // Both representations: the private format says
                    // "structure", the text reads anywhere.
                    cb.set(vec![
                        clipboard_rs::ClipboardContent::Other(
                            CLIPBOARD_FORMAT.to_string(),
                            text.clone().into_bytes(),
                        ),
                        clipboard_rs::ClipboardContent::Text(text),
                    ])
                } else {
                    cb.set_text(text)
                }
            })
            .is_ok()
    }

    /// The private format's payload, when the clipboard carries one.
    pub(crate) fn clipboard_structure(&self) -> Option<Value> {
        use clipboard_rs::{Clipboard, ClipboardContext};
        let bytes = ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_buffer(CLIPBOARD_FORMAT).ok())?;
        selection::from_structure(&bytes)
    }

    /// Cmd+V while a pending is open and the clipboard CARRIES
    /// STRUCTURE — the private format, not a text shape — commits the
    /// value into the pending, ahead of the focused query's own text
    /// paste. Everything else keeps the text path: pasting "hi",
    /// 0xff, or even text that happens to spell Value JSON lands in
    /// the query as characters. Claims the chord even when the pick
    /// declines (the label stage takes only what can label).
    pub(crate) fn pending_paste_key(&mut self, event: &KeyboardEvent) -> bool {
        if !event.state.is_down() || !projection::command(&event.modifiers) {
            return false;
        }
        if !matches!(&event.key, Key::Character(c) if c.to_lowercase().as_str() == "v") {
            return false;
        }
        if !matches!(
            &self.model.selection,
            Some(Selected::Tree(current)) if current.stage() != selection::Stage::Edge
        ) {
            return false;
        }
        let Some(value) = self.clipboard_structure() else {
            return false;
        };
        self.pick_identity(value);
        true
    }

    /// Pastes the clipboard's value — the private format's structure
    /// when it carries one, else the text's query reading: into an
    /// open pending first (the label stage narrows to atoms through
    /// the pick), else over the selected edge — one undo step, the
    /// selection remounted so a pasted atom gets its editor.
    pub(crate) fn paste_clipboard(&mut self) -> bool {
        use clipboard_rs::{Clipboard, ClipboardContext};
        let value = match self.clipboard_structure() {
            Some(value) => value,
            None => {
                let Some(text) = ClipboardContext::new()
                    .ok()
                    .and_then(|cb| cb.get_text().ok())
                else {
                    return false;
                };
                if text.is_empty() {
                    return false;
                }
                selection::from_clipboard(&text)
            }
        };
        if self.pick_identity(value.clone()) {
            return true;
        }
        let Some(Selected::Tree(current)) = &self.model.selection else {
            return false;
        };
        if current.stage() != selection::Stage::Edge {
            return false;
        }
        let path = current.path().to_vec();
        // Idempotent pastes stay off the undo stack, as write_through
        // keeps no-op rewrites off it.
        if self.sources().resolve(&path) == Some(&value) {
            return true;
        }
        let before = self.model.doc.clone();
        if selection::set_value(&mut self.model.doc, &self.stack.library, &path, value) {
            self.model.history.record(before, Some(path.clone()));
            self.refresh_title();
            self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                &self.sources(),
                &self.stack.projection,
                path,
            )));
            true
        } else {
            false
        }
    }

    /// Enter advances a pending stage or begins one (the chains live
    /// in raw). Plain Enter is a new peer BESIDE the selection: a
    /// sibling element in a list (Shift+Enter before), a new field on
    /// the parent record otherwise; the root has nothing beside it
    /// and takes the field on itself. The command chord authors
    /// WITHIN the selection: a field on the selected cell, an
    /// appended element on a list (with Shift, at the front). Labels
    /// author first, then values; list elements are one-stage value
    /// pendings, the projection minting the position.
    /// On an empty document Enter begins the root value. Escape
    /// clears the selection from anywhere, discarding any pending
    /// with the graph untouched; Backspace on an empty query cancels
    /// a pending back to its anchor instead, keeping the keyboard
    /// flow.
    pub(crate) fn insert_key(
        &mut self,
        descends: &[navigate::Descend],
        popup: &Option<completion::Popup>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && match &event.key {
                // While pending, plain vertical arrows drive the popup
                // choice; chorded arrows stay structure keys.
                Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown))
                    if !projection::command(&event.modifiers) =>
                {
                    match &mut self.model.selection {
                        Some(Selected::Tree(current))
                            if current.stage() != selection::Stage::Edge =>
                        {
                            let len = popup.as_ref().map(|p| p.entries.len()).unwrap_or(0);
                            let choice = current.choice();
                            current.set_choice(match direction {
                                NamedKey::ArrowUp => choice.saturating_sub(1),
                                _ => (choice + 1).min(len.saturating_sub(1)),
                            });
                            true
                        }
                        _ => false,
                    }
                }
                Key::Named(NamedKey::Enter) => match self.model.selection.take() {
                    Some(Selected::Tree(current))
                        if current.stage() != selection::Stage::Edge =>
                    {
                        let labels = current.stage() == selection::Stage::Label;
                        let fallback = selection::line_edit("");
                        let query = current.edit().unwrap_or(&fallback);
                        let action = Self::chosen_action(popup, query, current.choice(), labels);
                        if labels {
                            self.commit_label(
                                current.path().to_vec(),
                                current.replacing(),
                                &action,
                            );
                        } else {
                            self.commit_value(current.path().to_vec(), &action);
                        }
                        true
                    }
                    selection => {
                        // Only a tree selection anchors authoring; a
                        // graph selection has no path to author at.
                        let tree = match &selection {
                            Some(Selected::Tree(current)) => Some(current),
                            _ => None,
                        };
                        let sources = self.sources();
                        let shift = event.modifiers.shift();
                        let started = match tree {
                            Some(current) if projection::command(&event.modifiers) => {
                                selection::pending_insert(&sources, current.path(), shift)
                            }
                            Some(current) => {
                                selection::pending_enter(&sources, current.path(), shift)
                            }
                            None => selection::pending_root(&sources),
                        };
                        let began = started.is_some();
                        self.model.selection = started.map(Selected::Tree).or(selection);
                        began
                    }
                },
                Key::Named(NamedKey::Escape) => self.model.selection.take().is_some(),
                Key::Named(NamedKey::Backspace) => {
                    match &self.model.selection {
                        Some(Selected::Tree(current))
                            if current.stage() == selection::Stage::Pending =>
                        {
                            let back =
                                navigate::selection_after_delete(descends, current.path());
                            // Cancelling the empty document's root
                            // pending deselects — reselecting it
                            // would pend again.
                            self.model.selection =
                                (!(back.is_empty() && self.model.doc.root.is_none())).then(|| {
                                    Selected::Tree(selection::Selection::edge(
                                        &self.sources(),
                                        &self.stack.projection,
                                        back,
                                    ))
                                });
                            true
                        }
                        Some(Selected::Tree(current))
                            if current.stage() == selection::Stage::Label =>
                        {
                            // A cancelled rename returns to its field;
                            // a cancelled new field to the record.
                            let mut back = current.path().to_vec();
                            if let Some(old) = current.replacing() {
                                back.push(Step::Key(old));
                            }
                            self.model.selection =
                                Some(Selected::Tree(selection::Selection::edge(
                                    &self.sources(),
                                    &self.stack.projection,
                                    back,
                                )));
                            true
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
    }

    /// Cmd+L re-opens the selected field's label as its seeded rename
    /// query — the keyboard route to what clicking the label does. The
    /// popup opens only on this explicit ask, never during navigation.
    /// (Cmd+R belongs to the Raw view toggle.)
    pub(crate) fn rename_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && projection::command(&event.modifiers)
            && matches!(&event.key, Key::Character(c) if c.to_lowercase().as_str() == "l")
            && match &self.model.selection {
                Some(Selected::Tree(current))
                    if current.stage() == selection::Stage::Edge =>
                {
                    let path = current.path().to_vec();
                    match selection::pending_rename(&self.sources(), &path) {
                        Some(pending) => {
                            self.model.selection = Some(Selected::Tree(pending));
                            true
                        }
                        None => false,
                    }
                }
                _ => false,
            }
    }

    /// Space toggles the selection's collapse override, and Cmd+Up /
    /// Cmd+Down close and open it — the fold axis of the keyboard's
    /// third dimension, under the same keys that walk the rows. A
    /// focused string editor claims Space first and types instead.
    pub(crate) fn collapse_key(&mut self, event: &KeyboardEvent) -> bool {
        if !event.state.is_down() {
            return false;
        }
        let set = match &event.key {
            Key::Character(c) if c.as_str() == " " => None,
            Key::Named(NamedKey::ArrowUp) if projection::command(&event.modifiers) => Some(true),
            Key::Named(NamedKey::ArrowDown) if projection::command(&event.modifiers) => Some(false),
            _ => return false,
        };
        let Some(Selected::Tree(current)) = &self.model.selection else {
            return false;
        };
        if current.stage() != selection::Stage::Edge {
            return false;
        }
        let path = current.path().to_vec();
        let sources = sources::Sources {
            doc: &self.model.doc,
            library: &self.stack.library,
        };
        match set {
            None => selection::toggle_collapse(
                &sources,
                &mut self.model.annotations,
                &path,
            ),
            Some(closed) => selection::set_collapse(
                &sources,
                &mut self.model.annotations,
                &path,
                closed,
            ),
        }
    }
}
