//! Document-facing widget operations, given the session a dispatch lends or,
//! where they haven't moved onto it, the live editor.

use crate::{Editor, projection, selection, sources, sources::Sources, workspace::Root};
use gid::{Path, Step, Value};
use puri::{Point, edit::EditCtx};

mod scope;
mod session;
pub use scope::Read;
pub(crate) use scope::{Access, Scope};
pub(crate) use session::Session;

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

pub(crate) fn start_gesture(
    app: &mut Editor,
    continuation: Box<dyn crate::display::widget::gesture::Gesture<Editor>>,
    samples: &[Point],
) {
    app.finish_gesture();
    app.gesture = Some(continuation);
    app.advance_gesture(samples);
}

pub(crate) fn commit_value(
    app: &mut Editor,
    value: Value,
    on_commit: Option<crate::site::Continuation>,
) {
    if app
        .model
        .selection
        .as_ref()
        .is_some_and(|s| s.stage(&app.sources()) == selection::Stage::Pending)
    {
        app.commit_completion(value, None, on_commit);
    }
}

/// Commits an empty cell, list or record into the pending, then opens it,
/// so what's typed next goes inside it.
pub(crate) fn commit_container(app: &mut Editor, value: Value) {
    if app
        .model
        .selection
        .as_ref()
        .is_some_and(|s| s.stage(&app.sources()) == selection::Stage::Pending)
        && app.commit_completion(
            value,
            None,
            Some(crate::selection::at(&[], crate::selection::payload::edge())),
        )
    {
        app.open_within();
    }
}

/// Commits a reference to a new cell, defined as given, into the pending.
pub(crate) fn commit_new_cell(
    app: &mut Editor,
    definition: Value,
    on_commit: Option<crate::site::Continuation>,
) {
    if app
        .model
        .selection
        .as_ref()
        .is_some_and(|s| s.stage(&app.sources()) == selection::Stage::Pending)
    {
        app.commit_completion(gid::new_cell_id().into(), Some(definition), on_commit);
    }
}

pub(crate) fn commit_label(
    app: &mut Editor,
    label: gid::CellId,
    definition: Option<Value>,
    on_commit: Option<crate::site::Continuation>,
) {
    if app
        .model
        .selection
        .as_ref()
        .is_some_and(|s| s.stage(&app.sources()) == selection::Stage::Label)
    {
        app.commit_completion(label.into(), definition, on_commit);
    }
}

pub(crate) fn completion_view(app: &mut Editor, root: &Root, scroll: f64, choice: usize) {
    let sources = Sources {
        doc: &app.model.doc,
        libraries: &app.stack.libraries,
    };
    if let Some(selection) = app.model.selection.as_mut()
        && selection.root() == root
        && selection.stage(&sources) != selection::Stage::Edge
    {
        selection.set_completion_view(scroll, choice);
    }
}

/// A retained handler can outlive the selection's editor; missing state declines.
pub(crate) fn edit_query(app: &mut Editor, operation: &puri::edit::EditOperation<'_>) -> bool {
    let Editor {
        model,
        stack,
        font_cx,
        layout_cx,
        text_clipboard,
        ..
    } = app;
    let sources = sources::Sources {
        doc: &model.doc,
        libraries: &stack.libraries,
    };
    model
        .selection
        .as_mut()
        .filter(|selection| {
            selection.stage(&sources) != selection::Stage::Edge && selection.writable(&sources)
        })
        .is_some_and(|selection| {
            selection.edit_query(|state| {
                operation(EditCtx {
                    state,
                    fonts: font_cx,
                    layouts: layout_cx,
                    clipboard: text_clipboard,
                })
            })
        })
}

pub(crate) fn edit_line(
    session: &mut Session<'_>,
    root: &Root,
    path: &[gid::Step],
    line: &crate::display::LineEdit,
    operation: &puri::edit::EditOperation<'_>,
) -> bool {
    let before = session.model.snapshot();
    let Session {
        model,
        libraries,
        text,
    } = session;
    let sources = sources::Sources {
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
