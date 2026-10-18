//! Document-facing widget operations, given the session a dispatch lends.

use crate::display::widget::navigation::Geometry;
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

pub(crate) fn select(session: &mut Session<'_>, root: &Root, path: &[Step]) {
    let fresh = session.model.selection.as_ref().is_none_or(|current| {
        current.root() != root
            || current.stage(&session.sources()) == selection::Stage::Label
            || current.path() != path
    });
    if fresh {
        session.model.selection = Some(selection::Selection::edge(root, path.to_vec()));
    } else if let Some(line) = session
        .model
        .selection
        .as_mut()
        .and_then(selection::Selection::edit_mut)
    {
        line.cursor_to_end();
    }
}

pub(crate) fn annotate(
    session: &mut Session<'_>,
    root: &Root,
    path: &[Step],
    state: Value,
) -> bool {
    match session.model.workspace.view_mut(root) {
        Some(view) if view.annotations.at(path) != Some(&state) => {
            view.annotations.set(path, Some(state));
            true
        }
        _ => false,
    }
}

pub(crate) fn edit_line(
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
/// opens an undo step.
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
            update(&sources, editor.text(), current).filter(|next| current != Some(next))
        });
    if next.is_some_and(|next| {
        selection
            .location()
            .is_some_and(|location| crate::selection::set_value(doc, libraries, &location, next))
    }) {
        let first = !selection.recorded();
        selection.preserve_recorded(true);
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

/// What Enter would start from `selection`, leaving it as it is: a peer
/// beside it (a sibling element in a list, with Shift before it, or a new
/// field on the record holding it, the root taking one on itself), with
/// the command chord something within it (a field on a record or cell, an
/// element appended to a list, with Shift at the front), or with no
/// selection an empty document's root value. Labels author first, then
/// values; list elements are one-stage value pendings, the projection
/// minting the position.
pub(crate) fn enter_pending(
    session: &Session<'_>,
    selection: Option<&selection::Selection>,
    command: CommandModifier,
    event: &KeyboardEvent,
) -> Option<selection::Selection> {
    let sources = session.sources();
    let shift = event.modifiers.shift();
    let root = selection
        .map(|current| current.root().clone())
        .unwrap_or_else(|| session.model.workspace.document_root().clone());
    let scope = selection
        .map(|current| current.scope().clone())
        .unwrap_or_default();
    match selection {
        Some(current) if command.pressed(&event.modifiers) => {
            selection::pending_insert(&root, &scope.view(sources), current.path(), shift)
        }
        Some(current) => {
            selection::pending_enter(&root, &scope.view(sources), current.path(), shift)
        }
        None => selection::pending_root(&root, &sources),
    }
    .map(|mut pending| {
        pending.set_scope(scope);
        pending.with_origin(selection.map(|current| current.path().to_vec()))
    })
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
    let Some(current) = session
        .model
        .selection
        .as_ref()
        .filter(|current| current.root() == root && current.path().starts_with(reached))
    else {
        return false;
    };
    let sources = session.sources();
    if current.stage(&sources) != selection::Stage::Edge
        || current.value_edit().is_some() && !command.pressed(&event.modifiers)
    {
        return false;
    }
    let Some(pending) = enter_pending(session, Some(current), command, event) else {
        return false;
    };
    let lands = match pending.stage(&sources) {
        selection::Stage::Label => pending.path(),
        _ => pending
            .path()
            .split_last()
            .map_or(&[][..], |(_, parent)| parent),
    };
    if lands != container {
        return false;
    }
    session.model.selection = Some(pending);
    geometry.reveal_selection(session.model);
    true
}
