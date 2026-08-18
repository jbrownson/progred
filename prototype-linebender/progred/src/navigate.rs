//! Keyboard navigation over a frame's settled descends.

use crate::selection::Selection;
use gid::{Path, Step};
use progred_libraries::name;
use std::collections::HashMap;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use vello::kurbo::Rect;

/// A projected value's settled position: the path it stands for and the
/// rect it occupied, collected fresh every frame in placement order.
/// [`step_selection`] reads it to move the selection by keyboard;
/// clicks go through each descend's own handler, not this list.
#[derive(Clone)]
pub struct Descend {
    pub path: Path,
    /// The settled rect, for scroll-to-selection.
    pub rect: Rect,
}

/// Placement contexts that accumulate descends as the projection
/// places, so the shell can step selection by keyboard.
pub trait HasDescends {
    fn descends(&mut self) -> &mut Vec<Descend>;
}

/// Where the selection lands after deleting `path`: the next sibling,
/// else the previous, else the parent. Also where a discarded pending
/// edge returns to.
pub fn selection_after_delete(descends: &[Descend], path: &[Step]) -> Path {
    sibling(descends, path, true)
        .or_else(|| sibling(descends, path, false))
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
/// root when nothing is selected. `line` is one nominal line height,
/// the quantum separating "beside" from "below". Returns the path to
/// select, or `None` for keys navigation doesn't own.
pub fn step_selection(
    descends: &[Descend],
    selection: Option<&Selection>,
    line: f64,
    event: &KeyboardEvent,
) -> Option<Path> {
    let modified = event.modifiers.ctrl()
        || event.modifiers.meta()
        || event.modifiers.alt()
        || event.modifiers.shift();
    let arrow = match &event.key {
        Key::Named(
            named @ (NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::ArrowUp
            | NamedKey::ArrowDown),
        ) => Some(*named),
        _ => None,
    }
    .filter(|_| event.state.is_down() && !modified)?;
    let Some(selection) = selection else {
        return Some(Vec::new());
    };
    let path = selection.path();
    let order = reading_order(descends, line);
    let at = order
        .iter()
        .position(|stop| descends[stop.descend].path.as_slice() == path);
    let found = |stop: &Stop| Some(descends[stop.descend].path.clone());
    match (arrow, at) {
        (NamedKey::ArrowDown, Some(at)) => {
            order[at + 1..].iter().find(|stop| stop.row).and_then(found)
        }
        (NamedKey::ArrowUp, Some(at)) => order[..at]
            .iter()
            .rev()
            .find(|stop| stop.row)
            .and_then(found),
        (NamedKey::ArrowRight, Some(at)) => {
            order.get(at + 1).filter(|stop| !stop.row).and_then(found)
        }
        (NamedKey::ArrowLeft, Some(at)) if !order[at].row => found(&order[at - 1]),
        (NamedKey::ArrowLeft, _) => path.split_last().map(|(_, parent)| parent.to_vec()),
        _ => None,
    }
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
        [owner @ .., Step::Follow, Step::Key(label)] if *label == name::vocabulary::NAME => {
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
fn reading_order(descends: &[Descend], line: f64) -> Vec<Stop> {
    let by_path: HashMap<&[Step], usize> = descends
        .iter()
        .enumerate()
        .map(|(index, descend)| (descend.path.as_slice(), index))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); descends.len()];
    let mut roots = Vec::new();
    for (index, descend) in descends.iter().enumerate() {
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
fn sibling(descends: &[Descend], path: &[Step], next: bool) -> Option<Path> {
    let mut path = path.to_vec();
    loop {
        let (_, parent) = path.split_last()?;
        let siblings: Vec<&Path> = descends
            .iter()
            .map(|descend| &descend.path)
            .filter(|p| p.split_last().is_some_and(|(_, prefix)| prefix == parent))
            .collect();
        let index = siblings.iter().position(|p| **p == path)?;
        let neighbor = if next {
            siblings.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|index| siblings.get(index))
        };
        match neighbor {
            Some(found) => return Some((*found).clone()),
            None => {
                path.pop();
            }
        }
    }
}
