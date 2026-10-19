//! One composable function over input events. Later handlers run first;
//! unconsumed input continues to earlier handlers. Widgets close over their
//! description and receive caller-owned state and dispatch inputs explicitly.

use std::borrow::Cow;
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events::pointer::{
    PointerButtonEvent, PointerGestureEvent, PointerInfo, PointerScrollEvent, PointerUpdate,
};

/// Acceptance is independent of whether state changed. The remainder can
/// represent part of an input, or the entire input when it was declined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome<Remainder> {
    pub remaining: Remainder,
    handled: bool,
}

pub type ScrollOutcome<Delta = ScrollDelta> = Outcome<Delta>;
pub type EventOutcome<'a> = Outcome<Option<Event<'a>>>;

impl<Remainder> Outcome<Remainder> {
    pub fn new(remaining: Remainder, handled: bool) -> Self {
        Self { remaining, handled }
    }

    pub fn handled_leaving(remaining: Remainder) -> Self {
        Self {
            remaining,
            handled: true,
        }
    }

    pub fn unhandled(remaining: Remainder) -> Self {
        Self {
            remaining,
            handled: false,
        }
    }

    pub fn map<Mapped>(self, map: impl FnOnce(Remainder) -> Mapped) -> Outcome<Mapped> {
        Outcome {
            remaining: map(self.remaining),
            handled: self.handled,
        }
    }

    pub fn handled(&self) -> bool {
        self.handled
    }
}

impl ScrollOutcome {
    pub fn pass(event: &PointerScrollEvent) -> Self {
        Self::unhandled(event.delta)
    }

    pub fn consume(event: &PointerScrollEvent) -> Self {
        Self::handled_leaving(match event.delta {
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
            pointer: original.pointer,
            delta: self.remaining,
            state: original.state.clone(),
        })
    }
}

/// Text composition events, mirroring winit's Ime.
pub enum ImeEvent {
    Enabled,
    Disabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Input, not editor actions. Scroll and gestures retain every observed packet
/// in order; a partially consumed batch may own its adjusted remainder.
#[derive(Clone)]
pub enum Event<'a> {
    PointerDown(&'a PointerButtonEvent),
    PointerMove(&'a PointerUpdate),
    PointerUp(&'a PointerButtonEvent),
    PointerCancel(&'a PointerInfo),
    Scroll(Cow<'a, [PointerScrollEvent]>),
    Gesture(Cow<'a, [PointerGestureEvent]>),
    Key(&'a KeyboardEvent),
    /// A caller translates an unclaimed directional key into navigation intent.
    Navigate(NavigationDirection),
    Ime(&'a ImeEvent),
    ModifiersChanged(&'a Modifiers),
    /// The caller settled a different hover target. Its dispatch input
    /// supplies the current target; Puri owns neither identity nor scheduling.
    HoverChanged,
}

impl<'a> EventOutcome<'a> {
    pub fn decline(event: Event<'a>) -> Self {
        Self::unhandled(Some(event))
    }

    pub fn accept() -> Self {
        Self::handled_leaving(None)
    }

    pub fn from_handled(event: Event<'a>, handled: bool) -> Self {
        if handled {
            Self::accept()
        } else {
            Self::decline(event)
        }
    }
}

type Dispatch<C, P> = Box<dyn for<'a> Fn(&mut C, Event<'a>, &mut P) -> EventOutcome<'a>>;

/// P is caller-owned frame data, supplied at dispatch rather than captured.
pub struct Handler<C, P = ()>(Option<Dispatch<C, P>>);

impl<C, P> Default for Handler<C, P> {
    fn default() -> Self {
        Self(None)
    }
}

impl<C, P> Handler<C, P> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_function(
        dispatch: impl for<'a> Fn(&mut C, Event<'a>, &mut P) -> EventOutcome<'a> + 'static,
    ) -> Self {
        Self(Some(Box::new(dispatch)))
    }

    pub fn dispatch<'a>(&self, ctx: &mut C, event: Event<'a>, input: &mut P) -> EventOutcome<'a> {
        match &self.0 {
            Some(dispatch) => dispatch(ctx, event, input),
            None => EventOutcome::decline(event),
        }
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
        input: &mut P,
    ) -> bool {
        self.dispatch(ctx, Event::PointerDown(event), input)
            .handled()
    }

    pub fn dispatch_pointer_move(&self, ctx: &mut C, event: &PointerUpdate) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::PointerMove(event), &mut P::default())
            .handled()
    }

    pub fn dispatch_pointer_up(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::PointerUp(event), &mut P::default())
            .handled()
    }

    pub fn dispatch_pointer_cancel(&self, ctx: &mut C, event: &PointerInfo) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::PointerCancel(event), &mut P::default())
            .handled()
    }

    pub fn dispatch_scroll<'a>(
        &self,
        ctx: &mut C,
        event: &'a PointerScrollEvent,
    ) -> EventOutcome<'a>
    where
        P: Default,
    {
        self.dispatch(
            ctx,
            Event::Scroll(Cow::Borrowed(std::slice::from_ref(event))),
            &mut P::default(),
        )
    }

    pub fn dispatch_key(&self, ctx: &mut C, event: &KeyboardEvent) -> bool
    where
        P: Default,
    {
        self.dispatch_key_with(ctx, event, &mut P::default())
    }

    pub fn dispatch_key_with(&self, ctx: &mut C, event: &KeyboardEvent, input: &mut P) -> bool {
        self.dispatch(ctx, Event::Key(event), input).handled()
    }

    pub fn dispatch_ime(&self, ctx: &mut C, event: &ImeEvent) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::Ime(event), &mut P::default())
            .handled()
    }
}

impl<C: 'static, P: 'static> Handler<C, P> {
    /// Later contributions run first, in the same order as painting.
    pub fn over(self, above: Self) -> Self {
        match (self.0, above.0) {
            (base, None) => Self(base),
            (None, above) => Self(above),
            (Some(base), Some(above)) => Self::from_function(move |ctx, event, input| {
                let outcome = above(ctx, event, input);
                match outcome.remaining {
                    Some(event) => {
                        let next = base(ctx, event, input);
                        Outcome {
                            handled: outcome.handled || next.handled,
                            ..next
                        }
                    }
                    None => outcome,
                }
            }),
        }
    }

    /// Stack `above` on this handler in place, as [`Self::over`] does.
    pub fn stack(&mut self, above: Self) {
        *self = std::mem::take(self).over(above);
    }

    pub fn on(
        &mut self,
        dispatch: impl for<'a> Fn(&mut C, Event<'a>, &mut P) -> EventOutcome<'a> + 'static,
    ) {
        self.stack(Self::from_function(dispatch));
    }

    pub fn on_pointer_down(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) {
        self.on_pointer_down_with(move |ctx, event, _| dispatch(ctx, event));
    }

    pub fn on_pointer_down_with(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerButtonEvent, &mut P) -> bool + 'static,
    ) {
        self.on(move |ctx, event, input| match event {
            Event::PointerDown(pointer) => EventOutcome::from_handled(
                Event::PointerDown(pointer),
                dispatch(ctx, pointer, input),
            ),
            other => EventOutcome::decline(other),
        });
    }

    pub fn on_key(&mut self, dispatch: impl Fn(&mut C, &KeyboardEvent) -> bool + 'static) {
        self.on_key_with(move |ctx, event, _| dispatch(ctx, event));
    }

    pub fn on_key_with(
        &mut self,
        dispatch: impl Fn(&mut C, &KeyboardEvent, &mut P) -> bool + 'static,
    ) {
        self.on(move |ctx, event, input| match event {
            Event::Key(key) => {
                EventOutcome::from_handled(Event::Key(key), dispatch(ctx, key, input))
            }
            other => EventOutcome::decline(other),
        });
    }

    pub fn on_pointer_move(&mut self, dispatch: impl Fn(&mut C, &PointerUpdate) -> bool + 'static) {
        self.on(move |ctx, event, _| match event {
            Event::PointerMove(pointer) => {
                EventOutcome::from_handled(Event::PointerMove(pointer), dispatch(ctx, pointer))
            }
            other => EventOutcome::decline(other),
        });
    }

    pub fn on_pointer_up(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) {
        self.on(move |ctx, event, _| match event {
            Event::PointerUp(pointer) => {
                EventOutcome::from_handled(Event::PointerUp(pointer), dispatch(ctx, pointer))
            }
            other => EventOutcome::decline(other),
        });
    }

    pub fn on_pointer_cancel(&mut self, dispatch: impl Fn(&mut C, &PointerInfo) -> bool + 'static) {
        self.on(move |ctx, event, _| match event {
            Event::PointerCancel(pointer) => {
                EventOutcome::from_handled(Event::PointerCancel(pointer), dispatch(ctx, pointer))
            }
            other => EventOutcome::decline(other),
        });
    }

    pub fn on_scroll_batch(
        &mut self,
        dispatch: impl for<'a> Fn(&mut C, Cow<'a, [PointerScrollEvent]>) -> EventOutcome<'a> + 'static,
    ) {
        self.on(move |ctx, event, _| match event {
            Event::Scroll(scroll) => dispatch(ctx, scroll),
            other => EventOutcome::decline(other),
        });
    }

    /// Opt into sample-wise handling, preserving partial remainders in order.
    pub fn on_scroll(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
    ) {
        self.on_scroll_batch(move |ctx, scroll| {
            crate::scroll::each(scroll, |event| dispatch(ctx, event))
        });
    }

    pub fn on_gesture_batch(
        &mut self,
        dispatch: impl for<'a> Fn(&mut C, Cow<'a, [PointerGestureEvent]>) -> EventOutcome<'a> + 'static,
    ) {
        self.on(move |ctx, event, _| match event {
            Event::Gesture(events) => dispatch(ctx, events),
            other => EventOutcome::decline(other),
        });
    }

    pub fn on_gesture(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerGestureEvent) -> bool + 'static,
    ) {
        self.on_gesture_batch(move |ctx, events| {
            crate::gesture::each(events, |event| dispatch(ctx, event))
        });
    }

    pub fn on_ime(&mut self, dispatch: impl Fn(&mut C, &ImeEvent) -> bool + 'static) {
        self.on(move |ctx, event, _| match event {
            Event::Ime(ime) => EventOutcome::from_handled(Event::Ime(ime), dispatch(ctx, ime)),
            other => EventOutcome::decline(other),
        });
    }
}

/// A placement output may expose its handler for widget composition.
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

#[cfg(test)]
mod tests;
