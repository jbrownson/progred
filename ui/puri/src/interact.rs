//! Placement-time interaction helpers. The caller supplies settled
//! geometry; Puri registers transient behavior without depending on
//! the layout strategy that produced it.

use crate::geometry::Placement;
use crate::handler::HasHandler;
use kurbo::Point;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerGesture, PointerState, PointerType, PointerUpdate,
};

/// Pinch deltas are fractions of the current scale, independent of pixel density.
/// The caller owns the zoom policy. Other gestures and out-of-bounds samples pass on.
pub fn on_pinch<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    action: impl Fn(&mut C, f32) -> bool + 'static,
) {
    p.handler().on_gesture(move |ctx, event| {
        if let PointerGesture::Pinch(delta) = event.gesture {
            delta.is_finite()
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && action(ctx, delta)
        } else {
            false
        }
    });
}

/// Observed motion in arrival order, ending with the current sample.
/// Coalesced samples precede (and exclude) `current`; predictions are not input.
pub fn pointer_samples(event: &PointerUpdate) -> impl DoubleEndedIterator<Item = &PointerState> {
    event
        .coalesced
        .iter()
        .chain(std::iter::once(&event.current))
}

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

/// Register an ordinary primary click at `placement`.
pub fn clickable<C: 'static, P: HasHandler<C>>(
    p: &mut P,
    placement: Placement,
    on_click: impl Fn(&mut C) + 'static,
) {
    p.handler().on_pointer_down(move |ctx, event| {
        is_primary_contact(event)
            && placement.contains(Point::new(event.state.position.x, event.state.position.y))
            && {
                on_click(ctx);
                true
            }
    });
}

#[cfg(test)]
mod tests;
