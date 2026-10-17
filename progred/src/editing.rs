//! Document-facing widget operations, given the session a dispatch lends.

use crate::{projection, selection, sources::Sources, workspace::Root};
use gid::{Step, Value};
use puri::edit::EditCtx;

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
