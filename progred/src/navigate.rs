//! Selection commands, arrow normalization, and revealing selected occurrences.

use crate::selection::Selection;
use crate::workspace::{Root, Target};
use gid::{Path, Step};
use kurbo::{Rect, Vec2};
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

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

/// Borrowed geometry from the installed frame, used by explicit navigation actions.
#[derive(Clone, Copy)]
pub(crate) struct Geometry<'a> {
    pub descends: &'a [Descend<crate::Editor>],
    pub view_regions: &'a [crate::placed::ViewRegion],
    pub scale: f64,
}

impl Default for Geometry<'_> {
    fn default() -> Self {
        Self {
            descends: &[],
            view_regions: &[],
            scale: 1.0,
        }
    }
}

impl Geometry<'_> {
    /// Missing paths do nothing; new geometry does not retry the request.
    pub fn reveal_path(self, editor: &mut crate::Editor, root: &Root, path: &[Step]) {
        if let Some(target) = self
            .descends
            .iter()
            .find(|target| target.root.as_ref() == Some(root) && target.path.as_ref() == path)
        {
            self.reveal_rect(editor, root, target.rect);
        }
    }

    pub fn reveal_selection(self, editor: &mut crate::Editor) {
        if let Some(selection) = &editor.model.selection {
            let root = selection.root().clone();
            let path = selection.path().to_vec();
            self.reveal_path(editor, &root, &path);
        }
    }

    pub fn reveal_rect(self, editor: &mut crate::Editor, root: &Root, rect: Rect) -> bool {
        let Some(region) = self.view_regions.iter().find(|region| &region.root == root) else {
            return false;
        };
        let Some(view) = editor.model.workspace.view_mut(root) else {
            return false;
        };
        let before = view.scroll;
        let pad = 12.0 * self.scale;
        view.scroll = Vec2::new(
            reveal_axis(
                before.x,
                region.maximum.x,
                rect.x0,
                rect.x1,
                region.rect.x0,
                region.rect.x1,
                pad,
                self.scale,
            ),
            reveal_axis(
                before.y,
                region.maximum.y,
                rect.y0,
                rect.y1,
                region.rect.y0,
                region.rect.y1,
                pad,
                self.scale,
            ),
        );
        view.scroll != before
    }

    pub fn arrive(
        self,
        editor: &mut crate::Editor,
        target: &Descend<crate::Editor>,
        direction: Option<Direction>,
    ) -> bool {
        let handled = (target.select)(editor, direction);
        if handled {
            self.reveal_selection(editor);
        }
        handled
    }
}

fn reveal_axis(
    current: f64,
    maximum: f64,
    start: f64,
    end: f64,
    viewport_start: f64,
    viewport_end: f64,
    pad: f64,
    scale: f64,
) -> f64 {
    let current = current.clamp(0.0, maximum);
    let mut scroll = current;
    // Reveal the full span; the margin is a landing allowance, not a trigger.
    if end > viewport_end {
        scroll += (end + pad - viewport_end) / scale;
    }
    // Favor the leading edge when the whole target cannot fit.
    let adjusted_start = start - (scroll - current) * scale;
    if adjusted_start < viewport_start {
        scroll += (adjusted_start - pad - viewport_start) / scale;
    }
    scroll.clamp(0.0, maximum)
}

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
    geometry: Geometry<'_>,
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
    geometry: Geometry<'_>,
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

/// Selects the next tab stop after the selection, or with `back` the one
/// before it, if there is one.
pub(crate) fn advance(editor: &mut crate::Editor, geometry: Geometry<'_>, back: bool) -> bool {
    let selection = editor.model.selection.as_ref();
    let root = selection
        .map(Selection::root)
        .unwrap_or_else(|| editor.model.workspace.document_root())
        .clone();
    let anchor = selection.and_then(|selection| {
        geometry
            .descends
            .iter()
            .find(|descend| {
                descend.root.as_ref() == Some(&root) && descend.path.as_ref() == selection.path()
            })
            .map(|descend| descend.rect)
    });
    let stops = tab_stops(geometry.descends, &root);
    let target = match (anchor, back) {
        (None, false) => stops.first(),
        (None, true) => stops.last(),
        (Some(anchor), false) => stops.iter().find(|stop| follows(stop.rect, anchor)),
        (Some(anchor), true) => stops.iter().rev().find(|stop| follows(anchor, stop.rect)),
    }
    .map(|stop| (*stop).clone());
    target.is_some_and(|stop| geometry.arrive(editor, &stop, None))
}

/// The view's tab stops, the places to fill in that its widgets marked, in
/// reading order: line by line, where a line is what overlaps vertically,
/// then left to right.
fn tab_stops<'a>(
    descends: &'a [Descend<crate::Editor>],
    root: &Root,
) -> Vec<&'a Descend<crate::Editor>> {
    let mut stops: Vec<_> = descends
        .iter()
        .filter(|descend| descend.tab_stop && descend.root.as_ref() == Some(root))
        .collect();
    stops.sort_by(|a, b| a.rect.y0.total_cmp(&b.rect.y0));
    let mut lines: Vec<(f64, Vec<&Descend<crate::Editor>>)> = Vec::new();
    for stop in stops {
        match lines.last_mut() {
            Some((bottom, line)) if stop.rect.y0 < *bottom => {
                *bottom = bottom.max(stop.rect.y1);
                line.push(stop);
            }
            _ => lines.push((stop.rect.y1, vec![stop])),
        }
    }
    lines
        .into_iter()
        .flat_map(|(_, mut line)| {
            line.sort_by(|a, b| a.rect.x0.total_cmp(&b.rect.x0));
            line
        })
        .collect()
}

/// Whether `rect` comes after `anchor` in reading order: further along a
/// line they share, or below it.
fn follows(rect: Rect, anchor: Rect) -> bool {
    if rect.y0 < anchor.y1 && anchor.y0 < rect.y1 {
        rect.x0 > anchor.x0
    } else {
        rect.y0 >= anchor.y1
    }
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
