use super::*;
use kurbo::{Point, Rect};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType,
};

fn down_at(x: f64, y: f64) -> PointerButtonEvent {
    let mut state = PointerState::default();
    state.position.x = x;
    state.position.y = y;
    PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state,
    }
}

fn scroll(y: f32) -> PointerScrollEvent {
    PointerScrollEvent {
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        delta: ScrollDelta::LineDelta(0.0, y),
        state: PointerState::default(),
    }
}

#[test]
fn empty_composition_preserves_the_existing_function() {
    let increment = 7;
    let handler = Handler::from_function(move |count: &mut usize, event, input: &mut usize| {
        *count += increment;
        *input += 1;
        EventOutcome::decline(event)
    });
    let function = std::ptr::from_ref(handler.0.as_deref().unwrap()).cast::<()>();
    let handler = Handler::new().over(handler).over(Handler::new());
    assert_eq!(
        function,
        std::ptr::from_ref(handler.0.as_deref().unwrap()).cast::<()>()
    );
    let event = KeyboardEvent::default();
    let mut count = 0;
    let mut input = 0;
    let outcome = handler.dispatch(&mut count, Event::Key(&event), &mut input);
    assert!(!outcome.handled());
    assert!(matches!(outcome.remaining, Some(Event::Key(key)) if std::ptr::eq(key, &event)));
    assert_eq!((count, input), (7, 1));
    assert!(Handler::<()>::new().over(Handler::new()).0.is_none());
}

#[test]
fn an_empty_handler_preserves_borrowed_input() {
    let event = scroll(4.0);
    let outcome = Handler::<()>::new().dispatch_scroll(&mut (), &event);
    assert!(!outcome.handled());
    assert!(matches!(outcome.remaining,
        Some(Event::Scroll(Cow::Borrowed(remaining))) if std::ptr::eq(remaining.as_ptr(), &event)
    ));
}

#[test]
fn scroll_handlers_receive_whole_batches_and_forward_ordered_remainders() {
    let events = [scroll(4.0), scroll(-8.0), scroll(12.0)];
    let mut handler: Handler<Vec<Vec<ScrollDelta>>> = Handler::new();
    handler.on_scroll_batch(|log, events| {
        log.push(events.iter().map(|event| event.delta).collect());
        EventOutcome::decline(Event::Scroll(events))
    });
    handler.on_scroll_batch(|log, events| {
        assert_eq!(events.len(), 3);
        log.push(events.iter().map(|event| event.delta).collect());
        crate::scroll::each(events, |event| {
            let ScrollDelta::LineDelta(x, y) = event.delta else {
                unreachable!()
            };
            if y < 0.0 {
                ScrollOutcome::consume(event)
            } else {
                ScrollOutcome::with_remainder(ScrollDelta::LineDelta(x, y / 2.0))
            }
        })
    });
    let mut log = Vec::new();
    let outcome = handler.dispatch(&mut log, Event::Scroll(Cow::Borrowed(&events)), &mut ());
    assert!(outcome.handled());
    assert_eq!(
        log,
        [
            vec![
                ScrollDelta::LineDelta(0.0, 4.0),
                ScrollDelta::LineDelta(0.0, -8.0),
                ScrollDelta::LineDelta(0.0, 12.0)
            ],
            vec![
                ScrollDelta::LineDelta(0.0, 2.0),
                ScrollDelta::LineDelta(0.0, 6.0)
            ]
        ]
    );
    let Some(Event::Scroll(remaining)) = outcome.remaining else {
        panic!("expected remainder")
    };
    assert_eq!(remaining[0].state, events[0].state);
    assert_eq!(remaining[1].state, events[2].state);
}

#[test]
fn scroll_event_conversion_preserves_acceptance_without_a_phantom_remainder() {
    let zero = scroll(0.0);
    let outcome = crate::scroll::each(
        Cow::Borrowed(std::slice::from_ref(&zero)),
        ScrollOutcome::pass,
    );
    assert!(!outcome.handled());
    assert!(outcome.remaining.is_none());
    let event = scroll(4.0);
    let outcome = crate::scroll::each(
        Cow::Borrowed(std::slice::from_ref(&event)),
        ScrollOutcome::pass,
    );
    assert!(!outcome.handled());
    assert!(matches!(
        outcome.remaining,
        Some(Event::Scroll(Cow::Borrowed(_)))
    ));
    let outcome = crate::scroll::each(Cow::Borrowed(std::slice::from_ref(&event)), |_| {
        ScrollOutcome::with_remainder(ScrollDelta::LineDelta(0.0, 2.0))
    });
    assert!(outcome.handled());
    assert!(
        matches!(outcome.remaining, Some(Event::Scroll(Cow::Owned(remaining)))
        if remaining[0].delta == ScrollDelta::LineDelta(0.0, 2.0))
    );
}

fn gated(
    rect: Rect,
    act: impl Fn(&mut Vec<&'static str>) + 'static,
) -> impl Fn(&mut Vec<&'static str>, &PointerButtonEvent) -> bool {
    move |log, event| {
        rect.contains(Point::new(event.state.position.x, event.state.position.y)) && {
            act(log);
            true
        }
    }
}

#[test]
fn newest_dispatch_wins_and_false_falls_through() {
    let mut handler: Handler<Vec<&'static str>> = Handler::new();
    handler.on_pointer_down(gated(Rect::new(0.0, 0.0, 100.0, 100.0), |log| {
        log.push("bottom");
    }));
    handler.on_pointer_down(gated(Rect::new(25.0, 25.0, 75.0, 75.0), |log| {
        log.push("top");
    }));
    handler.on_pointer_down(|_, _| false);

    let mut log = Vec::new();
    assert!(handler.dispatch_pointer_down(&mut log, &down_at(50.0, 50.0)));
    assert!(handler.dispatch_pointer_down(&mut log, &down_at(10.0, 10.0)));
    assert!(!handler.dispatch_pointer_down(&mut log, &down_at(200.0, 200.0)));
    assert_eq!(log, vec!["top", "bottom"]);
}

#[test]
fn scroll_remainder_flows_to_the_next_handler() {
    let mut handler: Handler<Vec<(&'static str, f32)>> = Handler::new();
    handler.on_scroll(|log, event| {
        let ScrollDelta::LineDelta(_, y) = event.delta else {
            unreachable!()
        };
        log.push(("outer", y));
        ScrollOutcome::consume(event)
    });
    handler.on_scroll(|log, event| {
        let ScrollDelta::LineDelta(x, y) = event.delta else {
            unreachable!()
        };
        log.push(("inner", y));
        ScrollOutcome::with_remainder(ScrollDelta::LineDelta(x, y / 2.0))
    });

    let event = scroll(4.0);
    let mut log = Vec::new();
    let outcome = handler.dispatch_scroll(&mut log, &event);
    assert!(outcome.handled());
    assert!(outcome.remaining.is_none());
    assert_eq!(log, [("inner", 4.0), ("outer", 2.0)]);
}

#[test]
fn ordinary_and_contextual_keys_share_precedence_and_the_current_input() {
    let mut handler: Handler<Vec<usize>, usize> = Handler::new();
    handler.on_key_with(|log, _, input| {
        log.push(*input);
        true
    });
    handler.on_key(|_, _| false);
    let mut log = Vec::new();
    assert!(handler.dispatch_key_with(&mut log, &KeyboardEvent::default(), &mut 7));
    assert!(handler.dispatch_key_with(&mut log, &KeyboardEvent::default(), &mut 11));
    handler.on_key(|log, _| {
        log.push(0);
        true
    });
    assert!(handler.dispatch_key_with(&mut log, &KeyboardEvent::default(), &mut 13));
    assert_eq!(log, [7, 11, 0]);
}

#[test]
fn channels_compose_independently() {
    let mut handler: Handler<Vec<&'static str>> = Handler::new();
    handler.on_pointer_down(|log, _| {
        log.push("pointer");
        true
    });
    handler.on_key(|log, event| {
        event.state.is_down() && {
            log.push("key");
            true
        }
    });

    let mut log = Vec::new();
    assert!(handler.dispatch_pointer_down(&mut log, &down_at(1.0, 1.0)));
    assert!(handler.dispatch_key(&mut log, &KeyboardEvent::default()));
    assert_eq!(log, vec!["pointer", "key"]);
}

#[test]
fn hover_and_modifier_notifications_use_the_ordinary_handler_chain() {
    let mut handler: Handler<Vec<&'static str>, usize> = Handler::new();
    handler.on(|log, event, input| {
        assert_eq!(*input, 7);
        match event {
            Event::HoverChanged => log.push("hover"),
            Event::ModifiersChanged(modifiers) => {
                assert!(modifiers.shift());
                log.push("modifiers");
            }
            _ => return EventOutcome::decline(event),
        }
        EventOutcome::accept()
    });
    handler.on_key(|_, _| panic!("not a key event"));
    handler.on(|log, event, _| {
        log.push("above");
        EventOutcome::decline(event)
    });
    let mut log = Vec::new();
    assert!(
        handler
            .dispatch(&mut log, Event::HoverChanged, &mut 7)
            .handled()
    );
    assert!(
        handler
            .dispatch(&mut log, Event::ModifiersChanged(&Modifiers::SHIFT), &mut 7)
            .handled()
    );
    assert_eq!(log, ["above", "hover", "above", "modifiers"]);
}

#[test]
fn a_generic_wrapper_forwards_every_event_and_current_input() {
    let mut child: Handler<Vec<usize>, usize> = Handler::new();
    child.on_key_with(|log, _, input| {
        log.push(*input);
        true
    });
    child.on_ime(|log, _| {
        log.push(99);
        true
    });
    let wrapper = Handler::from_function(move |log: &mut Vec<usize>, event, input| {
        log.push(1);
        let result = child.dispatch(log, event, input);
        log.push(2);
        result
    });
    let mut log = vec![];
    assert!(wrapper.dispatch_key_with(&mut log, &KeyboardEvent::default(), &mut 7));
    assert!(wrapper.dispatch_ime(&mut log, &ImeEvent::Commit("hello".into())));
    assert_eq!(log, [1, 7, 2, 1, 99, 2]);
}

#[test]
fn composition_preserves_acceptance_and_the_last_remainder() {
    let mut inner: Handler<Vec<f32>> = Handler::new();
    inner.on_scroll(|log, event| {
        let ScrollDelta::LineDelta(x, y) = event.delta else {
            panic!("expected lines")
        };
        log.push(y);
        ScrollOutcome::with_remainder(ScrollDelta::LineDelta(x, y / 2.0))
    });
    let mut outer = Handler::new();
    outer.on_scroll(|log: &mut Vec<f32>, event| {
        let ScrollDelta::LineDelta(_, y) = event.delta else {
            panic!("expected lines")
        };
        log.push(y);
        ScrollOutcome::pass(event)
    });
    let mut log = vec![];
    let event = scroll(8.0);
    let result = outer.over(inner).dispatch_scroll(&mut log, &event);
    assert!(result.handled());
    assert!(matches!(result.remaining, Some(Event::Scroll(rest))
        if rest[0].delta == ScrollDelta::LineDelta(0.0, 4.0)));
    assert_eq!(log, [8.0, 4.0]);
}

#[test]
fn consumed_scroll_never_reaches_the_next_handler_even_for_zero_input() {
    let mut handler: Handler<()> = Handler::new();
    handler.on_scroll(|_, _| panic!("consumed input propagated"));
    handler.on_scroll(|_, event| ScrollOutcome::consume(event));
    for delta in [0.0, 4.0] {
        let event = scroll(delta);
        let result = handler.dispatch_scroll(&mut (), &event);
        assert!(result.handled());
        assert!(result.remaining.is_none());
    }
}

#[test]
fn handler_composition_is_associative_and_empty_is_identity() {
    fn contribution(id: usize) -> Handler<Vec<usize>> {
        Handler::from_function(move |log: &mut Vec<usize>, event, _| {
            log.push(id);
            match event {
                Event::Key(_) if id == 2 => EventOutcome::accept(),
                other => EventOutcome::decline(other),
            }
        })
    }
    let left = contribution(1).over(contribution(2)).over(contribution(3));
    let right = contribution(1).over(contribution(2).over(contribution(3)));
    for handler in [left, right] {
        let handler = Handler::new().over(handler).over(Handler::new());
        let mut log = vec![];
        assert!(handler.dispatch_key(&mut log, &KeyboardEvent::default()));
        assert_eq!(log, [3, 2]);
        log.clear();
        assert!(!handler.dispatch_ime(&mut log, &ImeEvent::Enabled));
        assert_eq!(log, [3, 2, 1]);
    }
}

#[test]
#[ignore]
fn handler_dispatch_profile() {
    use std::{hint::black_box, time::Instant};
    let start = Instant::now();
    let mut handler: Handler<usize> = Handler::new();
    for _ in 0..512 {
        handler.on_pointer_down(|state, _| {
            black_box(state);
            false
        });
        handler.on_pointer_move(|state, _| {
            black_box(state);
            false
        });
        handler.on_pointer_up(|state, _| {
            black_box(state);
            false
        });
        handler.on_key(|state, _| {
            black_box(state);
            false
        });
        handler.on_ime(|state, _| {
            black_box(state);
            false
        });
        handler.on_scroll(|state, event| {
            black_box(state);
            ScrollOutcome::pass(event)
        });
    }
    let build = start.elapsed();
    let pointer = down_at(0.0, 0.0);
    let key = KeyboardEvent::default();
    let wheel = scroll(1.0);
    let mut state = 0;
    let start = Instant::now();
    for _ in 0..1000 {
        black_box(handler.dispatch_pointer_down(&mut state, &pointer));
        black_box(handler.dispatch_key(&mut state, &key));
        black_box(handler.dispatch_scroll(&mut state, &wheel));
    }
    eprintln!(
        "512 widgets, 6 event registrations each: build {build:?}, dispatch {:?}/event",
        start.elapsed() / 3000
    );
}

#[test]
fn captured_children_dispatch_through_their_wrapper() {
    let mut handler: Handler<Vec<&'static str>> = Handler::new();
    handler.on_pointer_down(gated(Rect::new(0.0, 0.0, 200.0, 200.0), |log| {
        log.push("outer");
    }));

    let inner = capture(&mut handler, |h| {
        h.on_pointer_down(gated(Rect::new(0.0, 0.0, 50.0, 50.0), |log| {
            log.push("child");
        }));
    });
    handler.on_pointer_down(move |log, event| {
        log.push("before");
        let handled = inner.dispatch_pointer_down(log, event);
        log.push("after");
        handled
    });

    let mut log = Vec::new();
    assert!(handler.dispatch_pointer_down(&mut log, &down_at(25.0, 25.0)));
    assert_eq!(log, vec!["before", "child", "after"]);

    log.clear();
    assert!(handler.dispatch_pointer_down(&mut log, &down_at(150.0, 150.0)));
    assert_eq!(log, vec!["before", "after", "outer"]);
}
