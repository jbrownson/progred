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

use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerInfo, PointerScrollEvent, PointerUpdate};

/// The part of a scroll event not accepted by this handler, plus
/// whether this handler accepted input, independently of state changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollOutcome<Delta = ScrollDelta> {
    pub remaining: Delta,
    handled: bool,
}

impl<Delta> ScrollOutcome<Delta> {
    pub fn with_remainder(delta: Delta) -> Self {
        Self {
            remaining: delta,
            handled: true,
        }
    }

    pub fn unhandled(delta: Delta) -> Self {
        Self {
            remaining: delta,
            handled: false,
        }
    }

    pub fn map<Mapped>(self, map: impl FnOnce(Delta) -> Mapped) -> ScrollOutcome<Mapped> {
        ScrollOutcome {
            remaining: map(self.remaining),
            handled: self.handled,
        }
    }

    pub fn handled(&self) -> bool {
        self.handled
    }

    pub fn followed_by(self, mut next: Self) -> Self {
        next.handled |= self.handled;
        next
    }
}

impl ScrollOutcome {
    pub fn pass(event: &PointerScrollEvent) -> Self {
        Self::unhandled(event.delta)
    }

    pub fn consume(event: &PointerScrollEvent) -> Self {
        Self::with_remainder(match event.delta {
            ScrollDelta::PageDelta(_, _) => ScrollDelta::PageDelta(0.0, 0.0),
            ScrollDelta::LineDelta(_, _) => ScrollDelta::LineDelta(0.0, 0.0),
            ScrollDelta::PixelDelta(_) => ScrollDelta::PixelDelta(Default::default()),
        })
    }

    pub fn event(self, original: &PointerScrollEvent) -> Option<PointerScrollEvent> {
        let empty = match self.remaining {
            ScrollDelta::PageDelta(x, y) | ScrollDelta::LineDelta(x, y) => x == 0.0 && y == 0.0,
            ScrollDelta::PixelDelta(delta) => delta.x == 0.0 && delta.y == 0.0,
        };
        (!empty).then(|| PointerScrollEvent {
            pointer: original.pointer.clone(),
            delta: self.remaining,
            state: original.state.clone(),
        })
    }
}

/// Text composition events, mirroring winit's `Ime` (which bypasses
/// ui-events); the shell converts.
pub enum ImeEvent {
    Enabled,
    Disabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
}

/// `P` is optional caller-owned dispatch input. Pointer starts and keys
/// may use settled frame data without changing ordinary widgets.
pub struct Handler<C, P = ()> {
    pub pointer_down: Box<dyn Fn(&mut C, &PointerButtonEvent, &mut P) -> bool>,
    pub pointer_move: Box<dyn Fn(&mut C, &PointerUpdate) -> bool>,
    pub pointer_up: Box<dyn Fn(&mut C, &PointerButtonEvent) -> bool>,
    pub pointer_cancel: Box<dyn Fn(&mut C, &PointerInfo) -> bool>,
    pub scroll: Box<dyn Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome>,
    pub key: Box<dyn Fn(&mut C, &KeyboardEvent, &mut P) -> bool>,
    pub ime: Box<dyn Fn(&mut C, &ImeEvent) -> bool>,
}

impl<C, P> Default for Handler<C, P> {
    fn default() -> Self {
        Self {
            pointer_down: Box::new(|_, _, _| false),
            pointer_move: Box::new(|_, _| false),
            pointer_up: Box::new(|_, _| false),
            pointer_cancel: Box::new(|_, _| false),
            scroll: Box::new(|_, event| ScrollOutcome::pass(event)),
            key: Box::new(|_, _, _| false),
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

fn compose_with<C: 'static, E: 'static, P: 'static>(
    slot: &mut Box<dyn Fn(&mut C, &E, &mut P) -> bool>,
    dispatch: impl Fn(&mut C, &E, &mut P) -> bool + 'static,
) {
    let rest = std::mem::replace(slot, Box::new(|_, _, _| false));
    *slot =
        Box::new(move |ctx, event, input| dispatch(ctx, event, input) || rest(ctx, event, input));
}

fn compose_scroll<C: 'static>(
    slot: &mut Box<dyn Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome>,
    dispatch: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) {
    let rest = std::mem::replace(slot, Box::new(|_, event| ScrollOutcome::pass(event)));
    *slot = Box::new(move |ctx, event| {
        let outcome = dispatch(ctx, event);
        match outcome.event(event) {
            Some(event) => outcome.followed_by(rest(ctx, &event)),
            None => outcome,
        }
    });
}

impl<C, P> Handler<C, P> {
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
        P: 'static,
    {
        self.on_pointer_down_with(move |ctx, event, _| dispatch(ctx, event));
    }

    /// Register with explicit caller-owned pointer dispatch context. This
    /// composes in exactly the same order as an ordinary pointer handler.
    pub fn on_pointer_down_with(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerButtonEvent, &mut P) -> bool + 'static,
    ) where
        C: 'static,
        P: 'static,
    {
        compose_with(&mut self.pointer_down, dispatch);
    }

    pub fn on_key(&mut self, dispatch: impl Fn(&mut C, &KeyboardEvent) -> bool + 'static)
    where
        C: 'static,
        P: 'static,
    {
        self.on_key_with(move |ctx, event, _| dispatch(ctx, event));
    }

    pub fn on_key_with(
        &mut self,
        dispatch: impl Fn(&mut C, &KeyboardEvent, &mut P) -> bool + 'static,
    ) where
        C: 'static,
        P: 'static,
    {
        compose_with(&mut self.key, dispatch);
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

    pub fn on_pointer_cancel(&mut self, dispatch: impl Fn(&mut C, &PointerInfo) -> bool + 'static)
    where
        C: 'static,
    {
        compose(&mut self.pointer_cancel, dispatch);
    }

    pub fn on_scroll(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
    ) where
        C: 'static,
    {
        compose_scroll(&mut self.scroll, dispatch);
    }

    pub fn on_ime(&mut self, dispatch: impl Fn(&mut C, &ImeEvent) -> bool + 'static)
    where
        C: 'static,
    {
        compose(&mut self.ime, dispatch);
    }

    pub fn dispatch_pointer_down(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool
    where
        P: Default,
    {
        self.dispatch_pointer_down_with(ctx, event, &mut P::default())
    }

    pub fn dispatch_pointer_down_with(
        &self,
        ctx: &mut C,
        event: &PointerButtonEvent,
        pointer: &mut P,
    ) -> bool {
        (self.pointer_down)(ctx, event, pointer)
    }

    pub fn dispatch_pointer_move(&self, ctx: &mut C, event: &PointerUpdate) -> bool {
        (self.pointer_move)(ctx, event)
    }

    pub fn dispatch_pointer_up(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool {
        (self.pointer_up)(ctx, event)
    }

    pub fn dispatch_pointer_cancel(&self, ctx: &mut C, event: &PointerInfo) -> bool {
        (self.pointer_cancel)(ctx, event)
    }

    pub fn dispatch_scroll(&self, ctx: &mut C, event: &PointerScrollEvent) -> ScrollOutcome {
        (self.scroll)(ctx, event)
    }

    pub fn dispatch_key(&self, ctx: &mut C, event: &KeyboardEvent) -> bool
    where
        P: Default,
    {
        self.dispatch_key_with(ctx, event, &mut P::default())
    }

    pub fn dispatch_key_with(&self, ctx: &mut C, event: &KeyboardEvent, input: &mut P) -> bool {
        (self.key)(ctx, event, input)
    }

    pub fn dispatch_ime(&self, ctx: &mut C, event: &ImeEvent) -> bool {
        (self.ime)(ctx, event)
    }
}

/// Implemented by placement contexts that carry a handler, so widgets
/// can scope their children with [`capture`].
pub trait HasHandler<C> {
    type Input: 'static;

    fn handler(&mut self) -> &mut Handler<C, Self::Input>;
}

impl<C, P: 'static> HasHandler<C> for Handler<C, P> {
    type Input = P;

    fn handler(&mut self) -> &mut Handler<C, P> {
        self
    }
}

/// Runs `place_children`, capturing everything it registers into a
/// fresh handler returned as a value. The caller decides how — and
/// whether — the captured dispatches ever run.
pub fn capture<C, P: HasHandler<C> + ?Sized>(
    p: &mut P,
    place_children: impl FnOnce(&mut P),
) -> Handler<C, P::Input> {
    let saved = std::mem::take(p.handler());
    place_children(p);
    std::mem::replace(p.handler(), saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Point, Rect};
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent,
        PointerState, PointerType,
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
        assert_eq!(outcome.remaining, ScrollDelta::LineDelta(0.0, 0.0));
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
            let handled = inner_pointer(log, event, &mut ());
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
