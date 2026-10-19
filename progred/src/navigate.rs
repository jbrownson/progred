//! The editor's navigation keys: arrows, Tab and Select All, and where the
//! selection lands when what it was on goes away. They act through the
//! installed frame's [`Geometry`].

use crate::display::widget::navigation::{Geometry, Landmark, advance, landmark_at};
use crate::display::widget::view::{Root, Target};
use crate::selection::Selection;
use gid::{Path, Step};
use puri::handler::NavigationDirection;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

pub fn direction(event: &KeyboardEvent) -> Option<NavigationDirection> {
    match &event.key {
        Key::Named(NamedKey::ArrowLeft) => Some(NavigationDirection::Left),
        Key::Named(NamedKey::ArrowRight) => Some(NavigationDirection::Right),
        Key::Named(NamedKey::ArrowUp) => Some(NavigationDirection::Up),
        Key::Named(NamedKey::ArrowDown) => Some(NavigationDirection::Down),
        _ => None,
    }
    .filter(|_| event.state.is_down())
}

/// Where the selection lands after removing `path`. Moving forward, on the
/// stop after it: its next sibling, or, when it's the last of its siblings,
/// the one after its nearest ancestor that has one. Otherwise, and moving
/// backward, on the sibling before it, else its parent.
pub fn selection_after_removing<World>(
    landmarks: &[Landmark<World>],
    root: Option<&Root>,
    path: &[Step],
    forward: bool,
) -> Path {
    forward
        .then(|| following(landmarks, root, path))
        .flatten()
        .or_else(|| preceding(landmarks, root, path))
        .unwrap_or_else(|| {
            path.split_last()
                .map(|(_, parent)| parent.to_vec())
                .unwrap_or_default()
        })
}

/// Handle Select All after focused controls decline the raw key. Arrow routing
/// is supplied separately by the projection's ordinary navigation handlers.
pub(crate) fn keyboard(
    editor: &mut crate::Editor,
    geometry: Geometry<'_, crate::Editor>,
    event: &KeyboardEvent,
) -> bool {
    let root = editor
        .model
        .selection
        .as_ref()
        .map(Selection::root)
        .unwrap_or_else(|| editor.model.workspace.document_root());
    if let Some(target) = select_all(
        editor.command_modifier,
        geometry.landmarks,
        Some(root),
        event,
    ) {
        return geometry.arrive(editor, target, None);
    }
    false
}

/// Tab moves to the next place to fill in, in reading order, and Shift+Tab
/// to the one before, within the selection's view; with nothing selected,
/// Tab starts at the first. With none left, a browser moves focus on to the
/// rest of its page, which clears the selection. A window has nowhere else
/// to go, so there Tab clears the selection itself and the next starts over.
pub(crate) fn tab(
    editor: &mut crate::Editor,
    geometry: Geometry<'_, crate::Editor>,
    event: &KeyboardEvent,
) -> bool {
    if !event.state.is_down()
        || event.key != Key::Named(NamedKey::Tab)
        || event.modifiers.ctrl()
        || event.modifiers.meta()
        || event.modifiers.alt()
    {
        return false;
    }
    if advance(editor, geometry, event.modifiers.shift()) {
        return true;
    }
    !cfg!(target_arch = "wasm32") && editor.model.selection.take().is_some()
}

pub(crate) fn initial_navigation(
    root: &Root,
    landmarks: &[Landmark<crate::Editor>],
) -> Option<puri::handler::Handler<crate::Editor, crate::frame::DispatchContext<crate::Editor>>> {
    let target = root_target(landmarks, Some(root))?;
    let root = root.clone();
    let path = target.path.clone();
    let scope = target.scope.clone();
    Some(puri::handler::Handler::from_function(
        move |editor: &mut crate::Editor, event, _| {
            if matches!(event, puri::handler::Event::Navigate(_))
                && editor.model.selection.is_none()
            {
                scope.open(editor.session()).select(&root, &path);
                puri::handler::EventOutcome::accept()
            } else {
                puri::handler::EventOutcome::decline(event)
            }
        },
    ))
}

pub fn select_all<'a, World>(
    command: puri::keyboard::CommandModifier,
    landmarks: &'a [Landmark<World>],
    root: Option<&Root>,
    event: &KeyboardEvent,
) -> Option<&'a Landmark<World>> {
    (event.state.is_down()
        && command.pressed(&event.modifiers)
        && !(event.modifiers.shift() || event.modifiers.alt())
        && matches!(&event.key, Key::Character(key) if key.eq_ignore_ascii_case("a")))
    .then(|| root_target(landmarks, root))
    .flatten()
}

fn root_target<'a, World>(
    landmarks: &'a [Landmark<World>],
    root: Option<&Root>,
) -> Option<&'a Landmark<World>> {
    let path = match root.map(Root::target) {
        Some(Target::Pane { path }) => path.as_slice(),
        _ => &[],
    };
    landmark_at(landmarks, root, path)
}

/// The stop after the drawn `path`: its next sibling, else the one after its
/// nearest ancestor that has one.
fn following<World>(
    landmarks: &[Landmark<World>],
    root: Option<&Root>,
    path: &[Step],
) -> Option<Path> {
    let (parent, siblings, index) = siblings(landmarks, root, path)?;
    siblings
        .get(index + 1)
        .map(|next| next.to_vec())
        .or_else(|| following(landmarks, root, parent))
}

/// The sibling before the drawn `path`.
fn preceding<World>(
    landmarks: &[Landmark<World>],
    root: Option<&Root>,
    path: &[Step],
) -> Option<Path> {
    let (_, siblings, index) = siblings(landmarks, root, path)?;
    index
        .checked_sub(1)
        .and_then(|index| siblings.get(index))
        .map(|previous| previous.to_vec())
}

/// The drawn `path`'s parent, and its siblings in placement order, itself
/// among them at `index`.
fn siblings<'a, World>(
    landmarks: &'a [Landmark<World>],
    root: Option<&Root>,
    path: &'a [Step],
) -> Option<(&'a [Step], Vec<&'a [Step]>, usize)> {
    let (_, parent) = path.split_last()?;
    let siblings: Vec<&[Step]> = landmarks
        .iter()
        .filter(|landmark| root.is_none_or(|root| landmark.root.as_ref() == Some(root)))
        .map(|landmark| landmark.path.as_ref())
        .filter(|p| p.split_last().is_some_and(|(_, prefix)| prefix == parent))
        .collect();
    let index = siblings.iter().position(|p| *p == path)?;
    Some((parent, siblings, index))
}

#[cfg(test)]
mod tests;
