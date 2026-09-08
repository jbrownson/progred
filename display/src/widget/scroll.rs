use super::HoverCallback;
use puri::handler::{HasHandler, ScrollOutcome};
use puri::{Point, Size, Vec2};

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
mod tests {
    use super::*;
    use puri::handler::{
        Event, PointerInfo, PointerScrollEvent, PointerState, PointerType, ScrollDelta,
    };
    use puri::{Placement, Rect};

    fn event(delta: ScrollDelta) -> PointerScrollEvent {
        PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState::default(),
            delta,
        }
    }

    #[test]
    fn acceptance_does_not_require_a_state_write() {
        for accepts in [false, true] {
            let mut frame = crate::widget::Fragment::default();
            let mut output =
                crate::widget::HoverContext::<(), ()>::new(Default::default(), &mut frame);
            scroll(1.0, move |_, delta| {
                if accepts {
                    ScrollOutcome::with_remainder(Vec2::ZERO)
                } else {
                    ScrollOutcome::unhandled(delta)
                }
            })(
                &mut output,
                Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)),
            );
            let event = event(ScrollDelta::LineDelta(0.0, 1.0));
            let outcome = frame.handler.unwrap().dispatch_scroll(&mut (), &event);
            assert_eq!(outcome.handled(), accepts);
            match outcome.remaining {
                None => assert!(accepts),
                Some(Event::Scroll(remaining)) => {
                    assert!(!accepts);
                    assert_eq!(remaining.delta, event.delta);
                }
                _ => panic!("unexpected scroll remainder"),
            }
        }
    }

    #[test]
    fn partial_consumption_round_trips_all_units() {
        for (delta, expected, remaining) in [
            (
                ScrollDelta::LineDelta(2.0, 4.0),
                Vec2::new(80.0, 160.0),
                ScrollDelta::LineDelta(2.0, 2.0),
            ),
            (
                ScrollDelta::PageDelta(2.0, 4.0),
                Vec2::new(20.0, 60.0),
                ScrollDelta::PageDelta(2.0, 2.0),
            ),
            (
                ScrollDelta::PixelDelta((2.0, 4.0).into()),
                Vec2::new(1.0, 2.0),
                ScrollDelta::PixelDelta((2.0, 2.0).into()),
            ),
        ] {
            let mut frame = crate::widget::Fragment::default();
            let mut output =
                crate::widget::HoverContext::<(), ()>::new(Default::default(), &mut frame);
            scroll(2.0, move |_, input| {
                assert_eq!(input, expected);
                ScrollOutcome::with_remainder(Vec2::new(input.x, input.y / 2.0))
            })(
                &mut output,
                Placement::root(Rect::new(0.0, 0.0, 20.0, 30.0)),
            );
            let event = event(delta);
            let outcome = frame.handler.unwrap().dispatch_scroll(&mut (), &event);
            assert!(outcome.handled());
            let Some(Event::Scroll(result)) = outcome.remaining else {
                panic!("expected unconsumed scroll");
            };
            assert_eq!(result.delta, remaining);
        }
    }

    #[test]
    fn nested_handlers_receive_only_unused_scroll_and_respect_clipping() {
        let mut frame = crate::widget::Fragment::default();
        let mut output =
            crate::widget::HoverContext::<Vec<Vec2>, ()>::new(Default::default(), &mut frame);
        let outer = Placement::root(Rect::new(0.0, 0.0, 100.0, 100.0));
        scroll(2.0, |log: &mut Vec<Vec2>, delta| {
            log.push(delta);
            ScrollOutcome::with_remainder(Vec2::ZERO)
        })(&mut output, outer);
        scroll(2.0, |log: &mut Vec<Vec2>, delta| {
            log.push(delta);
            ScrollOutcome::with_remainder(Vec2::new(delta.x, delta.y / 2.0))
        })(
            &mut output,
            Placement::new(
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Rect::new(0.0, 0.0, 10.0, 20.0),
            ),
        );
        let handler = frame.handler.unwrap();
        for (x, expected) in [
            (5.0, vec![Vec2::new(80.0, 160.0), Vec2::new(80.0, 80.0)]),
            (15.0, vec![Vec2::new(80.0, 160.0)]),
            (25.0, vec![Vec2::new(80.0, 160.0)]),
        ] {
            let mut event = event(ScrollDelta::LineDelta(2.0, 4.0));
            event.state.position = (x, 5.0).into();
            let mut log = vec![];
            let outcome = handler.dispatch_scroll(&mut log, &event);
            assert!(outcome.handled());
            assert!(outcome.remaining.is_none());
            assert_eq!(log, expected);
        }
    }

    #[test]
    fn empty_viewports_keep_page_remainders_finite() {
        let original = ScrollDelta::PageDelta(2.0, 4.0);
        let outcome = units(2.0, Size::ZERO).handle(original, ScrollOutcome::unhandled);
        assert_eq!(outcome.remaining, original);
        assert!(!outcome.handled());
    }
}
