//! Convert scroll input and its unconsumed remainder through the same units.

use crate::handler::{Event, EventOutcome, PointerScrollEvent, ScrollDelta, ScrollOutcome};
use kurbo::{Size, Vec2};
use std::borrow::Cow;

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
    outcome(remaining, handled)
}

fn outcome(events: Cow<'_, [PointerScrollEvent]>, handled: bool) -> EventOutcome<'_> {
    let remaining = (!events.is_empty()).then_some(Event::Scroll(events));
    if handled {
        EventOutcome::with_remainder(remaining)
    } else {
        EventOutcome::unhandled(remaining)
    }
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
mod tests {
    use super::*;
    use crate::handler::{PointerInfo, PointerState, PointerType};

    fn packet(x: f64, y: f32, time: u64) -> PointerScrollEvent {
        PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (x, 0.0).into(),
                time,
                ..Default::default()
            },
            delta: ScrollDelta::LineDelta(0.0, y),
        }
    }

    #[test]
    fn sample_adapter_preserves_metadata_and_borrows_untouched_batches() {
        let packets = [packet(1.0, 2.0, 10), packet(2.0, 3.0, 20)];
        let result = each(Cow::Borrowed(&packets), ScrollOutcome::pass);
        assert!(!result.handled());
        assert!(
            matches!(result.remaining,Some(Event::Scroll(Cow::Borrowed(events))) if std::ptr::eq(events.as_ptr(),packets.as_ptr()))
        );
        let result = each(Cow::Borrowed(&packets), |_| {
            ScrollOutcome::with_remainder(ScrollDelta::LineDelta(0.0, 1.0))
        });
        let Some(Event::Scroll(events)) = result.remaining else {
            panic!("expected remainder")
        };
        for (event, original) in events.iter().zip(&packets) {
            assert_eq!(event.state, original.state);
            assert_eq!(event.pointer, original.pointer);
            assert_eq!(event.delta, ScrollDelta::LineDelta(0.0, 1.0));
        }
    }

    #[test]
    fn clipping_preserves_in_bounds_batches_and_ordered_remainders() {
        let packets = [
            packet(-1.0, 1.0, 10),
            packet(1.0, 2.0, 20),
            packet(2.0, 3.0, 30),
            packet(-1.0, 4.0, 40),
            packet(1.0, 5.0, 50),
        ];
        let mut runs = Vec::new();
        let result = filter(
            Cow::Borrowed(&packets),
            |event| event.state.position.x >= 0.0,
            |events| {
                runs.push(events.iter().map(|e| e.state.time).collect::<Vec<_>>());
                each(events, |event| {
                    if event.state.time == 30 {
                        ScrollOutcome::consume(event)
                    } else {
                        ScrollOutcome::with_remainder(ScrollDelta::LineDelta(0.0, 0.5))
                    }
                })
            },
        );
        assert!(result.handled());
        assert_eq!(runs, [vec![20, 30], vec![50]]);
        let Some(Event::Scroll(events)) = result.remaining else {
            panic!("expected remainder")
        };
        assert_eq!(
            events
                .iter()
                .map(|e| (e.state.time, e.delta))
                .collect::<Vec<_>>(),
            [
                (10, ScrollDelta::LineDelta(0.0, 1.0)),
                (20, ScrollDelta::LineDelta(0.0, 0.5)),
                (40, ScrollDelta::LineDelta(0.0, 4.0)),
                (50, ScrollDelta::LineDelta(0.0, 0.5))
            ]
        );
    }
}
