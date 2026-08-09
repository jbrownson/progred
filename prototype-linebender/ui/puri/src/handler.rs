//! The frame's second output: a `Handler` — one composed dispatch
//! function per event kind — collected during placement. The caller
//! retains it and dispatches events into it; because it is a pure
//! function of the state its pass read, it is single-shot: the first
//! handled (mutating) event spends it, and the caller mints a
//! successor from the mutated state before the next event dispatches.
//! Unhandled events leave it standing. Dispatch geometry therefore
//! always matches the presented frame, and dispatches must decline on
//! absent state for any window where mutation happens outside
//! dispatch.
//!
//! Dispatches receive the caller's context `C` by `&mut` (state is
//! passed in, never closed over) and do their effects directly. The
//! pass itself stays read-only; all mutation happens in dispatch,
//! after placement completes. Composition is function composition:
//! `on_*` wraps the existing function so the newest dispatch tries
//! first and declines fall through. Widgets gate by their own settled
//! rects inline — there is no region registry — and a parent scopes
//! its children with [`capture`], receiving their handler as a value
//! (destructure it to wrap the channels) it can call, wrap with
//! before/after behavior, transform events for, or drop.

use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent, PointerUpdate};

/// Text composition events, mirroring winit's `Ime` (which bypasses
/// ui-events); the shell converts.
pub enum ImeEvent {
    Enabled,
    Disabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
}

pub struct Handler<C> {
    pub pointer_down: Box<dyn Fn(&mut C, &PointerButtonEvent) -> bool>,
    pub pointer_move: Box<dyn Fn(&mut C, &PointerUpdate) -> bool>,
    pub pointer_up: Box<dyn Fn(&mut C, &PointerButtonEvent) -> bool>,
    pub scroll: Box<dyn Fn(&mut C, &PointerScrollEvent) -> bool>,
    pub key: Box<dyn Fn(&mut C, &KeyboardEvent) -> bool>,
    pub ime: Box<dyn Fn(&mut C, &ImeEvent) -> bool>,
}

impl<C> Default for Handler<C> {
    fn default() -> Self {
        Self {
            pointer_down: Box::new(|_, _| false),
            pointer_move: Box::new(|_, _| false),
            pointer_up: Box::new(|_, _| false),
            scroll: Box::new(|_, _| false),
            key: Box::new(|_, _| false),
            ime: Box::new(|_, _| false),
        }
    }
}

/// Wrap `slot` so `dispatch` tries first and declines fall through.
fn compose<C: 'static, E: 'static>(
    slot: &mut Box<dyn Fn(&mut C, &E) -> bool>,
    dispatch: impl Fn(&mut C, &E) -> bool + 'static,
) {
    let rest = std::mem::replace(slot, Box::new(|_, _| false));
    *slot = Box::new(move |ctx, event| dispatch(ctx, event) || rest(ctx, event));
}

impl<C> Handler<C> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compose a dispatch in front: it tries first; on decline the
    /// event falls through to what was already registered. A dispatch
    /// returning `false` should leave the context unchanged.
    pub fn on_pointer_down(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) where
        C: 'static,
    {
        compose(&mut self.pointer_down, dispatch);
    }

    pub fn on_key(&mut self, dispatch: impl Fn(&mut C, &KeyboardEvent) -> bool + 'static)
    where
        C: 'static,
    {
        compose(&mut self.key, dispatch);
    }

    pub fn on_pointer_move(&mut self, dispatch: impl Fn(&mut C, &PointerUpdate) -> bool + 'static)
    where
        C: 'static,
    {
        compose(&mut self.pointer_move, dispatch);
    }

    pub fn on_pointer_up(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) where
        C: 'static,
    {
        compose(&mut self.pointer_up, dispatch);
    }

    pub fn on_scroll(&mut self, dispatch: impl Fn(&mut C, &PointerScrollEvent) -> bool + 'static)
    where
        C: 'static,
    {
        compose(&mut self.scroll, dispatch);
    }

    pub fn on_ime(&mut self, dispatch: impl Fn(&mut C, &ImeEvent) -> bool + 'static)
    where
        C: 'static,
    {
        compose(&mut self.ime, dispatch);
    }

    pub fn dispatch_pointer_down(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool {
        (self.pointer_down)(ctx, event)
    }

    pub fn dispatch_pointer_move(&self, ctx: &mut C, event: &PointerUpdate) -> bool {
        (self.pointer_move)(ctx, event)
    }

    pub fn dispatch_pointer_up(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool {
        (self.pointer_up)(ctx, event)
    }

    pub fn dispatch_scroll(&self, ctx: &mut C, event: &PointerScrollEvent) -> bool {
        (self.scroll)(ctx, event)
    }

    pub fn dispatch_key(&self, ctx: &mut C, event: &KeyboardEvent) -> bool {
        (self.key)(ctx, event)
    }

    pub fn dispatch_ime(&self, ctx: &mut C, event: &ImeEvent) -> bool {
        (self.ime)(ctx, event)
    }
}

/// Implemented by placement contexts that carry a handler, so widgets
/// can scope their children with [`capture`].
pub trait HasHandler<C> {
    fn handler(&mut self) -> &mut Handler<C>;
}

impl<C> HasHandler<C> for Handler<C> {
    fn handler(&mut self) -> &mut Handler<C> {
        self
    }
}

/// Runs `place_children`, capturing everything it registers into a
/// fresh handler returned as a value. The caller decides how — and
/// whether — the captured dispatches ever run.
pub fn capture<C, P: HasHandler<C> + ?Sized>(
    p: &mut P,
    place_children: impl FnOnce(&mut P),
) -> Handler<C> {
    let saved = std::mem::take(p.handler());
    place_children(p);
    std::mem::replace(p.handler(), saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Point, Rect};
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
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
        let Handler {
            pointer_down: inner_pointer,
            ..
        } = inner;
        handler.on_pointer_down(move |log, event| {
            log.push("before");
            let handled = inner_pointer(log, event);
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
}
