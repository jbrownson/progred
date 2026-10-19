//! The editor's editing keys, and what they do: insert, delete, replace,
//! and the clipboard.

use crate::Editor;
use crate::display::widget::navigation::{Geometry, landmark_at};
use crate::display::widget::view::Root;
use crate::location::Location;
use crate::modifiers;
use crate::navigate;
use crate::selection::{self, Selection};
use gid::{Path, Step, Value};
use puri::edit::TextClipboard;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

impl Editor {
    /// Select what this frame draws at `path` in `root`'s view, else the
    /// edge there, read through `scope`.
    fn select_landmark_or_edge(
        &mut self,
        geometry: Geometry<'_, Editor>,
        root: &Root,
        path: Path,
        scope: crate::editing::Scope,
    ) {
        match landmark_at(geometry.landmarks, Some(root), &path) {
            Some(target) => {
                (target.select)(self, None);
            }
            None => self.model.selection = Some(Selection::edge(root, path).with_scope(scope)),
        }
    }

    /// Backspace or Delete empties the selected edge in place — a focused
    /// atom editor claims the keys while it has text and declines on an
    /// empty buffer, so emptying a string then backspacing again empties
    /// its place. Pressing again on the hole removes it (see insert_key).
    pub(crate) fn delete_key(
        &mut self,
        geometry: Geometry<'_, Editor>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && modifiers::plain(&event.modifiers)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && self.delete_selected_edge(geometry)
    }

    /// Deletes the selected edge and keeps its place selected as a hole,
    /// ready for a replacement — Backspace/Delete's action, and cut's
    /// second half.
    pub(crate) fn delete_selected_edge(&mut self, geometry: Geometry<'_, Editor>) -> bool {
        match &self.model.selection {
            // Only a real edge deletes; a pending's Backspace is its
            // cancel, handled by insert_key.
            Some(current) if current.stage(&self.sources()) == selection::Stage::Edge => {
                let root = current.root().clone();
                let path = current.path().to_vec();
                let scope = current.scope().clone();
                // Backspacing through the value and once more to
                // delete the edge is one gesture: when this edge has
                // the open run, its frame (pre-run document, edge
                // intact) already covers the deletion.
                let covered = current.recorded();
                current
                    .location()
                    .map(Location::into_owned)
                    .is_some_and(|location| {
                        let before = self.model.snapshot();
                        crate::location::delete_edge(
                            &mut self.model.doc,
                            &self.stack.libraries,
                            &location,
                        ) && {
                            if !covered {
                                self.model.history.record(before);
                                self.refresh_title();
                            }
                            self.model.selection =
                                Some(selection::pending_value(&root, path).with_scope(scope));
                            geometry.reveal_selection(&mut self.model);
                            true
                        }
                    })
            }
            _ => false,
        }
    }

    /// Structural paste, the shell's fallback after text editing.
    /// Copy and cut belong to the selected projected occurrence. Deliberately
    /// NOT menu items — native menu accelerators intercept ahead of key
    /// dispatch, which would take Cmd+C/V away from text editing.
    pub(crate) fn paste_key(&mut self, event: &KeyboardEvent) -> bool {
        modifiers::chord(self.command_modifier, event, "v") && self.paste_clipboard()
    }

    /// Copies the supplied projected value — SHALLOW: a link is its identity
    /// alone, no cell values travel; the value carries its own inline
    /// structure.
    pub(crate) fn copy_value(&mut self, value: &Value) -> bool {
        let (text, structural) = crate::clipboard::to_clipboard(value);
        self.text_clipboard.copy(text, structural.then_some(value))
    }

    /// Cmd+V while a pending is open and the clipboard CARRIES
    /// STRUCTURE — the private format, not a text shape — commits the
    /// value into the pending, ahead of the focused query's own text
    /// paste. Everything else keeps the text path: pasting "hi",
    /// 0xff, or even text that happens to spell Value JSON lands in
    /// the query as characters. Claims the chord even when the pick
    /// declines (the label stage takes only what can label).
    pub(crate) fn pending_paste_key(&mut self, event: &KeyboardEvent) -> bool {
        modifiers::chord(self.command_modifier, event, "v")
            && matches!(
                &self.model.selection,
                Some(current) if current.stage(&self.sources()) != selection::Stage::Edge
            )
            && self.text_clipboard.structure().is_some_and(|value| {
                if let Some(picking) = self.session().pending() {
                    picking.pick(value);
                }
                true
            })
    }

    /// Pastes the clipboard's value — the private format's structure
    /// when it carries one, else the text's query reading: into an
    /// open pending first (the label stage narrows to atoms through
    /// the pick), else over the selected edge — one undo step,
    /// retaining ordinary structural selection at the changed site.
    pub(crate) fn paste_clipboard(&mut self) -> bool {
        self.text_clipboard
            .structure()
            .or_else(|| {
                self.text_clipboard
                    .get_text()
                    .filter(|text| !text.is_empty())
                    .map(|text| selection::resolve_query(&text))
            })
            .is_some_and(|value| self.paste_value(value))
    }

    pub(crate) fn paste_value(&mut self, value: Value) -> bool {
        self.session()
            .pending()
            .is_some_and(|picking| picking.pick(value.clone()))
            || match &self.model.selection {
                Some(current) if current.stage(&self.sources()) == selection::Stage::Edge => {
                    // Pasting what's there already stays off the undo stack, as
                    // every write that changes nothing does.
                    current.value(&self.sources()) == Some(&value) || {
                        let root = current.root().clone();
                        let path = current.path().to_vec();
                        let scope = current.scope().clone();
                        scope.open(self.session()).replace(&path, value) && {
                            self.model.selection =
                                Some(Selection::edge(&root, path).with_scope(scope));
                            true
                        }
                    }
                }
                _ => false,
            }
    }

    /// Completion handlers own committing their query, and Enter while
    /// typing in a value only ends the typing. What Enter authors beside or
    /// within a selected value belongs to the projection of the container
    /// it lands in, through [`crate::editing::author_in`]; on an empty document Enter
    /// begins the root value. Escape and an empty query's Backspace or Delete
    /// step back out of a picker.
    pub(crate) fn insert_key(
        &mut self,
        geometry: Geometry<'_, Editor>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && match &event.key {
                Key::Named(NamedKey::Enter) => match self.model.selection.take() {
                    Some(current) if current.stage(&self.sources()) != selection::Stage::Edge => {
                        self.model.selection = Some(current);
                        false
                    }
                    // Typing has already written the value, so Enter only ends
                    // the typing, and the text re-spells from what was written.
                    Some(mut current)
                        if current.value_edit().is_some()
                            && !self.command_modifier.pressed(&event.modifiers) =>
                    {
                        current.clear_editor();
                        self.model.selection = Some(current);
                        true
                    }
                    Some(current) => {
                        self.model.selection = Some(current);
                        false
                    }
                    None => {
                        let within = self.command_modifier.pressed(&event.modifiers);
                        let started = crate::editing::enter_pending(
                            &self.session(),
                            within,
                            event.modifiers.shift(),
                        );
                        let began = started.is_some();
                        self.model.selection = started;
                        if began {
                            geometry.reveal_selection(&mut self.model);
                        }
                        began
                    }
                },
                Key::Named(NamedKey::Escape) => self.escape(geometry),
                Key::Named(key @ (NamedKey::Backspace | NamedKey::Delete)) => {
                    self.cancel_pending(geometry, *key == NamedKey::Delete)
                }
                _ => false,
            }
    }

    /// Escape steps back one level: a picker opened from a key returns to
    /// where it was opened, so a replacement gets its value back; anything
    /// else clears the selection.
    fn escape(&mut self, geometry: Geometry<'_, Editor>) -> bool {
        match self.model.selection.take() {
            Some(current) => {
                if current.stage(&self.sources()) != selection::Stage::Edge
                    && let Some(origin) = current.origin().map(<[Step]>::to_vec)
                {
                    self.return_to_origin(geometry, current, origin);
                }
                true
            }
            None => false,
        }
    }

    /// Leaves `current`, a picker, for the selection it was opened from at
    /// `origin`. A replacement's origin is its own place, where this frame
    /// draws the hole, so that value is selected afresh rather than through
    /// the frame.
    fn return_to_origin(
        &mut self,
        geometry: Geometry<'_, Editor>,
        current: Selection,
        origin: Path,
    ) {
        let scope = current.scope().clone();
        if origin == current.path() {
            scope.open(self.session()).select(current.root(), &origin);
        } else {
            self.select_landmark_or_edge(geometry, current.root(), origin, scope);
        }
        geometry.reveal_selection(&mut self.model);
    }

    /// An empty query's Backspace or Delete cancels its picker: back to
    /// where it was opened, else beside the hole it leaves, as
    /// [`navigate::selection_after_removing`] chooses: Backspace on the
    /// sibling before it or else its container, Delete on the stop after
    /// it. A label goes back to its record.
    fn cancel_pending(&mut self, geometry: Geometry<'_, Editor>, forward: bool) -> bool {
        let sources = crate::sources::Sources {
            doc: &self.model.doc,
            libraries: &self.stack.libraries,
        };
        let picker = self
            .model
            .selection
            .take_if(|current| current.stage(&sources) != selection::Stage::Edge);
        match picker {
            Some(current) => {
                match current.origin().map(<[Step]>::to_vec) {
                    Some(origin) => self.return_to_origin(geometry, current, origin),
                    None => {
                        let back = match current.stage(&self.sources()) {
                            selection::Stage::Label => current.path().to_vec(),
                            _ => navigate::selection_after_removing(
                                geometry.landmarks,
                                Some(current.root()),
                                current.path(),
                                forward,
                            ),
                        };
                        // Cancelling the empty document's root pending
                        // deselects: reselecting it would pend again.
                        if !back.is_empty() || self.model.doc.root.is_some() {
                            self.select_landmark_or_edge(
                                geometry,
                                current.root(),
                                back,
                                current.scope().clone(),
                            );
                        }
                        geometry.reveal_selection(&mut self.model);
                    }
                }
                true
            }
            None => false,
        }
    }

    /// Typing over a selected value starts replacing it: a picker in its
    /// place, seeded with what was typed. The document keeps the value
    /// until a choice commits, so Escape brings it back.
    pub(crate) fn replace_key(
        &mut self,
        geometry: Geometry<'_, Editor>,
        event: &KeyboardEvent,
    ) -> bool {
        let sources = self.sources();
        let replacing = match (&event.key, &self.model.selection) {
            (Key::Character(typed), Some(current))
                if event.state.is_down()
                    && !(event.modifiers.ctrl()
                        || event.modifiers.meta()
                        || event.modifiers.alt())
                    && !typed.trim().is_empty()
                    && current.stage(&sources) == selection::Stage::Edge
                    && current.value(&sources).is_some()
                    && current.writable(&sources) =>
            {
                Some(
                    selection::pending_with_query(current.root(), current.path().to_vec(), typed)
                        .with_origin(Some(current.path().to_vec()))
                        .with_scope(current.scope().clone()),
                )
            }
            _ => None,
        };
        replacing.is_some_and(|replacing| {
            self.model.selection = Some(replacing);
            geometry.reveal_selection(&mut self.model);
            true
        })
    }
}

#[cfg(test)]
mod tests;
