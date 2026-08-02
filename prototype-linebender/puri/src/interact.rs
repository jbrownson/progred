//! Interaction combinators: wrap a layout node so its settled rect
//! registers a transient handler. Pure like the rest of Puri — the
//! callback runs in dispatch, after placement, receiving the context
//! by `&mut`; nothing is retained across frames.

use crate::handler::HasHandler;
use crate::layout::{Node, Placement, before};
use kurbo::Point;
use ui_events::pointer::{PointerButton, PointerButtonEvent};

/// Attach a primary-button press whose predicate and action both see
/// the settled placement. A false action declines to the handler
/// composed behind this node.
pub fn on_primary_pointer_down_where<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    accepts: impl Fn(Placement, &PointerButtonEvent) -> bool + 'static,
    action: impl Fn(&mut C, Placement, &PointerButtonEvent) -> bool + 'static,
) -> Node<P> {
    before(node, move |p, placement| {
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(
                    event.state.position.x,
                    event.state.position.y,
                ))
                && accepts(placement, event)
                && action(ctx, placement, event)
        });
    })
}

/// Attach a primary-button press whose policy needs the pointer event
/// but not the settled placement.
pub fn on_primary_pointer_down<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    accepts: impl Fn(&PointerButtonEvent) -> bool + 'static,
    action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
) -> Node<P> {
    on_primary_pointer_down_where(
        node,
        move |_, event| accepts(event),
        move |ctx, _, event| action(ctx, event),
    )
}

/// Attach a primary click selected by click count.
pub fn on_primary_click<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    accepts: impl Fn(u8) -> bool + 'static,
    action: impl Fn(&mut C) -> bool + 'static,
) -> Node<P> {
    on_primary_pointer_down(
        node,
        move |event| accepts(event.state.count.max(1)),
        move |ctx, _| action(ctx),
    )
}

/// Wrap `node` so a primary-button press inside its settled rect runs
/// `on_click`. The handler registers before the wrapped subtree places,
/// so a child's own handler (registered later, tried first) takes
/// precedence and a press it declines falls through to here.
pub fn clickable<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    on_click: impl Fn(&mut C) + 'static,
) -> Node<P> {
    on_primary_click(node, |_| true, move |ctx| {
        on_click(ctx);
        true
    })
}

/// The double-click specialization. Compose it inside `clickable` so
/// its later registration wins on the second press.
pub fn double_clickable<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    on_double_click: impl Fn(&mut C) + 'static,
) -> Node<P> {
    on_primary_click(node, |count| count == 2, move |ctx| {
        on_double_click(ctx);
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::Handler;
    use crate::layout::{Extent, Placement, leaf, place, place_top_left};
    use ui_events::pointer::{
        PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
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

    fn down_at_count(x: f64, y: f64, count: u8) -> PointerButtonEvent {
        let mut event = down_at(x, y);
        event.state.count = count;
        event
    }

    struct Frame {
        handler: Handler<u32>,
    }

    impl HasHandler<u32> for Frame {
        fn handler(&mut self) -> &mut Handler<u32> {
            &mut self.handler
        }
    }

    /// A 10x10 clickable at the origin that sets the selected id to 7.
    fn placed(viewport: Option<kurbo::Rect>) -> Handler<u32> {
        let node = clickable(
            leaf(
                Extent {
                    width: 10.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                |_: &mut Frame, _| {},
            ),
            |sel: &mut u32| *sel = 7,
        );
        let mut frame = Frame {
            handler: Handler::new(),
        };
        let rect = node.extent.rect_at(Point::ZERO);
        place(
            node,
            &mut frame,
            match viewport {
                Some(clip_rect) => Placement::new(rect, clip_rect),
                None => Placement::root(rect),
            },
        );
        frame.handler
    }

    #[test]
    fn press_inside_fires() {
        let frame = placed(None);
        let mut selected = 0;
        assert!(frame.dispatch_pointer_down(&mut selected, &down_at(5.0, 5.0)));
        assert_eq!(selected, 7);
    }

    #[test]
    fn press_outside_falls_through() {
        let frame = placed(None);
        let mut selected = 0;
        assert!(!frame.dispatch_pointer_down(&mut selected, &down_at(50.0, 50.0)));
        assert_eq!(selected, 0);
    }

    #[test]
    fn press_in_the_clipped_part_falls_through() {
        let frame = placed(Some(kurbo::Rect::new(0.0, 0.0, 4.0, 10.0)));
        let mut selected = 0;
        assert!(!frame.dispatch_pointer_down(&mut selected, &down_at(7.0, 5.0)));
        assert_eq!(selected, 0);
        assert!(frame.dispatch_pointer_down(&mut selected, &down_at(3.0, 5.0)));
        assert_eq!(selected, 7);
    }

    #[test]
    fn double_click_overrides_the_ordinary_click() {
        let node = clickable(
            double_clickable(
                leaf(
                    Extent {
                        width: 10.0,
                        ascent: 8.0,
                        descent: 2.0,
                    },
                    |_: &mut Frame, _| {},
                ),
                |value| *value += 10,
            ),
            |value| *value += 1,
        );
        let mut frame = Frame {
            handler: Handler::new(),
        };
        place_top_left(node, &mut frame, Point::ZERO);

        let mut value = 0;
        assert!(frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at_count(5.0, 5.0, 2)));
        assert_eq!(value, 10);
    }

    #[test]
    fn placed_predicate_can_subdivide_the_clip_rect() {
        let node = on_primary_pointer_down_where(
            leaf(
                Extent {
                    width: 10.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                |_: &mut Frame, _| {},
            ),
            |placement, event| event.state.position.x >= placement.rect.center().x,
            |value, _, _| {
                *value = 7;
                true
            },
        );
        let mut frame = Frame {
            handler: Handler::new(),
        };
        place_top_left(node, &mut frame, Point::ZERO);

        let mut value = 0;
        assert!(!frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at(4.0, 5.0)));
        assert!(frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at(6.0, 5.0)));
        assert_eq!(value, 7);
    }
}
