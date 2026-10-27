//! The editor's navigation keys: arrows, Tab and Select All, and where the
//! selection lands when what it was on goes away. They act through the
//! installed frame's [`Geometry`].

use crate::display::widget::navigation::{Geometry, Landmark, advance};
use crate::display::widget::view::Area;
use crate::occurrence::{Path, Step, Trail};
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
    area: &Area,
    path: &[Step],
    forward: bool,
) -> Path {
    forward
        .then(|| following(landmarks, area, path))
        .flatten()
        .or_else(|| preceding(landmarks, area, path))
        .unwrap_or_else(|| container(landmarks, area, path))
}

/// The nearest drawn occurrence around `path`, past any drawn as part of
/// another, else `path`'s parent.
fn container<World>(landmarks: &[Landmark<World>], area: &Area, path: &[Step]) -> Path {
    let drawn = |path: &[Step]| {
        landmarks
            .iter()
            .any(|landmark| landmark.area.as_ref() == Some(area) && landmark.path == path)
    };
    let parent = path.split_last().map_or(&[][..], |(_, parent)| parent);
    (0..=parent.len())
        .rev()
        .map(|end| &parent[..end])
        .find(|around| drawn(around))
        .unwrap_or(parent)
        .to_vec()
}

/// Select All, once focused controls decline the raw key, selects the root
/// of the selection's area, else of the document's.
pub(crate) fn select_all_key(
    editor: &mut crate::Editor,
    geometry: Geometry<'_, crate::Editor>,
    event: &KeyboardEvent,
) -> bool {
    let area = editor.model.selection.as_ref().map_or_else(
        || editor.model.workspace.document_area(),
        |selection| {
            editor
                .model
                .workspace
                .area_of(selection.view(), selection.path())
        },
    );
    select_all(editor.command_modifier, geometry.landmarks, &area, event)
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

/// An arrow with no selection selects the root of `area`. So does one with
/// the selection still `undrawn`, on nothing this frame draws.
pub(crate) fn initial_navigation(
    area: &Area,
    landmarks: &[Landmark<crate::Editor>],
    undrawn: Option<Path>,
) -> Option<puri::handler::Handler<crate::Editor, crate::frame::DispatchContext<crate::Editor>>> {
    let target = root_target(landmarks, area)?;
    let (view, path) = (target.path.view().clone(), target.path.to_vec());
    Some(puri::handler::Handler::from_function(
        move |editor: &mut crate::Editor, event, _| {
            if matches!(event, puri::handler::Event::Navigate(_))
                && editor
                    .model
                    .selection
                    .as_ref()
                    .map(|selection| selection.path())
                    == undrawn.as_deref()
            {
                crate::editing::Edit::new(editor.session()).select(&view, &path);
                puri::handler::EventOutcome::accept()
            } else {
                puri::handler::EventOutcome::decline(event)
            }
        },
    ))
}

/// The root of `area`, when `event` is Select All.
pub fn select_all<'a, World>(
    command: puri::keyboard::CommandModifier,
    landmarks: &'a [Landmark<World>],
    area: &Area,
    event: &KeyboardEvent,
) -> Option<&'a Landmark<World>> {
    (crate::modifiers::chord(command, event, "a")
        && !(event.modifiers.shift() || event.modifiers.alt()))
    .then(|| root_target(landmarks, area))
    .flatten()
}

/// What's drawn at the root of `area`: its outermost value, such as a
/// part's, or the place a view shows.
fn root_target<'a, World>(
    landmarks: &'a [Landmark<World>],
    area: &Area,
) -> Option<&'a Landmark<World>> {
    landmarks
        .iter()
        .filter(|landmark| landmark.area.as_ref() == Some(area))
        .min_by_key(|landmark| landmark.path.len())
}

/// The stop after the drawn `path`: its next sibling, else the one after its
/// nearest ancestor that has one.
fn following<World>(landmarks: &[Landmark<World>], area: &Area, path: &[Step]) -> Option<Path> {
    let (parent, siblings, index) = siblings(landmarks, area, path)?;
    siblings
        .get(index + 1)
        .map(|next| next.to_vec())
        .or_else(|| following(landmarks, area, parent))
}

/// The sibling before the drawn `path`.
fn preceding<World>(landmarks: &[Landmark<World>], area: &Area, path: &[Step]) -> Option<Path> {
    let (_, siblings, index) = siblings(landmarks, area, path)?;
    index
        .checked_sub(1)
        .and_then(|index| siblings.get(index))
        .map(|previous| previous.to_vec())
}

/// The drawn `path`'s parent, and its siblings in placement order, itself
/// among them at `index`.
fn siblings<'a, World>(
    landmarks: &'a [Landmark<World>],
    area: &Area,
    path: &'a [Step],
) -> Option<(&'a [Step], Vec<&'a Trail>, usize)> {
    let (_, parent) = path.split_last()?;
    let siblings: Vec<&Trail> = landmarks
        .iter()
        .filter(|landmark| landmark.area.as_ref() == Some(area))
        .map(|landmark| &landmark.path)
        .filter(|p| p.parent().is_some_and(|prefix| *prefix == parent))
        .collect();
    let index = siblings.iter().position(|p| **p == path)?;
    Some((parent, siblings, index))
}

#[cfg(test)]
#[path = "tests/navigate/mod.rs"]
mod tests;
