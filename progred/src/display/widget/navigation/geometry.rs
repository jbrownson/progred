//! The installed frame's landmarks and views, which navigation acts through
//! once the frame is placed: scrolling a view to show a place, arriving at a
//! landmark, and Tab's search for the next place to fill in.

use super::{Direction, Landmark};
use crate::display::Host;
use crate::display::widget::frame::ViewRegion;
use crate::model::Model;
use crate::selection::Selection;
use crate::workspace::Root;
use gid::Step;
use kurbo::{Rect, Vec2};

/// Borrowed geometry from the installed frame, used by explicit navigation actions.
pub(crate) struct Geometry<'a, W> {
    pub descends: &'a [Landmark<W>],
    pub view_regions: &'a [ViewRegion],
    pub scale: f64,
}

impl<W> Clone for Geometry<'_, W> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W> Copy for Geometry<'_, W> {}

impl<W> Default for Geometry<'_, W> {
    fn default() -> Self {
        Self {
            descends: &[],
            view_regions: &[],
            scale: 1.0,
        }
    }
}

impl<W> Geometry<'_, W> {
    /// Missing paths do nothing; new geometry does not retry the request.
    pub fn reveal_path(self, model: &mut Model, root: &Root, path: &[Step]) {
        if let Some(target) = self
            .descends
            .iter()
            .find(|target| target.root.as_ref() == Some(root) && target.path.as_ref() == path)
        {
            self.reveal_rect(model, root, target.rect);
        }
    }

    pub fn reveal_selection(self, model: &mut Model) {
        if let Some(selection) = &model.selection {
            let root = selection.root().clone();
            let path = selection.path().to_vec();
            self.reveal_path(model, &root, &path);
        }
    }

    pub fn reveal_rect(self, model: &mut Model, root: &Root, rect: Rect) -> bool {
        let Some(region) = self.view_regions.iter().find(|region| &region.root == root) else {
            return false;
        };
        let Some(view) = model.workspace.view_mut(root) else {
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
}

impl<W: Host> Geometry<'_, W> {
    /// Arrives at `target` as its widget says to, revealing what it selects.
    pub fn arrive(self, world: &mut W, target: &Landmark<W>, direction: Option<Direction>) -> bool {
        let handled = (target.select)(world, direction);
        if handled {
            self.reveal_selection(world.session().model);
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

/// Selects the next tab stop after the selection, or with `back` the one
/// before it, if there is one.
pub(crate) fn advance<W: Host>(world: &mut W, geometry: Geometry<'_, W>, back: bool) -> bool {
    let target = {
        let session = world.session();
        let selection = session.model.selection.as_ref();
        let root = selection
            .map(Selection::root)
            .unwrap_or_else(|| session.model.workspace.document_root())
            .clone();
        let anchor = selection.and_then(|selection| {
            geometry
                .descends
                .iter()
                .find(|descend| {
                    descend.root.as_ref() == Some(&root)
                        && descend.path.as_ref() == selection.path()
                })
                .map(|descend| descend.rect)
        });
        let stops = tab_stops(geometry.descends, &root);
        match (anchor, back) {
            (None, false) => stops.first(),
            (None, true) => stops.last(),
            (Some(anchor), false) => stops.iter().find(|stop| follows(stop.rect, anchor)),
            (Some(anchor), true) => stops.iter().rev().find(|stop| follows(anchor, stop.rect)),
        }
        .map(|stop| (*stop).clone())
    };
    target.is_some_and(|stop| geometry.arrive(world, &stop, None))
}

/// The view's tab stops, the places to fill in that its widgets marked, in
/// reading order: line by line, where a line is what overlaps vertically,
/// then left to right.
fn tab_stops<'a, W>(descends: &'a [Landmark<W>], root: &Root) -> Vec<&'a Landmark<W>> {
    let mut stops: Vec<_> = descends
        .iter()
        .filter(|descend| descend.tab_stop && descend.root.as_ref() == Some(root))
        .collect();
    stops.sort_by(|a, b| a.rect.y0.total_cmp(&b.rect.y0));
    let mut lines: Vec<(f64, Vec<&Landmark<W>>)> = Vec::new();
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

#[cfg(test)]
mod tests;
