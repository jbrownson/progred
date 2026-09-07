//! One composable function over input events. Later handlers run first;
//! unconsumed input continues to earlier handlers. Widgets close over their
//! description and receive caller-owned state and dispatch inputs explicitly.

use std::borrow::Cow;
pub use ui_events::ScrollDelta;
pub use ui_events::keyboard::{KeyState, KeyboardEvent, Modifiers};
pub use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
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
    pub fn with_remainder(remaining: Remainder) -> Self {
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
    pub fn into_event<'a>(self, original: Cow<'a, PointerScrollEvent>) -> EventOutcome<'a> {
        if self.remaining == Self::consume(&original).remaining {
            self.map(|_| None)
        } else if self.remaining == original.delta {
            self.map(|_| Some(Event::Scroll(original)))
        } else {
            let remaining = self.event(&original);
            self.map(|_| remaining.map(|event| Event::Scroll(Cow::Owned(event))))
        }
    }

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

/// Text composition events, mirroring winit's Ime.
pub enum ImeEvent {
    Enabled,
    Disabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
}

/// Input, not editor actions. A scroll remainder may own its adjusted packet;
/// ordinary dispatch borrows the platform packet without copying it.
#[derive(Clone)]
pub enum Event<'a> {
    PointerDown(&'a PointerButtonEvent),
    PointerMove(&'a PointerUpdate),
    PointerUp(&'a PointerButtonEvent),
    PointerCancel(&'a PointerInfo),
    Scroll(Cow<'a, PointerScrollEvent>),
    Key(&'a KeyboardEvent),
    Ime(&'a ImeEvent),
}

impl<'a> EventOutcome<'a> {
    pub fn decline(event: Event<'a>) -> Self {
        Self::unhandled(Some(event))
    }

    pub fn accept() -> Self {
        Self::with_remainder(None)
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
pub struct Handler<C, P = ()>(Dispatch<C, P>);

impl<C, P> Default for Handler<C, P> {
    fn default() -> Self {
        Self(Box::new(|_, event, _| EventOutcome::decline(event)))
    }
}

impl<C, P> Handler<C, P> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_function(
        dispatch: impl for<'a> Fn(&mut C, Event<'a>, &mut P) -> EventOutcome<'a> + 'static,
    ) -> Self {
        Self(Box::new(dispatch))
    }

    pub fn dispatch<'a>(&self, ctx: &mut C, event: Event<'a>, input: &mut P) -> EventOutcome<'a> {
        (self.0)(ctx, event, input)
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
        self.dispatch(ctx, Event::Scroll(Cow::Borrowed(event)), &mut P::default())
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
        Self::from_function(move |ctx, event, input| {
            let outcome = above.dispatch(ctx, event, input);
            match outcome.remaining {
                Some(event) => {
                    let next = self.dispatch(ctx, event, input);
                    Outcome {
                        handled: outcome.handled || next.handled,
                        ..next
                    }
                }
                None => outcome,
            }
        })
    }

    pub fn on(
        &mut self,
        dispatch: impl for<'a> Fn(&mut C, Event<'a>, &mut P) -> EventOutcome<'a> + 'static,
    ) {
        *self = std::mem::take(self).over(Self::from_function(dispatch));
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

    pub fn on_scroll(
        &mut self,
        dispatch: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
    ) {
        self.on(move |ctx, event, _| match event {
            Event::Scroll(scroll) => dispatch(ctx, &scroll).into_event(scroll),
            other => EventOutcome::decline(other),
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

/// Capture child handlers as an ordinary value which a wrapper can transform.
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

    #[test]
    fn scroll_event_conversion_preserves_acceptance_without_a_phantom_remainder() {
        let zero = scroll(0.0);
        let outcome = ScrollOutcome::pass(&zero).into_event(Cow::Borrowed(&zero));
        assert!(!outcome.handled());
        assert!(outcome.remaining.is_none());
        let event = scroll(4.0);
        let outcome = ScrollOutcome::pass(&event).into_event(Cow::Borrowed(&event));
        assert!(!outcome.handled());
        assert!(matches!(
            outcome.remaining,
            Some(Event::Scroll(Cow::Borrowed(_)))
        ));
        let outcome = ScrollOutcome::with_remainder(ScrollDelta::LineDelta(0.0, 2.0))
            .into_event(Cow::Borrowed(&event));
        assert!(outcome.handled());
        assert!(
            matches!(outcome.remaining, Some(Event::Scroll(Cow::Owned(remaining)))
            if remaining.delta == ScrollDelta::LineDelta(0.0, 2.0))
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
            if rest.delta == ScrollDelta::LineDelta(0.0, 4.0)));
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
}
