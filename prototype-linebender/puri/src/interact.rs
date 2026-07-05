//! Interaction combinators: wrap a layout node so its settled rect
//! registers a transient handler. Pure like the rest of Puri — the
//! callback runs in dispatch, after placement, receiving the context
//! by `&mut`; nothing is retained across frames.

use crate::handler::HasHandler;
use crate::layout::{Node, decorate};
use kurbo::Point;
use ui_events::pointer::PointerButton;

/// Wrap `node` so a primary-button press inside its settled rect runs
/// `on_click`. The handler registers before the wrapped subtree places,
/// so a child's own handler (registered later, tried first) takes
/// precedence and a press it declines falls through to here.
pub fn clickable<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    on_click: impl Fn(&mut C) + 'static,
) -> Node<P> {
    decorate(node, move |p, rect| {
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    on_click(ctx);
                    true
                }
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::Handler;
    use crate::layout::{Extent, leaf, place_top_left};
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

    /// A 10x10 clickable at the origin that sets the selected id to 7.
    fn placed() -> Handler<u32> {
        let node = clickable(
            leaf(
                Extent {
                    width: 10.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                |_: &mut Handler<u32>, _| {},
            ),
            |sel: &mut u32| *sel = 7,
        );
        let mut frame: Handler<u32> = Handler::new();
        place_top_left(node, &mut frame, Point::new(0.0, 0.0));
        frame
    }

    #[test]
    fn press_inside_fires() {
        let frame = placed();
        let mut selected = 0;
        assert!(frame.dispatch_pointer_down(&mut selected, &down_at(5.0, 5.0)));
        assert_eq!(selected, 7);
    }

    #[test]
    fn press_outside_falls_through() {
        let frame = placed();
        let mut selected = 0;
        assert!(!frame.dispatch_pointer_down(&mut selected, &down_at(50.0, 50.0)));
        assert_eq!(selected, 0);
    }
}
