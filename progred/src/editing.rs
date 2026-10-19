//! Document-facing widget operations, given the session a dispatch lends.

use crate::display::widget::navigation::Geometry;
use crate::location::Location;
use crate::{display::widget::view::Root, selection, sources::Sources};
use gid::{Step, Value};
use puri::edit::EditCtx;
use puri::keyboard::CommandModifier;
use ui_events::keyboard::KeyboardEvent;

mod picking;
mod scope;
mod session;
mod site;
pub use scope::Read;
pub(crate) use scope::{Edit, Scope};
pub(crate) use session::{Session, TextTools};
pub(crate) use site::{Selected, Site};

/// Writes `value` at `location` as an undo step of its own; whether the
/// document changed.
pub(crate) fn write(session: &mut Session<'_>, location: &Location, value: Value) -> bool {
    let before = session.model.snapshot();
    if crate::location::set_value(&mut session.model.doc, session.libraries, location, value) {
        session.model.history.record(before);
        true
    } else {
        false
    }
}

fn edit_line(
    session: &mut Session<'_>,
    root: &Root,
    path: &[Step],
    line: &crate::display::LineEdit,
    operation: &puri::edit::EditOperation<'_>,
) -> bool {
    let before = session.model.snapshot();
    let Session {
        model,
        libraries,
        text,
        ..
    } = session;
    let sources = Sources {
        doc: &model.doc,
        libraries,
    };
    let selected = model.selection.as_mut().filter(|selected| {
        selected.root() == root
            && selected.path() == path
            && selected.stage(&sources) == selection::Stage::Edge
            && selected.writable(&sources)
    });
    let (handled, record) = match selected {
        Some(selected) => line_edit(&mut model.doc, libraries, selected, line, |state| {
            operation(EditCtx {
                state,
                fonts: text.fonts,
                layouts: text.layouts,
                clipboard: text.clipboard,
            })
        }),
        None => (false, false),
    };
    if record {
        model.history.record(before);
    }
    handled
}

/// Apply this line's conversion of what was typed, and report whether it
/// opens an undo step. It writes apart from [`write`] because its caller
/// takes the undo snapshot before the keystroke changes the line, and the
/// edit run's flag lives on the selection.
pub(crate) fn commit_line(
    doc: &mut std::rc::Rc<gid::Document>,
    libraries: &crate::library::Libraries,
    selection: &mut selection::Selection,
    update: &crate::display::LineUpdate,
) -> bool {
    let sources = Sources { doc, libraries };
    let next = selection
        .value_edit()
        .filter(|_| selection.writable(&sources))
        .and_then(|editor| {
            let current = selection.value(&sources);
            update(editor.text(), current).filter(|next| current != Some(next))
        });
    if next.is_some_and(|next| {
        selection
            .location()
            .is_some_and(|location| crate::location::set_value(doc, libraries, &location, next))
    }) {
        let first = !selection.recorded();
        selection.set_recorded(true);
        first
    } else {
        false
    }
}

/// Apply `operation` to the line being typed in `selection`, committing what
/// it typed; whether the line took it, and whether that opens an undo step.
fn line_edit(
    doc: &mut std::rc::Rc<gid::Document>,
    libraries: &crate::library::Libraries,
    selection: &mut selection::Selection,
    line: &crate::display::LineEdit,
    operation: impl FnOnce(&mut puri::edit::LineEditState) -> bool,
) -> (bool, bool) {
    let typing = selection.edit().is_some();
    let state = selection.edit_line_mut(&line.text);
    let before = state.text().to_owned();
    let handled = operation(state);
    let changed = before != state.text();
    // A key the line didn't take doesn't start typing.
    if !handled && !typing {
        selection.clear_editor();
    }
    let record = handled && changed && commit_line(doc, libraries, selection, &line.update);
    (handled, record)
}

/// What Enter would start from the selection, leaving it as it is: a peer
/// beside it (a sibling element in a list, with `shift` before it, or a new
/// field on the record holding it, the root taking one on itself), or
/// `within` it, as the command chord asks, something inside it (a field on
/// a record or cell, an element appended to a list, with `shift` at the
/// front); with no selection, an empty document's root value. Labels author
/// first, then values; list elements are one-stage value pendings, the
/// projection minting the position.
pub(crate) fn enter_pending(
    session: &Session<'_>,
    within: bool,
    shift: bool,
) -> Option<selection::Selection> {
    let sources = session.sources();
    match &session.model.selection {
        Some(current) => {
            let read = current.scope().view(sources);
            if within {
                selection::pending_insert(current.root(), &read, current.path(), shift)
            } else {
                selection::pending_enter(current.root(), &read, current.path(), shift)
            }
            .map(|pending| pending.with_origin(current.path().to_vec()))
        }
        None => selection::pending_root(session.model.workspace.document_root(), &sources),
    }
}

/// Starts what Enter would author from the selected value in the view
/// `root`, if it lands in the container drawn at `container`: a field on
/// that record, or an element of that list or cell. Containers offer this
/// through [`crate::display::structure::authoring`], children first, so a
/// projection without it, such as a number's, takes nothing in.
pub(crate) fn author_in<W>(
    session: &mut Session<'_>,
    root: &Root,
    container: &[Step],
    command: CommandModifier,
    geometry: Geometry<'_, W>,
    event: &KeyboardEvent,
) -> bool {
    // What lands in a container is on the way to the selection, or the
    // value a cell there refers to: a cheap test before the full one.
    let reached = container
        .iter()
        .rposition(|step| !matches!(step, Step::Follow(_)))
        .map_or(&[][..], |last| &container[..=last]);
    let within = command.pressed(&event.modifiers);
    let sources = session.sources();
    let pending = session
        .model
        .selection
        .as_ref()
        .filter(|current| {
            current.root() == root
                && current.path().starts_with(reached)
                && current.stage(&sources) == selection::Stage::Edge
                && (current.value_edit().is_none() || within)
        })
        .and_then(|_| enter_pending(session, within, event.modifiers.shift()))
        .filter(|pending| {
            let lands = match pending.stage(&sources) {
                selection::Stage::Label => pending.path(),
                _ => pending
                    .path()
                    .split_last()
                    .map_or(&[][..], |(_, parent)| parent),
            };
            lands == container
        });
    match pending {
        Some(pending) => {
            session.model.selection = Some(pending);
            geometry.reveal_selection(session.model);
            true
        }
        None => false,
    }
}
