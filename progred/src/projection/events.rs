//! Bind the remaining projection gesture requests to editor state.

use crate::frame::Hovered;
use crate::hover::Hover;
use crate::placed::{Placed, before};
use gid::Path;
use measured::Measured;
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::{Canvas, Placement, Point};
use std::rc::Rc;

pub(super) fn realize_scrub<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    target: Hover,
    handler: progred_display::ScrubHandler,
    start: Rc<dyn Fn(&mut C, Path, progred_display::ScrubHandler, Point, f64) -> bool>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.pick_with(Hovered::Tree(target), move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            placement.contains(point) && start(world, path.clone(), handler.clone(), point, scale)
        });
    })
}

pub(super) fn realize_state_drag<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    target: Hover,
    on_press: progred_display::ActionHandler<C>,
    handler: progred_display::StateDragHandler,
    start: Rc<dyn Fn(&mut C, Path, progred_display::StateDragHandler, Point, f64)>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.activate_with(Hovered::Tree(target), move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            if placement.contains(point) && on_press(world) {
                start(world, path.clone(), handler.clone(), point, scale);
                true
            } else {
                false
            }
        });
    })
}

pub(super) fn realize_point<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    handler: progred_display::PointHandler,
    start: Rc<dyn Fn(&mut C, Path, Placement, progred_display::PointHandler, Point) -> bool>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            is_primary_contact(event)
                && placement.contains(point)
                && start(world, path.clone(), placement, handler.clone(), point)
        });
    })
}
