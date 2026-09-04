//! Editor commands: insert, delete, clipboard, and collapse.

#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::CLIPBOARD_FORMAT;
use crate::Editor;
use crate::completion;
use crate::modifiers;
use crate::navigate;
use crate::selection;
use crate::sources;
use gid::{Path, Step, Value};
use puri::edit::{LineEditState, TextClipboard};
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

impl Editor {
    /// Backspace or Delete removes the selected edge — a focused atom
    /// editor claims the keys while it has text and declines on an
    /// empty buffer, so emptying a string then backspacing again
    /// deletes the element. Selection lands on the next sibling, else
    /// the previous, else the parent.
    pub(crate) fn delete_key(
        &mut self,
        descends: &[navigate::Descend<Editor>],
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && modifiers::plain(&event.modifiers)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && self.delete_selected_edge(descends)
    }

    /// Deletes the selected edge and lands the selection on a
    /// survivor — Backspace/Delete's action, and cut's second half.
    pub(crate) fn delete_selected_edge(&mut self, descends: &[navigate::Descend<Editor>]) -> bool {
        match &self.model.selection {
            // Only a real edge deletes; a pending's Backspace is its
            // cancel, handled by insert_key.
            Some(current) if current.stage() == selection::Stage::Edge => {
                let root = current.root().clone();
                let path = current.path().to_vec();
                // Backspacing through the value and once more to
                // delete the edge is one gesture: when this edge has
                // the open run, its frame (pre-run document, edge
                // intact) already covers the deletion.
                let covered = current.recorded();
                let before = self.model.doc.clone();
                selection::delete_edge(&mut self.model.doc, &self.stack.libraries, &path) && {
                    if !covered {
                        self.model.history.record(before, Some(path.clone()));
                        self.refresh_title();
                    }
                    let next = navigate::selection_after_delete(descends, Some(&root), &path);
                    self.model.selection =
                        Some(selection::Selection::edge(&self.sources(), next).with_root(root));
                    true
                }
            }
            _ => false,
        }
    }

    /// The chosen entry's action — from the frame's offers, else the
    /// query's inferred atom.
    pub(crate) fn chosen_action(
        completion: &Option<completion::Offers>,
        query: &LineEditState,
        choice: usize,
        labels: bool,
    ) -> completion::EntryAction {
        completion
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
            Some(current) if current.stage() == selection::Stage::Label
        ) && id.as_cell().is_none()
        {
            return false;
        }
        match self.model.selection.take() {
            Some(current) => {
                let root = current.root().clone();
                match current.stage() {
                    selection::Stage::Pending => {
                        self.commit_value(
                            root,
                            current.path().to_vec(),
                            &completion::EntryAction::Value(id),
                        );
                        true
                    }
                    selection::Stage::Label => {
                        self.commit_label(
                            root,
                            current.path().to_vec(),
                            &completion::EntryAction::Value(id),
                        );
                        true
                    }
                    selection::Stage::Edge => {
                        self.model.selection = Some(current);
                        false
                    }
                }
            }
            selection => {
                self.model.selection = selection;
                false
            }
        }
    }

    /// Commits the pending value stage — one undo step — and selects
    /// the edge it wrote.
    pub(crate) fn commit_value(
        &mut self,
        root: crate::workspace::Root,
        path: Path,
        action: &completion::EntryAction,
    ) {
        let before = self.model.doc.clone();
        if completion::commit_pending(&mut self.model.doc, &self.stack.libraries, &path, action) {
            self.model.history.record(before, None);
            self.refresh_title();
        }
        self.model.selection =
            Some(selection::Selection::edge(&self.sources(), path).with_root(root));
    }

    /// A resolved new label advances the pending edge to its value
    /// stage, or selects the existing field when the label is taken.
    /// A free-text label persists its newly named cell first; a
    /// bare-cell choice has nothing to persist.
    pub(crate) fn commit_label(
        &mut self,
        root: crate::workspace::Root,
        parent: Path,
        action: &completion::EntryAction,
    ) {
        let Some((label, created)) = completion::resolve_label(action) else {
            return;
        };
        let mut path = parent.clone();
        path.push(Step::Key(label));
        if self.sources().resolve_path(&path).is_some() {
            self.model.selection =
                Some(selection::Selection::edge(&self.sources(), path).with_root(root));
            return;
        }
        if let Some((cell, value)) = created {
            let before = self.model.doc.clone();
            self.model.doc.cells.set_value(cell, value);
            self.model.history.record(before, None);
            self.refresh_title();
        }
        self.model.selection = Some(selection::pending_value(path).with_root(root));
    }

    /// Structural copy/paste, the shell's fallback: a focused text
    /// editor's own clipboard handling wins by dispatch order, so
    /// these fire on structural selections. Deliberately
    /// NOT menu items — native menu accelerators intercept ahead of key
    /// dispatch, which would take Cmd+C/V away from text editing.
    pub(crate) fn clipboard_key(
        &mut self,
        descends: &[navigate::Descend<Editor>],
        event: &KeyboardEvent,
    ) -> bool {
        if !event.state.is_down() || !modifiers::command(&event.modifiers) {
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
    /// structure.
    pub(crate) fn copy_selection(&mut self) -> bool {
        let sources = self.sources();
        let value = match &self.model.selection {
            Some(selection) => sources.resolve_path(selection.path()).cloned(),
            None => None,
        };
        let Some(value) = value else {
            return false;
        };
        let (text, structural) = selection::to_clipboard(&value);
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        return clipboard_rs::ClipboardContext::new()
            .and_then(|cb| {
                use clipboard_rs::Clipboard;
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
            .is_ok();
        #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
        {
            self.text_clipboard.text = Some(text);
            self.text_clipboard.structure = structural.then_some(value);
            true
        }
    }

    /// The private format's payload, when the clipboard carries one.
    pub(crate) fn clipboard_structure(&mut self) -> Option<Value> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            use clipboard_rs::Clipboard;
            let bytes = clipboard_rs::ClipboardContext::new()
                .ok()
                .and_then(|cb| cb.get_buffer(CLIPBOARD_FORMAT).ok())?;
            return selection::from_structure(&bytes);
        }
        #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
        self.text_clipboard.structure.clone()
    }

    /// Cmd+V while a pending is open and the clipboard CARRIES
    /// STRUCTURE — the private format, not a text shape — commits the
    /// value into the pending, ahead of the focused query's own text
    /// paste. Everything else keeps the text path: pasting "hi",
    /// 0xff, or even text that happens to spell Value JSON lands in
    /// the query as characters. Claims the chord even when the pick
    /// declines (the label stage takes only what can label).
    pub(crate) fn pending_paste_key(&mut self, event: &KeyboardEvent) -> bool {
        if !event.state.is_down() || !modifiers::command(&event.modifiers) {
            return false;
        }
        if !matches!(&event.key, Key::Character(c) if c.to_lowercase().as_str() == "v") {
            return false;
        }
        if !matches!(
            &self.model.selection,
            Some(current) if current.stage() != selection::Stage::Edge
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
    /// the pick), else over the selected edge — one undo step,
    /// retaining ordinary structural selection at the changed site.
    pub(crate) fn paste_clipboard(&mut self) -> bool {
        let value = match self.clipboard_structure() {
            Some(value) => value,
            None => {
                let Some(text) = self.text_clipboard.get_text() else {
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
        let Some(current) = &self.model.selection else {
            return false;
        };
        if current.stage() != selection::Stage::Edge {
            return false;
        }
        let root = current.root().clone();
        let path = current.path().to_vec();
        // Idempotent pastes stay off the undo stack, as write_through
        // keeps no-op rewrites off it.
        if self.sources().resolve_path(&path) == Some(&value) {
            return true;
        }
        let before = self.model.doc.clone();
        if selection::set_value(&mut self.model.doc, &self.stack.libraries, &path, value) {
            self.model.history.record(before, Some(path.clone()));
            self.refresh_title();
            self.model.selection =
                Some(selection::Selection::edge(&self.sources(), path).with_root(root));
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
    /// clears the selection from anywhere, discarding any pending;
    /// Backspace on an empty query cancels a pending back to its
    /// anchor instead, keeping the keyboard flow.
    pub(crate) fn insert_key(
        &mut self,
        descends: &[navigate::Descend<Editor>],
        completion: &Option<completion::Offers>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && match &event.key {
                Key::Named(NamedKey::Enter) => match self.model.selection.take() {
                    Some(current) if current.stage() != selection::Stage::Edge => {
                        let root = current.root().clone();
                        let labels = current.stage() == selection::Stage::Label;
                        let fallback = selection::line_edit("");
                        let query = current.edit().unwrap_or(&fallback);
                        let action =
                            Self::chosen_action(completion, query, current.choice(), labels);
                        if labels {
                            self.commit_label(root, current.path().to_vec(), &action);
                        } else {
                            self.commit_value(root, current.path().to_vec(), &action);
                        }
                        true
                    }
                    selection => {
                        let sources = self.sources();
                        let shift = event.modifiers.shift();
                        let root = selection
                            .as_ref()
                            .map(|current| current.root().clone())
                            .unwrap_or_else(|| self.model.workspace.document_root().clone());
                        let started = match selection.as_ref() {
                            Some(current) if modifiers::command(&event.modifiers) => {
                                selection::pending_insert(&sources, current.path(), shift)
                            }
                            Some(current) => {
                                selection::pending_enter(&sources, current.path(), shift)
                            }
                            None => selection::pending_root(&sources),
                        };
                        let began = started.is_some();
                        self.model.selection = started
                            .map(|selection| selection.with_root(root))
                            .or(selection);
                        began
                    }
                },
                Key::Named(NamedKey::Escape) => self.model.selection.take().is_some(),
                Key::Named(NamedKey::Backspace) => {
                    match &self.model.selection {
                        Some(current) if current.stage() == selection::Stage::Pending => {
                            let root = current.root().clone();
                            let back = navigate::selection_after_delete(
                                descends,
                                Some(&root),
                                current.path(),
                            );
                            // Cancelling the empty document's root
                            // pending deselects — reselecting it
                            // would pend again.
                            self.model.selection = (!(back.is_empty()
                                && self.model.doc.root.is_none()))
                            .then(|| {
                                selection::Selection::edge(&self.sources(), back).with_root(root)
                            });
                            true
                        }
                        Some(current) if current.stage() == selection::Stage::Label => {
                            let root = current.root().clone();
                            self.model.selection = Some(
                                selection::Selection::edge(
                                    &self.sources(),
                                    current.path().to_vec(),
                                )
                                .with_root(root),
                            );
                            true
                        }
                        _ => false,
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
            Key::Named(NamedKey::ArrowUp) if modifiers::command(&event.modifiers) => Some(true),
            Key::Named(NamedKey::ArrowDown) if modifiers::command(&event.modifiers) => Some(false),
            _ => return false,
        };
        let Some(current) = &self.model.selection else {
            return false;
        };
        if current.stage() != selection::Stage::Edge {
            return false;
        }
        let path = current.path().to_vec();
        let root = current.root().clone();
        let sources = sources::Sources {
            doc: &self.model.doc,
            libraries: &self.stack.libraries,
        };
        let Some(view) = self.model.workspace.view_mut(&root) else {
            return false;
        };
        match set {
            None => selection::toggle_collapse(&sources, &mut view.annotations, &path),
            Some(closed) => selection::set_collapse(&sources, &mut view.annotations, &path, closed),
        }
    }
}
