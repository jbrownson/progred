//! Placement-time interaction helpers. The caller supplies settled
//! geometry; Puri registers transient behavior without depending on
//! the layout strategy that produced it.

use crate::geometry::Placement;
use crate::handler::HasHandler;
use kurbo::Point;
use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerType, PointerUpdate};

/// Whether a button event represents the ordinary direct-contact
/// gesture: the primary mouse/pen button, or a touch contact (which
/// truthfully has no button in `ui-events`).
pub fn is_primary_contact(event: &PointerButtonEvent) -> bool {
    event.button == Some(PointerButton::Primary)
        || (event.button.is_none() && event.pointer.pointer_type == PointerType::Touch)
}

/// Whether a pointer move is continuing the ordinary direct-contact
/// gesture. Touch moves only arrive while that contact is active, and
/// therefore do not need a synthetic entry in `state.buttons`.
pub fn is_primary_contact_move(event: &PointerUpdate) -> bool {
    event.current.buttons.contains(PointerButton::Primary)
        || event.pointer.pointer_type == PointerType::Touch
}

/// Attach a primary-button press whose predicate and action both see
/// the settled placement. A false action declines to the handler
/// composed behind this registration.
pub fn on_primary_pointer_down_where<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    accepts: impl Fn(Placement, &PointerButtonEvent) -> bool + 'static,
    action: impl Fn(&mut C, Placement, &PointerButtonEvent) -> bool + 'static,
) {
    p.handler().on_pointer_down(move |ctx, event| {
        is_primary_contact(event)
            && placement.contains(Point::new(
                event.state.position.x,
                event.state.position.y,
            ))
            && accepts(placement, event)
            && action(ctx, placement, event)
    });
}

/// Attach a primary-button press whose policy needs the pointer event
/// but not the settled placement.
pub fn on_primary_pointer_down<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    accepts: impl Fn(&PointerButtonEvent) -> bool + 'static,
    action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
) {
    on_primary_pointer_down_where(
        p,
        placement,
        move |_, event| accepts(event),
        move |ctx, _, event| action(ctx, event),
    )
}

/// Attach a primary click selected by click count.
pub fn on_primary_click<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    accepts: impl Fn(u8) -> bool + 'static,
    action: impl Fn(&mut C) -> bool + 'static,
) {
    on_primary_pointer_down(
        p,
        placement,
        move |event| accepts(event.state.count.max(1)),
        move |ctx, _| action(ctx),
    )
}

/// Register an ordinary primary click at `placement`.
pub fn clickable<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    on_click: impl Fn(&mut C) + 'static,
) {
    on_primary_click(p, placement, |_| true, move |ctx| {
        on_click(ctx);
        true
    })
}

/// The double-click specialization. Register it after [`clickable`] so
/// newest-first composition gives it the second press.
pub fn double_clickable<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    on_double_click: impl Fn(&mut C) + 'static,
) {
    on_primary_click(p, placement, |count| count == 2, move |ctx| {
        on_double_click(ctx);
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::Handler;
    use kurbo::Rect;
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

    fn touch_at(x: f64, y: f64) -> PointerButtonEvent {
        let mut event = down_at(x, y);
        event.button = None;
        event.pointer.pointer_type = PointerType::Touch;
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
        let mut frame = Frame {
            handler: Handler::new(),
        };
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        let placement = match viewport {
            Some(clip_rect) => Placement::new(rect, clip_rect),
            None => Placement::root(rect),
        };
        clickable(
            &mut frame,
            placement,
            |sel: &mut u32| *sel = 7,
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
    fn touch_contact_fires_without_inventing_a_button() {
        let event = touch_at(5.0, 5.0);
        assert_eq!(event.button, None);
        let frame = placed(None);
        let mut selected = 0;
        assert!(frame.dispatch_pointer_down(&mut selected, &event));
        assert_eq!(selected, 7);
    }

    #[test]
    fn touch_move_is_an_active_contact_without_inventing_a_button() {
        let event = ui_events::pointer::PointerUpdate {
            pointer: touch_at(5.0, 5.0).pointer,
            current: PointerState::default(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        };
        assert!(event.current.buttons.is_empty());
        assert!(is_primary_contact_move(&event));
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
        let mut frame = Frame {
            handler: Handler::new(),
        };
        let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
        clickable(&mut frame, placement, |value| *value += 1);
        double_clickable(&mut frame, placement, |value| *value += 10);

        let mut value = 0;
        assert!(frame
            .handler
            .dispatch_pointer_down(&mut value, &down_at_count(5.0, 5.0, 2)));
        assert_eq!(value, 10);
    }

    #[test]
    fn placed_predicate_can_subdivide_the_clip_rect() {
        let mut frame = Frame {
            handler: Handler::new(),
        };
        on_primary_pointer_down_where(
            &mut frame,
            Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
            |placement, event| event.state.position.x >= placement.rect.center().x,
            |value, _, _| {
                *value = 7;
                true
            },
        );

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
