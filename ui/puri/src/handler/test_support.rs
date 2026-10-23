//! One-event dispatch shorthands over [`Handler::dispatch`], for tests here
//! and in dependent crates.

use super::{Event, EventOutcome, Handler, ImeEvent};
use std::borrow::Cow;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerInfo, PointerScrollEvent, PointerUpdate};

/// Each shorthand reports whether a handler took its event, except
/// `dispatch_scroll`, which answers the outcome with its remainder. Those
/// without an `input` dispatch with the default.
pub trait DispatchShorthands<C, P> {
    fn dispatch_pointer_down(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool
    where
        P: Default;

    fn dispatch_pointer_down_with(
        &self,
        ctx: &mut C,
        event: &PointerButtonEvent,
        input: &mut P,
    ) -> bool;

    fn dispatch_pointer_move(&self, ctx: &mut C, event: &PointerUpdate) -> bool
    where
        P: Default;

    fn dispatch_pointer_up(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool
    where
        P: Default;

    fn dispatch_pointer_cancel(&self, ctx: &mut C, event: &PointerInfo) -> bool
    where
        P: Default;

    fn dispatch_scroll<'a>(&self, ctx: &mut C, event: &'a PointerScrollEvent) -> EventOutcome<'a>
    where
        P: Default;

    fn dispatch_key(&self, ctx: &mut C, event: &KeyboardEvent) -> bool
    where
        P: Default;

    fn dispatch_key_with(&self, ctx: &mut C, event: &KeyboardEvent, input: &mut P) -> bool;

    fn dispatch_ime(&self, ctx: &mut C, event: &ImeEvent) -> bool
    where
        P: Default;
}

impl<C, P> DispatchShorthands<C, P> for Handler<C, P> {
    fn dispatch_pointer_down(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool
    where
        P: Default,
    {
        self.dispatch_pointer_down_with(ctx, event, &mut P::default())
    }

    fn dispatch_pointer_down_with(
        &self,
        ctx: &mut C,
        event: &PointerButtonEvent,
        input: &mut P,
    ) -> bool {
        self.dispatch(ctx, Event::PointerDown(event), input)
            .handled()
    }

    fn dispatch_pointer_move(&self, ctx: &mut C, event: &PointerUpdate) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::PointerMove(event), &mut P::default())
            .handled()
    }

    fn dispatch_pointer_up(&self, ctx: &mut C, event: &PointerButtonEvent) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::PointerUp(event), &mut P::default())
            .handled()
    }

    fn dispatch_pointer_cancel(&self, ctx: &mut C, event: &PointerInfo) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::PointerCancel(event), &mut P::default())
            .handled()
    }

    fn dispatch_scroll<'a>(&self, ctx: &mut C, event: &'a PointerScrollEvent) -> EventOutcome<'a>
    where
        P: Default,
    {
        self.dispatch(
            ctx,
            Event::Scroll(Cow::Borrowed(std::slice::from_ref(event))),
            &mut P::default(),
        )
    }

    fn dispatch_key(&self, ctx: &mut C, event: &KeyboardEvent) -> bool
    where
        P: Default,
    {
        self.dispatch_key_with(ctx, event, &mut P::default())
    }

    fn dispatch_key_with(&self, ctx: &mut C, event: &KeyboardEvent, input: &mut P) -> bool {
        self.dispatch(ctx, Event::Key(event), input).handled()
    }

    fn dispatch_ime(&self, ctx: &mut C, event: &ImeEvent) -> bool
    where
        P: Default,
    {
        self.dispatch(ctx, Event::Ime(event), &mut P::default())
            .handled()
    }
}
