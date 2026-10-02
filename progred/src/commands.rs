//! Editor commands: insert, delete, clipboard, and collapse.

use crate::Editor;
#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
use crate::clipboard::CLIPBOARD_FORMAT;
use crate::modifiers;
use crate::navigate;
use crate::selection;
use crate::sources;
use gid::Value;
use puri::edit::TextClipboard;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

impl Editor {
    fn select_landmark_or_edge(
        &mut self,
        geometry: navigate::Geometry<'_>,
        root: &crate::workspace::Root,
        path: gid::Path,
    ) {
        if let Some(target) = geometry
            .descends
            .iter()
            .find(|target| target.root.as_ref() == Some(root) && target.path.as_ref() == path)
        {
            (target.select)(self, None);
        } else {
            let scope = self
                .model
                .selection
                .as_ref()
                .map(|selection| selection.scope().clone())
                .unwrap_or_default();
            self.model.selection = None;
            scope
                .open(crate::editing::Access::new(self))
                .select(root, &path);
        }
    }

    pub(crate) fn commit_completion(
        &mut self,
        value: Value,
        definition: Option<Value>,
        on_commit: Option<crate::site::Continuation>,
    ) -> bool {
        let Some((root, scope)) = self
            .model
            .selection
            .as_ref()
            .map(|selection| (selection.root().clone(), selection.scope().clone()))
        else {
            return false;
        };
        let before = self.model.snapshot();
        let model = &mut self.model;
        let libraries = &self.stack.libraries;
        let (Some(selection), Some(view)) =
            (model.selection.as_ref(), model.workspace.view_mut(&root))
        else {
            return false;
        };
        let Some(prepared) = crate::completion::prepare(
            &sources::Sources {
                doc: &model.doc,
                libraries,
            },
            selection,
            &view.annotations,
            value,
            definition,
            on_commit.as_ref(),
        ) else {
            return false;
        };
        let document_changed = prepared.document_changed;
        if document_changed {
            model.doc = prepared.document;
            model.history.record(before);
        }
        crate::site::install_scoped(
            prepared.effects,
            &sources::Sources {
                doc: &model.doc,
                libraries,
            },
            &root,
            scope,
            &prepared.path,
            &mut view.annotations,
            &mut model.selection,
        );
        if document_changed {
            self.refresh_title();
        }
        true
    }

    /// Backspace or Delete empties the selected edge in place — a focused
    /// atom editor claims the keys while it has text and declines on an
    /// empty buffer, so emptying a string then backspacing again empties
    /// its place. Pressing again on the hole removes it (see insert_key).
    pub(crate) fn delete_key(
        &mut self,
        geometry: navigate::Geometry<'_>,
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
    pub(crate) fn delete_selected_edge(&mut self, geometry: navigate::Geometry<'_>) -> bool {
        match &self.model.selection {
            // Only a real edge deletes; a pending's Backspace is its
            // cancel, handled by insert_key.
            Some(current) if current.stage(&self.sources()) == selection::Stage::Edge => {
                let root = current.root().clone();
                let path = current.path().to_vec();
                let scope = current.scope().clone();
                let Some(source_path) = current.source_path().map(|path| path.into_owned()) else {
                    return false;
                };
                // Backspacing through the value and once more to
                // delete the edge is one gesture: when this edge has
                // the open run, its frame (pre-run document, edge
                // intact) already covers the deletion.
                let covered = current.recorded();
                let before = self.model.snapshot();
                selection::delete_edge(&mut self.model.doc, &self.stack.libraries, &source_path)
                    && {
                        if !covered {
                            self.model.history.record(before);
                            self.refresh_title();
                        }
                        let mut hole = selection::pending_value(&root, path);
                        hole.set_scope(scope);
                        self.model.selection = Some(hole);
                        geometry.reveal_selection(self);
                        true
                    }
            }
            _ => false,
        }
    }

    /// Commits a pointed-at value into the open pending — the
    /// command-click gesture. A value-stage pending commits and
    /// selects the edge; a label stage advances to its value stage.
    /// False when nothing is pending — or when the picked value
    /// cannot label (a list, a record, a blob) at the label stage —
    /// so the click falls through rather than spending the pending.
    pub(crate) fn pick_identity(&mut self, id: Value) -> bool {
        let continuation = match self.model.selection.as_ref() {
            Some(current)
                if current.stage(&self.sources()) == selection::Stage::Label
                    && id.as_cell().is_some_and(|label| {
                        let path: Vec<_> = current
                            .path()
                            .iter()
                            .cloned()
                            .chain([gid::Step::Key(label)])
                            .collect();
                        current.scope().read(&self.sources(), &path).is_none()
                    }) =>
            {
                crate::libraries::selection::pending_at(&[])
            }
            _ => crate::libraries::selection::at(&[], crate::libraries::selection::edge()),
        };
        self.commit_completion(id, None, Some(continuation))
    }

    /// Structural paste, the shell's fallback after text editing.
    /// Copy and cut belong to the selected projected occurrence. Deliberately
    /// NOT menu items — native menu accelerators intercept ahead of key
    /// dispatch, which would take Cmd+C/V away from text editing.
    pub(crate) fn paste_key(&mut self, event: &KeyboardEvent) -> bool {
        if !event.state.is_down() || !self.command_modifier.pressed(&event.modifiers) {
            return false;
        }
        let Key::Character(c) = &event.key else {
            return false;
        };
        c.eq_ignore_ascii_case("v") && self.paste_clipboard()
    }

    /// Copies the supplied projected value — SHALLOW: a link is its identity
    /// alone, no cell values travel; the value carries its own inline
    /// structure.
    pub(crate) fn copy_value(&mut self, value: &Value) -> bool {
        let (text, structural) = selection::to_clipboard(value);
        #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
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
        #[cfg(any(test, target_arch = "wasm32"))]
        {
            self.text_clipboard.text = Some(text);
            self.text_clipboard.structure = structural.then(|| value.clone());
            true
        }
    }

    /// The private format's payload, when the clipboard carries one.
    pub(crate) fn clipboard_structure(&mut self) -> Option<Value> {
        #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
        {
            use clipboard_rs::Clipboard;
            let bytes = clipboard_rs::ClipboardContext::new()
                .ok()
                .and_then(|cb| cb.get_buffer(CLIPBOARD_FORMAT).ok())?;
            return selection::from_structure(&bytes);
        }
        #[cfg(any(test, target_arch = "wasm32"))]
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
        if !event.state.is_down() || !self.command_modifier.pressed(&event.modifiers) {
            return false;
        }
        if !matches!(&event.key, Key::Character(c) if c.to_lowercase().as_str() == "v") {
            return false;
        }
        if !matches!(
            &self.model.selection,
            Some(current) if current.stage(&self.sources()) != selection::Stage::Edge
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
        self.paste_value(value)
    }

    pub(crate) fn paste_value(&mut self, value: Value) -> bool {
        if self.pick_identity(value.clone()) {
            return true;
        }
        let Some(current) = &self.model.selection else {
            return false;
        };
        if current.stage(&self.sources()) != selection::Stage::Edge {
            return false;
        }
        let root = current.root().clone();
        let path = current.path().to_vec();
        let scope = current.scope().clone();
        // Idempotent pastes stay off the undo stack, as write_through
        // keeps no-op rewrites off it.
        if current.value(&self.sources()) == Some(&value) {
            return true;
        }
        if scope
            .open(crate::editing::Access::new(self))
            .replace(&path, value)
        {
            self.model.selection = None;
            scope
                .open(crate::editing::Access::new(self))
                .select(&root, &path);
            true
        } else {
            false
        }
    }

    /// Completion handlers own committing their query. Otherwise
    /// plain Enter is a new peer BESIDE the selection: a
    /// sibling element in a list (Shift+Enter before), a new field on
    /// the parent record otherwise; the root has nothing beside it
    /// and takes the field on itself. The command chord authors
    /// WITHIN the selection: a field on the selected cell, an
    /// appended element on a list (with Shift, at the front). Labels
    /// author first, then values; list elements are one-stage value
    /// pendings, the projection minting the position.
    /// On an empty document Enter begins the root value. Escape and an
    /// empty query's Backspace or Delete step back out of a picker.
    pub(crate) fn insert_key(
        &mut self,
        geometry: navigate::Geometry<'_>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && match &event.key {
                Key::Named(NamedKey::Enter) => match self.model.selection.take() {
                    Some(current) if current.stage(&self.sources()) != selection::Stage::Edge => {
                        self.model.selection = Some(current);
                        false
                    }
                    selection => {
                        let sources = self.sources();
                        let shift = event.modifiers.shift();
                        let root = selection
                            .as_ref()
                            .map(|current| current.root().clone())
                            .unwrap_or_else(|| self.model.workspace.document_root().clone());
                        let scope = selection
                            .as_ref()
                            .map(|s| s.scope().clone())
                            .unwrap_or_default();
                        let started = match selection.as_ref() {
                            Some(current) if self.command_modifier.pressed(&event.modifiers) => {
                                selection::pending_insert(
                                    &root,
                                    &scope.view(sources),
                                    current.path(),
                                    shift,
                                )
                            }
                            Some(current) => selection::pending_enter(
                                &root,
                                &scope.view(sources),
                                current.path(),
                                shift,
                            ),
                            None => selection::pending_root(&root, &sources),
                        }
                        .map(|mut pending| {
                            pending.set_scope(scope);
                            pending.with_origin(
                                selection.as_ref().map(|current| current.path().to_vec()),
                            )
                        });
                        let began = started.is_some();
                        self.model.selection = started.or(selection);
                        if began {
                            geometry.reveal_selection(self);
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
    fn escape(&mut self, geometry: navigate::Geometry<'_>) -> bool {
        let Some(current) = &self.model.selection else {
            return false;
        };
        if current.stage(&self.sources()) != selection::Stage::Edge && current.origin().is_some() {
            self.return_to_origin(geometry);
        } else {
            self.model.selection = None;
        }
        true
    }

    /// Leaves a picker for the selection it was opened from. A replacement's
    /// origin is its own place, where this frame draws the hole, so that
    /// value is selected afresh rather than through the frame.
    fn return_to_origin(&mut self, geometry: navigate::Geometry<'_>) {
        let Some(current) = self.model.selection.take() else {
            return;
        };
        let Some(origin) = current.origin().map(<[gid::Step]>::to_vec) else {
            return;
        };
        let root = current.root().clone();
        if origin == current.path() {
            current
                .scope()
                .clone()
                .open(crate::editing::Access::new(self))
                .select(&root, &origin);
        } else {
            self.model.selection = Some(current);
            self.select_landmark_or_edge(geometry, &root, origin);
        }
        geometry.reveal_selection(self);
    }

    /// An empty query's Backspace or Delete cancels its picker: back to
    /// where it was opened, else to the stop before the hole it leaves
    /// (after it, for Delete). A label goes back to its record.
    fn cancel_pending(&mut self, geometry: navigate::Geometry<'_>, forward: bool) -> bool {
        let Some(current) = &self.model.selection else {
            return false;
        };
        let stage = current.stage(&self.sources());
        if stage == selection::Stage::Edge {
            return false;
        }
        if current.origin().is_some() {
            self.return_to_origin(geometry);
            return true;
        }
        let root = current.root().clone();
        let back = match stage {
            selection::Stage::Label => current.path().to_vec(),
            _ => navigate::selection_after_removing(
                geometry.descends,
                Some(&root),
                current.path(),
                forward,
            ),
        };
        // Cancelling the empty document's root pending deselects —
        // reselecting it would pend again.
        if back.is_empty() && self.model.doc.root.is_none() {
            self.model.selection = None;
        } else {
            self.select_landmark_or_edge(geometry, &root, back);
        }
        geometry.reveal_selection(self);
        true
    }

    /// Typing over a selected value starts replacing it: a picker in its
    /// place, seeded with what was typed. The document keeps the value
    /// until a choice commits, so Escape brings it back.
    pub(crate) fn replace_key(
        &mut self,
        geometry: navigate::Geometry<'_>,
        event: &KeyboardEvent,
    ) -> bool {
        if !event.state.is_down()
            || event.modifiers.ctrl()
            || event.modifiers.meta()
            || event.modifiers.alt()
        {
            return false;
        }
        let Key::Character(typed) = &event.key else {
            return false;
        };
        let Some(current) = &self.model.selection else {
            return false;
        };
        let sources = self.sources();
        if typed.trim().is_empty()
            || current.stage(&sources) != selection::Stage::Edge
            || current.value(&sources).is_none()
            || !current.writable(&sources)
        {
            return false;
        }
        let path = current.path().to_vec();
        let mut replacing = selection::pending_with_query(current.root(), path.clone(), typed)
            .with_origin(Some(path));
        replacing.set_scope(current.scope().clone());
        self.model.selection = Some(replacing);
        geometry.reveal_selection(self);
        true
    }

    /// Fold state belongs to an occurrence. Projection supplies its default;
    /// no document lookup or writable source is needed to change it.
    pub(crate) fn set_collapsed(
        &mut self,
        root: &crate::workspace::Root,
        path: &[gid::Step],
        default: bool,
        closed: Option<bool>,
    ) -> bool {
        let changed = self.model.set_collapsed(root, path, default, closed);
        if changed {
            self.finish_gesture();
        }
        changed
    }
}
