//! The editor's navigation keys: arrows, Tab and Select All, and where the
//! selection lands when what it was on goes away. They act through the
//! installed frame's [`Geometry`].

use crate::display::widget::navigation::{Geometry, Landmark, advance};
use crate::display::widget::view::Root;
use crate::occurrence::{Path, Step};
use crate::selection::Selection;
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
    root: &Root,
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

/// Select All, once focused controls decline the raw key, selects the root
/// of the selection's view, else the document's.
pub(crate) fn select_all_key(
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
    select_all(editor.command_modifier, geometry.landmarks, root, event)
        .is_some_and(|target| geometry.arrive(editor, target, None))
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
    event.state.is_down()
        && event.key == Key::Named(NamedKey::Tab)
        && crate::modifiers::at_most_shift(&event.modifiers)
        && (advance(editor, geometry, event.modifiers.shift())
            || !cfg!(target_arch = "wasm32") && editor.model.selection.take().is_some())
}

pub(crate) fn initial_navigation(
    root: &Root,
    landmarks: &[Landmark<crate::Editor>],
) -> Option<puri::handler::Handler<crate::Editor, crate::frame::DispatchContext<crate::Editor>>> {
    let target = root_target(landmarks, root)?;
    let path = target.path.clone();
    Some(puri::handler::Handler::from_function(
        move |editor: &mut crate::Editor, event, _| {
            if matches!(event, puri::handler::Event::Navigate(_))
                && editor.model.selection.is_none()
            {
                crate::editing::Edit::new(editor.session()).select(&path);
                puri::handler::EventOutcome::accept()
            } else {
                puri::handler::EventOutcome::decline(event)
            }
        },
    ))
}

/// The root of `root`'s view, when `event` is Select All.
pub fn select_all<'a, World>(
    command: puri::keyboard::CommandModifier,
    landmarks: &'a [Landmark<World>],
    root: &Root,
    event: &KeyboardEvent,
) -> Option<&'a Landmark<World>> {
    (crate::modifiers::chord(command, event, "a")
        && !(event.modifiers.shift() || event.modifiers.alt()))
    .then(|| root_target(landmarks, root))
    .flatten()
}

/// What's drawn at the root of `root`'s view: its outermost value, such as a
/// workspace's main value, or the place a pane shows.
fn root_target<'a, World>(
    landmarks: &'a [Landmark<World>],
    root: &Root,
) -> Option<&'a Landmark<World>> {
    landmarks
        .iter()
        .filter(|landmark| landmark.root.as_ref() == Some(root))
        .min_by_key(|landmark| landmark.path.len())
}

/// The stop after the drawn `path`: its next sibling, else the one after its
/// nearest ancestor that has one.
fn following<World>(landmarks: &[Landmark<World>], root: &Root, path: &[Step]) -> Option<Path> {
    let (parent, siblings, index) = siblings(landmarks, root, path)?;
    siblings
        .get(index + 1)
        .map(|next| next.to_vec())
        .or_else(|| following(landmarks, root, parent))
}

/// The sibling before the drawn `path`.
fn preceding<World>(landmarks: &[Landmark<World>], root: &Root, path: &[Step]) -> Option<Path> {
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
    root: &Root,
    path: &'a [Step],
) -> Option<(&'a [Step], Vec<&'a [Step]>, usize)> {
    let (_, parent) = path.split_last()?;
    let siblings: Vec<&[Step]> = landmarks
        .iter()
        .filter(|landmark| landmark.root.as_ref() == Some(root))
        .map(|landmark| landmark.path.as_ref())
        .filter(|p| p.split_last().is_some_and(|(_, prefix)| prefix == parent))
        .collect();
    let index = siblings.iter().position(|p| *p == path)?;
    Some((parent, siblings, index))
}

#[cfg(test)]
mod tests;
