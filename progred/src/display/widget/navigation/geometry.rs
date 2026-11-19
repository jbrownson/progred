//! The installed frame's landmarks and views, which navigation acts through
//! once the frame is placed: scrolling a view to show a place, arriving at a
//! landmark, and Tab's search for the next place to fill in.

use super::{Landmark, TabStop, landmark_at};
use crate::display::Host;
use crate::display::widget::frame::ViewRegion;
use crate::display::widget::view::Area;
use crate::model::Model;
use crate::occurrence::Step;
use kurbo::{Rect, Vec2};
use puri::handler::NavigationDirection;

/// Borrowed geometry from the installed frame, used by explicit navigation actions.
pub(crate) struct Geometry<'a, W> {
    pub landmarks: &'a [Landmark<W>],
    pub tab_stops: &'a [TabStop<W>],
    pub view_regions: &'a [ViewRegion],
    pub scale: f64,
}

impl<W> Clone for Geometry<'_, W> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W> Copy for Geometry<'_, W> {}

/// How far inside its view, in logical units, a revealed place lands.
const REVEAL_PAD: f64 = 12.0;

impl<W> Geometry<'_, W> {
    /// Missing paths do nothing; new geometry does not retry the request.
    fn reveal_path(self, model: &mut Model, area: &Area, path: &[Step]) {
        if let Some(target) = landmark_at(self.landmarks, Some(area), path) {
            self.reveal_rect(model, area, target.rect);
        }
    }

    pub fn reveal_selection(self, model: &mut Model) {
        if let Some(selection) = &model.selection {
            let area = model.workspace.area_of(selection.view(), selection.path());
            let path = selection.path().to_vec();
            self.reveal_path(model, &area, &path);
        }
    }

    pub fn reveal_rect(self, model: &mut Model, area: &Area, rect: Rect) {
        if let (Some(region), Some(pane)) = (
            self.view_regions.iter().find(|region| &region.area == area),
            model.workspace.pane_mut(area),
        ) {
            pane.scroll = Vec2::new(
                reveal_axis(
                    pane.scroll.x,
                    region.maximum.x,
                    (rect.x0, rect.x1),
                    (region.rect.x0, region.rect.x1),
                    self.scale,
                ),
                reveal_axis(
                    pane.scroll.y,
                    region.maximum.y,
                    (rect.y0, rect.y1),
                    (region.rect.y0, region.rect.y1),
                    self.scale,
                ),
            );
        }
    }
}

impl<W: Host> Geometry<'_, W> {
    /// Arrives at `target` as its widget says to, revealing what it selects.
    pub fn arrive(
        self,
        world: &mut W,
        target: &Landmark<W>,
        direction: Option<NavigationDirection>,
    ) -> bool {
        let handled = (target.select)(world, direction);
        if handled {
            self.reveal_selection(world.session().model);
        }
        handled
    }
}

/// The scroll offset along one axis that shows the span from `start` to `end`
/// within the viewport's span from `viewport_start` to `viewport_end`, both
/// in pixels, moving as little as it can from `current`.
fn reveal_axis(
    current: f64,
    maximum: f64,
    (start, end): (f64, f64),
    (viewport_start, viewport_end): (f64, f64),
    scale: f64,
) -> f64 {
    let pad = REVEAL_PAD * scale;
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
/// before it, if there is one, in the order the view is drawn in.
pub(crate) fn advance<W: Host>(world: &mut W, geometry: Geometry<'_, W>, back: bool) -> bool {
    let (area, target) = {
        let session = world.session();
        let selection = session.model.selection.as_ref();
        let area = selection.map_or_else(
            || session.model.workspace.document_area(),
            |selection| {
                session
                    .model
                    .workspace
                    .area_of(selection.view(), selection.path())
            },
        );
        let mut stops: Vec<_> = geometry
            .tab_stops
            .iter()
            .filter(|stop| stop.area.as_ref() == Some(&area))
            .collect();
        stops.sort_by_key(|stop| stop.order);
        // Where the selection is in that order: the place to fill in it's
        // at, or else what's drawn there.
        let anchor = selection.and_then(|selection| {
            stops
                .iter()
                .find(|stop| stop.at.as_ref().is_some_and(|at| *at == selection.path()))
                .map(|stop| stop.order)
                .or_else(|| {
                    landmark_at(geometry.landmarks, Some(&area), selection.path())
                        .map(|landmark| landmark.order)
                })
        });
        let target = match (anchor, back) {
            (None, false) => stops.first(),
            (None, true) => stops.last(),
            (Some(anchor), false) => stops.iter().find(|stop| stop.order > anchor),
            (Some(anchor), true) => stops.iter().rev().find(|stop| stop.order < anchor),
        }
        .map(|stop| (stop.rect, stop.select.clone()));
        (area, target)
    };
    target.is_some_and(|(rect, select)| {
        let handled = select(world, None);
        if handled {
            // What it opens isn't drawn yet, so it's shown where it will be.
            geometry.reveal_rect(world.session().model, &area, rect);
        }
        handled
    })
}

#[cfg(test)]
#[path = "tests/geometry.rs"]
mod tests;
