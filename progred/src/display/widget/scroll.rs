use super::HoverCallback;
use puri::handler::{HasHandler, ScrollOutcome};
use puri::{Point, Size, Vec2};

// Shared frame output; only the browser needs synchronous wheel admission.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
#[derive(Clone, Copy)]
pub struct Probe {
    pub placement: puri::Placement,
    pub offset: Vec2,
    pub maximum: Vec2,
    pub scale: f64,
}

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
impl Probe {
    pub fn captures(&self, event: &puri::handler::PointerScrollEvent) -> bool {
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

pub fn offset(
    stored: Vec2,
    update: &puri::handler::PointerScrollEvent,
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
