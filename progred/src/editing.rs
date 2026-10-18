//! Document-facing widget operations, given the session a dispatch lends.

use crate::{navigate::Geometry, projection, selection, sources::Sources, workspace::Root};
use gid::{Step, Value};
use puri::edit::EditCtx;
use puri::keyboard::CommandModifier;
use ui_events::keyboard::KeyboardEvent;

mod picking;
mod scope;
mod session;
mod site;
pub use scope::Read;
pub(crate) use scope::{Access, Edit, Scope};
pub(crate) use session::Session;
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
        Some(selected) => {
            projection::line_control::edit(&mut model.doc, libraries, selected, line, |state| {
                operation(EditCtx {
                    state,
                    fonts: text.fonts,
                    layouts: text.layouts,
                    clipboard: text.clipboard,
                })
            })
        }
        None => (false, false),
    };
    if record {
        model.history.record(before);
    }
    handled
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
