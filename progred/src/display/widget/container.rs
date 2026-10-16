//! Containers compose placement outputs without knowing which widgets produced them.

use super::HoverPass;

use measured::{Extent, Measured};
use puri::handler::{Event, EventOutcome, Handler, HasHandler, PointerScrollEvent, ScrollOutcome};
use puri::{Placement, Point, Vec2};

pub trait Layers {
    fn clipped(&mut self, placement: Placement, content: impl FnOnce(&mut Self));
    fn float(&mut self, above: impl FnOnce(&mut Self) + 'static);
}

pub fn floating<O: Layers + 'static>(
    base: Measured<O>,
    content: Measured<O>,
    place: impl FnOnce(Placement, Extent) -> Option<Placement> + 'static,
) -> Measured<O> {
    let extent = content.extent;
    measured::around_into(base, move |placement, base, output| {
        base.place_into(output);
        if let Some(placement) = place(placement, extent) {
            output.float(move |output| measured::place_into(content, placement, output));
        }
    })
}

/// The caller owns the offset. Only gesture starts and scroll are bounded;
/// motion, release, and keyboard events remain available to active controls.
pub fn scrolled<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    offset: Vec2,
    scale: f64,
    on_scroll: impl Fn(&mut World, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<HoverPass<World, H>> {
    let extent = child.extent;
    measured::around_into(child, move |placement, inner, output| {
        output.visit(move |base| {
            if !placement.clipped_out() {
                base.scroll_probe(super::scroll::Probe {
                    placement,
                    offset: offset / scale,
                    maximum: Vec2::new(
                        ((extent.width - placement.rect.width()) / scale).max(0.0),
                        ((extent.height() - placement.rect.height()) / scale).max(0.0),
                    ),
                    scale,
                });
                base.handler().on_scroll(move |state, event| {
                    if placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    {
                        on_scroll(state, event)
                    } else {
                        ScrollOutcome::pass(event)
                    }
                });
            }
        });
        let child_rect = inner.extent().rect_at(placement.rect.origin() - offset);
        let child_placement = measured::child_placement(
            measured::clipped_placement(placement, placement.rect),
            child_rect,
        );
        output.clipped(placement, |output| {
            inner.place_at_into(child_placement, output)
        });
    })
}

pub fn gate_starts<World: 'static, Input: 'static>(
    child: Handler<World, Input>,
    placement: Placement,
) -> Handler<World, Input> {
    Handler::from_function(move |world, event, input| {
        if let Event::Scroll(events) = event {
            return puri::scroll::filter(
                events,
                |event| {
                    placement.contains(Point::new(event.state.position.x, event.state.position.y))
                },
                |events| child.dispatch(world, Event::Scroll(events), input),
            );
        }
        if let Event::Gesture(events) = event {
            return puri::gesture::filter(
                events,
                |event| {
                    placement.contains(Point::new(event.state.position.x, event.state.position.y))
                },
                |events| child.dispatch(world, Event::Gesture(events), input),
            );
        }
        let position = match &event {
            Event::PointerDown(event) => Some(event.state.position),
            _ => None,
        };
        if position.is_some_and(|point| !placement.contains(Point::new(point.x, point.y))) {
            EventOutcome::decline(event)
        } else {
            child.dispatch(world, event, input)
        }
    })
}

#[cfg(test)]
mod tests;
