//! Keyboard navigation over a frame's settled descends.

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
    .filter(|_| {
        event.state.is_down()
            && !(event.modifiers.ctrl()
                || event.modifiers.meta()
                || event.modifiers.alt()
                || event.modifiers.shift())
    })
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
    // A visible leading edge is useful even when the target exceeds the viewport.
    if (viewport_start..=viewport_end).contains(&start) {
        return current;
    }
    let mut scroll = current;
    if end > viewport_end {
        scroll += (end + pad - viewport_end) / scale;
    }
    let adjusted_start = start - (scroll - current) * scale;
    if adjusted_start < viewport_start {
        scroll += (adjusted_start - pad - viewport_start) / scale;
    }
    scroll.clamp(0.0, maximum)
}

/// Where the selection lands after deleting `path`: the next sibling,
/// else the previous, else the parent. Also where a discarded pending
/// edge returns to.
pub fn selection_after_delete<World>(
    descends: &[Descend<World>],
    root: Option<&Root>,
    path: &[Step],
) -> Path {
    sibling(descends, root, path, true)
        .or_else(|| sibling(descends, root, path, false))
        .unwrap_or_else(|| {
            path.split_last()
                .map(|(_, parent)| parent.to_vec())
                .unwrap_or_default()
        })
}

/// The root translates only unclaimed keys. Routing is supplied by the chosen
/// projection, not reconstructed from geometry or document ancestry.
pub(crate) fn keyboard(
    editor: &mut crate::Editor,
    navigation: &[crate::display::widget::navigation::ViewNavigation<
        crate::display::widget::navigation::Graph,
    >],
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
    let Some(direction) = direction(event) else {
        return false;
    };
    let Some(selection) = editor.model.selection.as_ref() else {
        let root = root.clone();
        return root_target(geometry.descends, Some(&root)).is_some_and(|target| {
            target
                .scope
                .open(crate::editing::Access::new(editor))
                .select(&root, &target.path);
            true
        });
    };
    let destination = navigation
        .iter()
        .filter(|route| route.root.as_ref() == Some(root))
        .find_map(|route| route.navigation.destination(selection.path(), direction));
    if let Some(destination) = destination {
        arrive(editor, root.clone(), destination, direction);
        geometry.reveal_selection(editor);
        true
    } else {
        false
    }
}

pub(crate) fn arrive(
    editor: &mut crate::Editor,
    root: Root,
    stop: &crate::display::widget::navigation::Stop,
    direction: Direction,
) {
    use crate::display::widget::navigation::Entry;
    let mut edit = stop.scope.open(crate::editing::Access::new(editor));
    if stop.entry == Entry::Line && direction == Direction::Right {
        use crate::selection::payload::vocabulary::{ANCHOR, FOCUS};
        edit.select_payload(
            &root,
            stop.path.to_vec(),
            gid::Value::record([
                (ANCHOR, crate::libraries::f64::value(0.0)),
                (FOCUS, crate::libraries::f64::value(0.0)),
            ]),
        );
    } else {
        edit.select(&root, &stop.path);
    }
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
/// ancestors at the ends — where the selection lands after a delete,
/// via [`selection_after_delete`].
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
mod tests {
    use super::*;
    use crate::placed::ViewRegion;
    use std::rc::Rc;

    #[test]
    fn an_oversized_target_with_a_visible_leading_edge_does_not_scroll() {
        assert_eq!(
            reveal_axis(120.0, 1_000.0, 80.0, 500.0, 30.0, 200.0, 12.0, 1.0),
            120.0
        );
    }

    #[test]
    fn a_target_starting_beyond_the_viewport_is_revealed() {
        assert_eq!(
            reveal_axis(120.0, 1_000.0, 220.0, 260.0, 30.0, 200.0, 12.0, 1.0),
            192.0
        );
    }

    #[test]
    fn reveal_clamps_an_offset_left_stale_by_a_resize() {
        assert_eq!(
            reveal_axis(300.0, 100.0, 80.0, 120.0, 30.0, 200.0, 12.0, 1.0),
            100.0
        );
    }

    #[test]
    fn reveal_uses_the_requested_view_and_converts_pixels_to_scroll_units() {
        let mut editor = crate::test_editor(gid::Document {
            root: None,
            cells: gid::Cells::new(),
        });
        let root = editor.model.workspace.document_root().clone();
        let other = Root::document();
        let descends = [
            Descend {
                scope: Default::default(),
                root: Some(other.clone()),
                path: Rc::from([]),
                rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                select: Rc::new(|_, _| false),
            },
            Descend {
                scope: Default::default(),
                root: Some(root.clone()),
                path: Rc::from([]),
                rect: Rect::new(0.0, 1_000.0, 20.0, 1_060.0),
                select: Rc::new(|_, _| false),
            },
        ];
        let regions = [ViewRegion {
            root: root.clone(),
            rect: Rect::new(0.0, 0.0, 400.0, 200.0),
            maximum: Vec2::new(0.0, 1_000.0),
        }];
        let geometry = Geometry {
            descends: &descends,
            view_regions: &regions,
            scale: 2.0,
        };
        geometry.reveal_path(&mut editor, &other, &[]);
        assert_eq!(editor.model.workspace.document.scroll, Vec2::ZERO);
        geometry.reveal_path(&mut editor, &root, &[]);
        assert_eq!(
            editor.model.workspace.document.scroll,
            Vec2::new(0.0, 442.0)
        );
    }
}
