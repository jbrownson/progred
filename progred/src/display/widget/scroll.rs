use super::HoverCallback;
use kurbo::{Point, Size, Vec2};
use measured::Extent;
use puri::handler::{HasHandler, ScrollOutcome};

// Shared frame output; only the browser needs synchronous wheel admission.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
#[derive(Clone, Copy)]
pub struct Probe {
    pub placement: uig::Placement,
    pub offset: Vec2,
    pub maximum: Vec2,
    pub scale: f64,
}

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
impl Probe {
    pub fn captures(&self, event: &ui_events::pointer::PointerScrollEvent) -> bool {
        self.placement
            .contains(Point::new(event.state.position.x, event.state.position.y))
            && offset(
                self.offset,
                event,
                self.scale,
                self.placement.rect.size(),
                self.maximum,
            )
            .1
            .handled()
    }
}

/// How far `content` scrolls in a `viewport`, in logical units.
pub fn maximum(content: Extent, viewport: Size, scale: f64) -> Vec2 {
    Vec2::new(
        ((content.width - viewport.width) / scale).max(0.0),
        ((content.height() - viewport.height) / scale).max(0.0),
    )
}

/// Where `update` scrolls `stored`, within `maximum`, and what of it is left
/// over. ScrollDelta documents positive as viewport-down/right, but
/// ui-events-winit passes winit deltas through raw, where positive is
/// scroll-up/left; subtract to match reality. Stepping from the clamped
/// position keeps the first tick responsive when a resize left the stored
/// offset out of bounds.
pub fn offset(
    stored: Vec2,
    update: &ui_events::pointer::PointerScrollEvent,
    scale: f64,
    viewport: Size,
    maximum: Vec2,
) -> (Vec2, ScrollOutcome) {
    let mut next = stored;
    let outcome = units(scale, viewport).handle(update.delta, |delta| {
        let current = Vec2::new(
            stored.x.clamp(0.0, maximum.x),
            stored.y.clamp(0.0, maximum.y),
        );
        next = Vec2::new(
            (current.x - delta.x).clamp(0.0, maximum.x),
            (current.y - delta.y).clamp(0.0, maximum.y),
        );
        if next != stored {
            ScrollOutcome::with_remainder(delta - (current - next))
        } else {
            ScrollOutcome::unhandled(delta)
        }
    });
    (next, outcome)
}

pub fn units(scale: f64, viewport: Size) -> puri::scroll::Units {
    puri::scroll::Units {
        scale,
        line: 40.0,
        // A pane may briefly have no room during resize.
        page: Size::new(
            viewport.width.max(1.0) / scale,
            viewport.height.max(1.0) / scale,
        ),
    }
}

pub fn scroll<World: 'static, Hover: 'static>(
    scale: f64,
    handler: impl Fn(&mut World, Vec2) -> ScrollOutcome<Vec2> + 'static,
) -> HoverCallback<World, Hover> {
    Box::new(move |output, placement| {
        let units = units(scale, placement.rect.size());
        output.handler().on_scroll(move |world, event| {
            if placement.contains(Point::new(event.state.position.x, event.state.position.y)) {
                units.handle(event.delta, |delta| handler(world, delta))
            } else {
                ScrollOutcome::pass(event)
            }
        });
    })
}

#[cfg(test)]
mod tests;
