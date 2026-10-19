//! Native pointer gestures, kept as samples rather than reduced to one delta.

use crate::handler::{Event, EventOutcome};
use std::borrow::Cow;
use ui_events::pointer::PointerGestureEvent;

/// Adapt a sample-wise control. Rejected samples continue to the next handler.
pub fn each<'a>(
    events: Cow<'a, [PointerGestureEvent]>,
    mut handle: impl FnMut(&PointerGestureEvent) -> bool,
) -> EventOutcome<'a> {
    let mut remaining = None;
    for (index, event) in events.iter().enumerate() {
        if handle(event) {
            remaining.get_or_insert_with(|| events[..index].to_vec());
        } else if let Some(remaining) = &mut remaining {
            remaining.push(event.clone());
        }
    }
    match remaining {
        None => EventOutcome::decline(Event::Gesture(events)),
        Some(remaining) => EventOutcome::handled_leaving(
            (!remaining.is_empty()).then_some(Event::Gesture(Cow::Owned(remaining))),
        ),
    }
}

/// Clip batches without changing their order or forcing sample-wise handling.
pub fn filter<'a>(
    events: Cow<'a, [PointerGestureEvent]>,
    includes: impl Fn(&PointerGestureEvent) -> bool,
    mut handle: impl for<'b> FnMut(Cow<'b, [PointerGestureEvent]>) -> EventOutcome<'b>,
) -> EventOutcome<'a> {
    crate::batch::filter(events, includes, |events| {
        handle(events).map(|remaining| match remaining {
            Some(Event::Gesture(events)) => events,
            None => Cow::Borrowed(&[][..]),
            _ => panic!("a gesture handler returned a non-gesture remainder"),
        })
    })
    .map(|events| (!events.is_empty()).then_some(Event::Gesture(events)))
}

#[cfg(test)]
mod tests;
