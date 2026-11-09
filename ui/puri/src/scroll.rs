//! Convert scroll input and its unconsumed remainder through the same units.

use crate::handler::{Event, EventOutcome, ScrollOutcome};
use kurbo::{Size, Vec2};
use std::borrow::Cow;
use ui_events::ScrollDelta;
use ui_events::pointer::PointerScrollEvent;

/// Adapt a sample-wise control without choosing this policy for every handler.
pub fn each<'a>(
    events: Cow<'a, [PointerScrollEvent]>,
    mut handle: impl FnMut(&PointerScrollEvent) -> ScrollOutcome,
) -> EventOutcome<'a> {
    let mut handled = false;
    let mut remaining: Option<Vec<PointerScrollEvent>> = None;
    for (index, event) in events.iter().enumerate() {
        let outcome = handle(event);
        handled |= outcome.handled();
        let empty = outcome.remaining == ScrollOutcome::consume(event).remaining;
        if empty || outcome.remaining != event.delta {
            let remaining = remaining.get_or_insert_with(|| events[..index].to_vec());
            remaining.extend(outcome.event(event));
        } else if let Some(remaining) = &mut remaining {
            remaining.push(event.clone());
        }
    }
    let remaining = remaining.map(Cow::Owned).unwrap_or(events);
    EventOutcome::new(
        (!remaining.is_empty()).then_some(Event::Scroll(remaining)),
        handled,
    )
}

/// A clip preserves batches within its bounds. Crossings divide the batch into
/// contiguous runs, so excluded samples and child remainders retain their order.
pub fn filter<'a>(
    events: Cow<'a, [PointerScrollEvent]>,
    includes: impl Fn(&PointerScrollEvent) -> bool,
    mut handle: impl for<'b> FnMut(Cow<'b, [PointerScrollEvent]>) -> EventOutcome<'b>,
) -> EventOutcome<'a> {
    crate::batch::filter(events, includes, |events| {
        handle(events).map(|remaining| match remaining {
            Some(Event::Scroll(events)) => events,
            None => Cow::Borrowed(&[][..]),
            _ => panic!("a scroll handler returned a non-scroll remainder"),
        })
    })
    .map(|events| (!events.is_empty()).then_some(Event::Scroll(events)))
}

/// Positive conversion factors: pixels per logical point, points per line,
/// and the page extent in logical points. Policy belongs to the caller.
pub struct Units {
    pub scale: f64,
    pub line: f64,
    pub page: Size,
}

impl Units {
    pub fn handle(
        &self,
        delta: ScrollDelta,
        handle: impl FnOnce(Vec2) -> ScrollOutcome<Vec2>,
    ) -> ScrollOutcome {
        let (input, units) = match delta {
            ScrollDelta::PixelDelta(point) => (
                Vec2::new(point.x, point.y),
                Vec2::new(1.0 / self.scale, 1.0 / self.scale),
            ),
            ScrollDelta::LineDelta(x, y) => (
                Vec2::new(f64::from(x), f64::from(y)),
                Vec2::new(self.line, self.line),
            ),
            ScrollDelta::PageDelta(x, y) => (
                Vec2::new(f64::from(x), f64::from(y)),
                Vec2::new(self.page.width, self.page.height),
            ),
        };
        handle(Vec2::new(input.x * units.x, input.y * units.y)).map(|remaining| {
            let remaining = Vec2::new(remaining.x / units.x, remaining.y / units.y);
            match delta {
                ScrollDelta::PixelDelta(_) => {
                    ScrollDelta::PixelDelta((remaining.x, remaining.y).into())
                }
                ScrollDelta::LineDelta(..) => {
                    ScrollDelta::LineDelta(remaining.x as f32, remaining.y as f32)
                }
                ScrollDelta::PageDelta(..) => {
                    ScrollDelta::PageDelta(remaining.x as f32, remaining.y as f32)
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "tests/scroll.rs"]
mod tests;
