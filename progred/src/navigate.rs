//! Keyboard navigation over a frame's settled descends.

use crate::selection::Selection;
use crate::workspace::{Root, Target};
use gid::{Path, Step};
use kurbo::Rect;
use progred_libraries::name;
use std::collections::HashMap;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Movement direction, or none for selection without directional navigation.
pub type Select<World> = Rc<dyn Fn(&mut World, Option<Direction>) -> bool>;

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

/// A projected value's settled position: the path it stands for and the
/// rect it occupied, collected fresh every frame in placement order.
/// [`step_selection`] reads it to move the selection by keyboard;
/// clicks go through each descend's own handler, not this list.
pub struct Descend<World> {
    /// The editor view that produced this occurrence. Paths may be
    /// projected in more than one pane at once.
    pub root: Option<Root>,
    pub path: Rc<[Step]>,
    /// The settled rect, for scroll-to-selection.
    pub rect: Rect,
    /// The projection-installed transition for landing here. This is
    /// usually ordinary edge selection, but a projected control may
    /// mount its own editing state without the shell inspecting the
    /// projected layout to rediscover it.
    pub select: Select<World>,
}

impl<World> Clone for Descend<World> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            rect: self.rect,
            select: self.select.clone(),
        }
    }
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

/// Keyboard navigation over the frame's descends, reading the layout
/// the frame actually chose. Down and up walk the ROWS — every stop
/// that opens a new line of its container — in reading order,
/// entering open blocks the way a file tree walks its visible rows,
/// so each press moves down (or up) the screen. Right and left walk
/// WITHIN the line, into and across the content beside the current
/// stop; left from a row widens to the parent. Any arrow selects the
/// view's root when nothing is selected; Cmd+A (Ctrl+A elsewhere)
/// selects that root from anywhere. `line` is one nominal line height,
/// the quantum separating "beside" from "below". Returns the settled
/// landmark whose installed transition should run, or `None` for keys
/// navigation doesn't own.
pub fn step_selection<'a, World>(
    descends: &'a [Descend<World>],
    root: Option<&Root>,
    selection: Option<&Selection>,
    line: f64,
    event: &KeyboardEvent,
) -> Option<&'a Descend<World>> {
    if event.state.is_down()
        && crate::modifiers::command(&event.modifiers)
        && !(event.modifiers.shift() || event.modifiers.alt())
        && matches!(&event.key, Key::Character(key) if key.eq_ignore_ascii_case("a"))
    {
        return root_target(descends, root);
    }
    let direction = direction(event)?;
    let Some(selection) = selection else {
        return root_target(descends, root);
    };
    let path = selection.path();
    let order = reading_order(descends, root, line);
    let at = order
        .iter()
        .position(|stop| descends[stop.descend].path.as_ref() == path);
    let found = |stop: &Stop| Some(&descends[stop.descend]);
    match (direction, at) {
        (Direction::Down, Some(at)) => order[at + 1..].iter().find(|stop| stop.row).and_then(found),
        (Direction::Up, Some(at)) => order[..at]
            .iter()
            .rev()
            .find(|stop| stop.row)
            .and_then(found),
        (Direction::Right, Some(at)) => order.get(at + 1).filter(|stop| !stop.row).and_then(found),
        (Direction::Left, Some(at)) if !order[at].row => found(&order[at - 1]),
        (Direction::Left, _) => path.split_last().and_then(|(_, parent)| {
            descends.iter().find(|descend| {
                root.is_none_or(|root| descend.root.as_ref() == Some(root))
                    && descend.path.as_ref() == parent
            })
        }),
        _ => None,
    }
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

/// One stop in the frame's reading order: pre-order over the
/// descends, with the bit saying whether the stop opens a new line of
/// its container (a row) or rides one beside its predecessor.
struct Stop {
    descend: usize,
    row: bool,
}

pub(crate) fn projected_name_owner(path: &[Step]) -> Option<&[Step]> {
    match path {
        [owner @ .., Step::Follow(_), Step::Key(label)] if *label == name::vocabulary::NAME => {
            Some(owner)
        }
        _ => None,
    }
}

/// The frame's stops in pre-order, each classified as row or beside
/// from the geometry the layout settled: a stop is a row when its
/// container stacked it — no shared line band with the sibling before
/// it, or first into a multi-line container — and beside when it
/// rides the same line. A projected simple-name field is its owner's
/// own first line, never a row. Order is rebuilt from per-parent
/// registration order, which is document order; the raw list settles
/// children first.
fn reading_order<World>(descends: &[Descend<World>], root: Option<&Root>, line: f64) -> Vec<Stop> {
    let by_path: HashMap<&[Step], usize> = descends
        .iter()
        .enumerate()
        .filter(|(_, descend)| root.is_none_or(|root| descend.root.as_ref() == Some(root)))
        .map(|(index, descend)| (descend.path.as_ref(), index))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); descends.len()];
    let mut roots = Vec::new();
    for (index, descend) in descends.iter().enumerate() {
        if root.is_some_and(|root| descend.root.as_ref() != Some(root)) {
            continue;
        }
        let parent = projected_name_owner(&descend.path)
            .and_then(|owner| by_path.get(owner).copied())
            .or_else(|| {
                (0..descend.path.len())
                    .rev()
                    .find_map(|end| by_path.get(&descend.path[..end]).copied())
            });
        match parent {
            Some(parent) => children[parent].push(index),
            None => roots.push(index),
        }
    }
    let mut order = Vec::with_capacity(descends.len());
    let mut stack: Vec<(usize, Option<usize>, Option<usize>)> = roots
        .into_iter()
        .rev()
        .map(|root| (root, None, None))
        .collect();
    while let Some((index, parent, before)) = stack.pop() {
        let descend = &descends[index];
        let row = match (parent, before) {
            (None, _) => true,
            _ if projected_name_owner(&descend.path).is_some() => false,
            (Some(_), Some(before)) => !same_line(descend.rect, descends[before].rect, line),
            (Some(parent), None) => descends[parent].rect.height() > line * 1.5,
        };
        order.push(Stop {
            descend: index,
            row,
        });
        let mut before = None;
        let entries: Vec<_> = children[index]
            .iter()
            .map(|&child| {
                let entry = (child, Some(index), before);
                before = Some(child);
                entry
            })
            .collect();
        stack.extend(entries.into_iter().rev());
    }
    order
}

/// Whether two settled rects share a line: their vertical bands
/// overlap by more than half a line — baseline-aligned neighbors
/// overlap by most of one, stacked rows touch at the edges at most.
fn same_line(a: Rect, b: Rect, line: f64) -> bool {
    a.y1.min(b.y1) - a.y0.max(b.y0) > line * 0.5
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
