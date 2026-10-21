//! The open picker: the pending a hole's picker is drawn for, which its
//! offers commit into and its query and card edit.

use super::{Session, Site};
use crate::selection::{self, Stage};
use crate::site::Continuation;
use crate::sources::Sources;
use gid::{CellId, Value};
use puri::edit::{EditCtx, EditOperation};

/// A pending value or new field's label, which a dispatch's session lends.
/// Committing closes it, so the commits consume it; each declines, changing
/// nothing, where its stage doesn't fit or a step declines.
pub(crate) struct Picking<'s, 'a> {
    session: &'s mut Session<'a>,
    stage: Stage,
}

impl<'a> Session<'a> {
    /// The open picker, if a value or a new field's label is pending.
    pub(crate) fn pending(&mut self) -> Option<Picking<'_, 'a>> {
        let stage = self.model.selection.as_ref()?.stage(&self.sources());
        (stage != Stage::Edge).then_some(Picking {
            session: self,
            stage,
        })
    }
}

impl Picking<'_, '_> {
    /// Whether a value or a new field's label is pending.
    pub(crate) fn stage(&self) -> Stage {
        self.stage
    }

    /// Whether this is `site`'s picker. Handlers a picker leaves behind can
    /// outlive it, and check before they act.
    pub(crate) fn is_at(&self, site: &Site) -> bool {
        self.session
            .model
            .selection
            .as_ref()
            .is_some_and(|selection| site.holds(selection))
    }

    /// What a commit replaces, if anything.
    pub(crate) fn value(&self) -> Option<&Value> {
        self.session
            .model
            .selection
            .as_ref()?
            .value(&self.session.sources())
    }

    /// How far the card is scrolled.
    pub(crate) fn scroll(&self) -> f64 {
        self.session
            .model
            .selection
            .as_ref()
            .map_or(0.0, selection::Selection::completion_scroll)
    }

    /// Commits `value` into a value's pending, then interprets `on_commit`
    /// against the result.
    pub(crate) fn commit(self, value: Value, on_commit: Option<Continuation>) -> bool {
        self.stage == Stage::Pending
            && crate::completion::commit(self.session, value, None, on_commit)
    }

    /// Commits a reference to a new cell, defined as given.
    pub(crate) fn commit_new_cell(
        self,
        definition: Value,
        on_commit: Option<Continuation>,
    ) -> bool {
        self.stage == Stage::Pending
            && crate::completion::commit(
                self.session,
                gid::new_cell_id().into(),
                Some(definition),
                on_commit,
            )
    }

    /// Commits `label` as the new field's, defining it as given.
    pub(crate) fn commit_label(
        self,
        label: CellId,
        definition: Option<Value>,
        on_commit: Option<Continuation>,
    ) -> bool {
        self.stage == Stage::Label
            && crate::completion::commit(self.session, label.into(), definition, on_commit)
    }

    /// Commits an empty cell, list or record, then opens it, so what's
    /// typed next goes inside it.
    pub(crate) fn commit_container(self, value: Value) -> bool {
        let committed = self.stage == Stage::Pending
            && crate::completion::commit(
                self.session,
                value,
                None,
                Some(selection::at(&[], selection::payload::edge())),
            );
        if committed {
            open_within(self.session);
        }
        committed
    }

    /// Commits a picked identity, as Cmd+click and structural paste do. A
    /// label that names a field not yet there leaves its value pending;
    /// anything else is selected.
    pub(crate) fn pick(self, id: Value) -> bool {
        let session = self.session;
        let continuation = match session.model.selection.as_ref() {
            Some(current)
                if self.stage == Stage::Label
                    && id.as_cell().is_some_and(|label| {
                        let path =
                            crate::occurrence::below(current.path(), &[gid::Step::Key(label)]);
                        crate::occurrence::read(&session.sources(), &path).is_none()
                    }) =>
            {
                selection::pending_at(&[])
            }
            _ => selection::at(&[], selection::payload::edge()),
        };
        crate::completion::commit(session, id, None, Some(continuation))
    }

    /// Hands the pending's place to a projection's own picker, as the new
    /// color offer opens color's: selects it with `payload` instead.
    pub(crate) fn select_with(self, payload: Value) -> bool {
        let Session {
            model, libraries, ..
        } = self.session;
        let sources = Sources {
            doc: &model.doc,
            libraries,
        };
        match model
            .selection
            .as_ref()
            .filter(|current| current.writable(&sources))
        {
            Some(current) => {
                model.selection = Some(selection::Selection::from_payload(
                    &sources,
                    current.path().to_vec(),
                    payload,
                ));
                true
            }
            None => false,
        }
    }

    /// Edits the query, where the pending's place is writable.
    pub(crate) fn edit_query(&mut self, operation: &EditOperation<'_>) -> bool {
        let Session {
            model,
            libraries,
            text,
            ..
        } = &mut *self.session;
        let sources = Sources {
            doc: &model.doc,
            libraries,
        };
        model
            .selection
            .as_mut()
            .filter(|selection| selection.writable(&sources))
            .is_some_and(|selection| {
                selection.edit_query(|state| {
                    operation(EditCtx {
                        state,
                        fonts: text.fonts,
                        layouts: text.layouts,
                        clipboard: text.clipboard,
                    })
                })
            })
    }

    /// Types into a value's pending as into the line its value will have:
    /// the query takes `operation`, and once that changes what's typed,
    /// `update`'s reading of it is written in the pending's place, which
    /// stays selected, typing going on in the value's own line. Whether the
    /// query took the operation.
    pub(crate) fn type_line(
        self,
        operation: &EditOperation<'_>,
        update: &crate::display::LineUpdate,
    ) -> bool {
        let before = self.session.model.snapshot();
        let Session {
            model,
            libraries,
            text,
            ..
        } = self.session;
        let sources = Sources {
            doc: &model.doc,
            libraries,
        };
        let typed = model
            .selection
            .as_mut()
            .filter(|selection| self.stage == Stage::Pending && selection.writable(&sources))
            .map(|selection| {
                let was = selection
                    .edit()
                    .map_or_else(String::new, |line| line.text().to_owned());
                let handled = selection.edit_query(|state| {
                    operation(EditCtx {
                        state,
                        fonts: text.fonts,
                        layouts: text.layouts,
                        clipboard: text.clipboard,
                    })
                });
                let line = selection.edit().map_or("", |line| line.text());
                let written = (line != was)
                    .then(|| update(line, None))
                    .flatten()
                    .zip(selection.location());
                (handled, written)
            });
        match typed {
            Some((handled, Some((value, location)))) => {
                if crate::location::set_value(&mut model.doc, libraries, &location, value) {
                    model.history.record(before);
                    if let Some(selection) = model.selection.as_mut() {
                        selection.typing_on();
                    }
                }
                handled
            }
            Some((handled, None)) => handled,
            None => false,
        }
    }

    /// Scrolls the card and chooses its offer at `choice`.
    pub(crate) fn set_view(&mut self, scroll: f64, choice: usize) {
        if let Some(selection) = self.session.model.selection.as_mut() {
            selection.set_completion_view(scroll, choice);
        }
    }
}

/// Opens the selected container for what goes inside it, as Cmd+Enter does.
fn open_within(session: &mut Session<'_>) {
    if let Some(within) = super::enter_pending(session, true, false) {
        session.model.selection = Some(within);
    }
}
