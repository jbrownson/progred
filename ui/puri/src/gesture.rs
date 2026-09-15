//! Native pointer gestures, kept as samples rather than reduced to one delta.

use crate::handler::{Event, EventOutcome, PointerGestureEvent};
use std::borrow::Cow;

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
        Some(remaining) => EventOutcome::with_remainder(
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
mod tests {
    use super::*;
    use crate::handler::{Handler, PointerGesture, PointerInfo, PointerState, PointerType};

    fn sample(x: f64, gesture: PointerGesture) -> PointerGestureEvent {
        PointerGestureEvent {
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (x, 0.0).into(),
                ..Default::default()
            },
            gesture,
        }
    }

    #[test]
    fn rejected_samples_reach_the_next_handler_in_order() {
        let events = [
            sample(0.0, PointerGesture::Pinch(0.1)),
            sample(1.0, PointerGesture::Rotate(0.2)),
            sample(2.0, PointerGesture::Pinch(-0.3)),
        ];
        let mut handler = Handler::<Vec<f64>>::new();
        handler.on_gesture(|log, event| {
            log.push(event.state.position.x);
            true
        });
        handler.on_gesture(|_, event| matches!(event.gesture, PointerGesture::Pinch(_)));
        let mut log = vec![];
        let result = handler.dispatch(&mut log, Event::Gesture(Cow::Borrowed(&events)), &mut ());
        assert!(result.handled());
        assert!(result.remaining.is_none());
        assert_eq!(log, [1.0]);
    }

    #[test]
    fn clipping_preserves_contiguous_batches_and_remainders() {
        let events = [0.0, 1.0, 2.0, 3.0, 4.0].map(|x| sample(x, PointerGesture::Pinch(0.1)));
        let mut runs = vec![];
        let result = filter(
            Cow::Borrowed(&events),
            |event| (1.0..=3.0).contains(&event.state.position.x),
            |events| {
                runs.push(events.len());
                each(events, |event| event.state.position.x != 2.0)
            },
        );
        assert_eq!(runs, [3]);
        assert!(result.handled());
        let Some(Event::Gesture(rest)) = result.remaining else {
            panic!("expected remainder")
        };
        assert_eq!(
            rest.iter()
                .map(|event| event.state.position.x)
                .collect::<Vec<_>>(),
            [0.0, 2.0, 4.0]
        );
    }

    #[test]
    fn declined_batch_stays_borrowed() {
        let events = [sample(0.0, PointerGesture::Pinch(0.1))];
        let result = each(Cow::Borrowed(&events), |_| false);
        assert!(!result.handled());
        assert!(
            matches!(result.remaining, Some(Event::Gesture(Cow::Borrowed(rest))) if std::ptr::eq(rest.as_ptr(), events.as_ptr()))
        );
    }
}
