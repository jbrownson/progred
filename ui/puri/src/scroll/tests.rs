use super::*;
use ui_events::pointer::{PointerInfo, PointerState, PointerType};

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
