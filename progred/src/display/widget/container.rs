//! Containers compose placement outputs without knowing which widgets produced them.

use super::HoverPass;

use kurbo::{Point, Rect, Vec2};
use measured::{Extent, Measured};
use puri::handler::{Event, EventOutcome, Handler, HasHandler, ScrollOutcome};
use ui_events::pointer::PointerScrollEvent;
use uig::Placement;

/// `content` floating over `base`, where `place` puts it from `base`'s
/// placement, its extent, and the bounds floats may use.
pub fn floating<W: 'static, H: 'static>(
    base: Measured<HoverPass<W, H>>,
    content: Measured<HoverPass<W, H>>,
    place: impl FnOnce(Placement, Extent, Rect) -> Option<Placement> + 'static,
) -> Measured<HoverPass<W, H>> {
    let extent = content.extent;
    measured::around_into(base, move |placement, base, output| {
        base.place_into(output);
        if let Some(placement) = place(placement, extent, output.bounds()) {
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
                    maximum: super::scroll::maximum(extent, placement.rect.size(), scale),
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
    Handler::from_function(move |world, event, input| match event {
        Event::Scroll(events) => puri::scroll::filter(
            events,
            |event| placement.contains(Point::new(event.state.position.x, event.state.position.y)),
            |events| child.dispatch(world, Event::Scroll(events), input),
        ),
        Event::Gesture(events) => puri::gesture::filter(
            events,
            |event| placement.contains(Point::new(event.state.position.x, event.state.position.y)),
            |events| child.dispatch(world, Event::Gesture(events), input),
        ),
        Event::PointerDown(down)
            if !placement.contains(Point::new(down.state.position.x, down.state.position.y)) =>
        {
            EventOutcome::decline(Event::PointerDown(down))
        }
        event => child.dispatch(world, event, input),
    })
}

#[cfg(test)]
mod tests;
