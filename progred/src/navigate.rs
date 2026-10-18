//! The editor's navigation keys: arrows, Tab and Select All, and where the
//! selection lands when what it was on goes away. They act through the
//! installed frame's [`Geometry`].

use crate::display::widget::navigation::advance;
use crate::selection::Selection;
use crate::workspace::{Root, Target};
use gid::{Path, Step};
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

pub(crate) use crate::display::widget::navigation::Geometry;

pub use crate::display::widget::{Direction, Select};

pub fn direction(event: &KeyboardEvent) -> Option<Direction> {
    match &event.key {
        Key::Named(NamedKey::ArrowLeft) => Some(Direction::Left),
        Key::Named(NamedKey::ArrowRight) => Some(Direction::Right),
        Key::Named(NamedKey::ArrowUp) => Some(Direction::Up),
        Key::Named(NamedKey::ArrowDown) => Some(Direction::Down),
        _ => None,
    }
    .filter(|_| event.state.is_down())
}

pub use crate::display::widget::navigation::Landmark as Descend;

/// Where the selection lands after removing `path`: the sibling after it
/// (before it, moving backward), else the other one, else the parent.
pub fn selection_after_removing<World>(
    descends: &[Descend<World>],
    root: Option<&Root>,
    path: &[Step],
    forward: bool,
) -> Path {
    sibling(descends, root, path, forward)
        .or_else(|| sibling(descends, root, path, !forward))
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
        geometry.descends,
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
    descends: &[Descend<crate::Editor>],
) -> Option<puri::handler::Handler<crate::Editor, crate::placed::DispatchContext<crate::Editor>>> {
    let target = root_target(descends, Some(root))?;
    let root = root.clone();
    let path = target.path.clone();
    let scope = target.scope.clone();
    Some(puri::handler::Handler::from_function(
        move |editor: &mut crate::Editor, event, _| {
            if matches!(event, puri::handler::Event::Navigate(_))
                && editor.model.selection.is_none()
            {
                scope
                    .open(crate::editing::Access::new(editor))
                    .select(&root, &path);
                puri::handler::EventOutcome::accept()
            } else {
                puri::handler::EventOutcome::decline(event)
            }
        },
    ))
}

pub fn select_all<'a, World>(
    command: puri::keyboard::CommandModifier,
    descends: &'a [Descend<World>],
    root: Option<&Root>,
    event: &KeyboardEvent,
) -> Option<&'a Descend<World>> {
    (event.state.is_down()
        && command.pressed(&event.modifiers)
        && !(event.modifiers.shift() || event.modifiers.alt())
        && matches!(&event.key, Key::Character(key) if key.eq_ignore_ascii_case("a")))
    .then(|| root_target(descends, root))
    .flatten()
}

fn root_target<'a, World>(
    descends: &'a [Descend<World>],
    root: Option<&Root>,
) -> Option<&'a Descend<World>> {
    let path = match root.map(Root::target) {
        Some(Target::Pane { path }) => path.as_slice(),
        _ => &[],
    };
    descends.iter().find(|descend| {
        root.is_none_or(|root| descend.root.as_ref() == Some(root)) && descend.path.as_ref() == path
    })
}

/// The neighboring sibling in placement order, continuing through
/// ancestors at the ends — where the selection lands after a removal,
/// via [`selection_after_removing`].
fn sibling<World>(
    descends: &[Descend<World>],
    root: Option<&Root>,
    path: &[Step],
    next: bool,
) -> Option<Path> {
    let mut path = path.to_vec();
    loop {
        let (_, parent) = path.split_last()?;
        let siblings: Vec<&[Step]> = descends
            .iter()
            .filter(|descend| root.is_none_or(|root| descend.root.as_ref() == Some(root)))
            .map(|descend| descend.path.as_ref())
            .filter(|p| p.split_last().is_some_and(|(_, prefix)| prefix == parent))
            .collect();
        let index = siblings.iter().position(|p| *p == path)?;
        let neighbor = if next {
            siblings.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|index| siblings.get(index))
        };
        match neighbor {
            Some(found) => return Some(found.to_vec()),
            None => {
                path.pop();
            }
        }
    }
}

#[cfg(test)]
mod tests;
