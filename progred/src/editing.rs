//! Document-facing widget operations. Handlers pass the live editor explicitly.

use crate::{Editor, gesture, projection, selection, sources, sources::Sources, workspace::Root};
use gid::{Path, Step, Value};
use puri::{Point, edit::EditCtx};

pub(crate) fn select(app: &mut Editor, root: &Root, path: &[Step]) {
    let fresh = app.model.selection.as_ref().is_none_or(|current| {
        current.root() != root
            || current.stage(&app.sources()) == selection::Stage::Label
            || current.path() != path
    });
    if fresh {
        app.model.selection = Some(selection::Selection::edge(root, path.to_vec()));
    } else if let Some(line) = app
        .model
        .selection
        .as_mut()
        .and_then(selection::Selection::edit_mut)
    {
        line.cursor_to_end();
    }
}

pub(crate) fn select_payload(app: &mut Editor, root: &Root, path: Path, payload: Value) {
    app.model.selection = Some(selection::Selection::from_payload(
        root,
        &app.sources(),
        path,
        payload,
    ));
}

pub(crate) fn annotate(app: &mut Editor, root: &Root, path: &[Step], state: Value) -> bool {
    match app.model.workspace.view_mut(root) {
        Some(view) if view.annotations.at(path) != Some(&state) => {
            view.annotations.set(path, Some(state));
            true
        }
        _ => false,
    }
}

pub(crate) fn insert(app: &mut Editor, root: &Root, path: &[Step]) {
    if let Some(pending) = selection::pending_after(root, &app.sources(), path) {
        app.model.selection = Some(pending);
    }
}

pub(crate) fn start_gesture(
    app: &mut Editor,
    root: Root,
    path: Path,
    continuation: Box<dyn crate::display::widget::gesture::Gesture<Editor>>,
    samples: &[Point],
) {
    app.gesture = Some(gesture::Active::new(root, path, continuation));
    app.advance_gesture(samples);
}

pub(crate) fn commit_value(app: &mut Editor, value: Value, on_commit: Option<Value>) {
    if app
        .model
        .selection
        .as_ref()
        .is_some_and(|s| s.stage(&app.sources()) == selection::Stage::Pending)
    {
        app.commit_completion(value, None, on_commit);
    }
}

pub(crate) fn commit_label(
    app: &mut Editor,
    label: gid::CellId,
    definition: Option<Value>,
    on_commit: Option<Value>,
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

pub(crate) fn completion_view(
    app: &mut Editor,
    root: &Root,
    scroll: f64,
    choice: usize,
    everything: bool,
) {
    let sources = Sources {
        doc: &app.model.doc,
        libraries: &app.stack.libraries,
    };
    if let Some(selection) = app.model.selection.as_mut()
        && selection.root() == root
        && selection.stage(&sources) != selection::Stage::Edge
    {
        selection.set_completion_view(scroll, choice, everything);
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
            selection.stage(&sources) != selection::Stage::Edge
                && selection::writable_at(&sources, selection.path())
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
    app: &mut Editor,
    root: &Root,
    path: &[gid::Step],
    line: &crate::display::LineEdit,
    operation: &puri::edit::EditOperation<'_>,
) -> bool {
    let before = app.model.snapshot();
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
    let selected = model.selection.as_mut().filter(|selected| {
        selected.root() == root
            && selected.path() == path
            && selected.stage(&sources) == selection::Stage::Edge
            && selection::writable_at(&sources, path)
    });
    let (handled, record) = match selected {
        Some(selected) => projection::line_control::edit(
            &mut model.doc,
            &stack.libraries,
            selected,
            line,
            |state| {
                operation(EditCtx {
                    state,
                    fonts: font_cx,
                    layouts: layout_cx,
                    clipboard: text_clipboard,
                })
            },
        ),
        None => (false, false),
    };
    if record {
        model.history.record(before);
        app.refresh_title();
    }
    handled
}

pub(crate) fn picking(event: &puri::handler::PointerButtonEvent) -> bool {
    crate::modifiers::pick(&event.state.modifiers)
}

pub(crate) fn primary_edit(event: &puri::handler::PointerButtonEvent) -> bool {
    puri::interact::is_primary_contact(event) && !picking(event)
}
