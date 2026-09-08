//! Explicitly enabled, thread-local diagnostic scopes. No retained frame state.
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum Kind {
    Other,
    Projection,
    LineEdit,
    Delimiter,
    Text,
    Drawing,
    Decoration,
    Row,
    Column,
    Padding,
    Overlay,
    Program,
    Shared,
    Completion,
    Choices,
    Placement,
    Hover,
    Paint,
    Disposal,
}

pub const KINDS: [Kind; 19] = [
    Kind::Other,
    Kind::Projection,
    Kind::LineEdit,
    Kind::Delimiter,
    Kind::Text,
    Kind::Drawing,
    Kind::Decoration,
    Kind::Row,
    Kind::Column,
    Kind::Padding,
    Kind::Overlay,
    Kind::Program,
    Kind::Shared,
    Kind::Completion,
    Kind::Choices,
    Kind::Placement,
    Kind::Hover,
    Kind::Paint,
    Kind::Disposal,
];

#[derive(Clone, Copy, Default)]
pub struct Cost {
    pub time: Duration,
    pub calls: usize,
    pub allocations: usize,
    pub bytes: usize,
}

struct State {
    kind: Kind,
    since: Instant,
    costs: [Cost; KINDS.len()],
}

impl State {
    fn charge(&mut self, now: Instant) {
        self.costs[self.kind as usize].time += now - self.since;
        self.since = now;
    }
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn begin() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert!(state.is_none(), "profiling scopes cannot nest captures");
        *state = Some(State {
            kind: Kind::Other,
            since: Instant::now(),
            costs: [Cost::default(); KINDS.len()],
        });
    });
}

pub fn finish() -> [Cost; KINDS.len()] {
    STATE.with(|state| {
        let mut state = state.borrow_mut().take().expect("active profiling capture");
        state.charge(Instant::now());
        state.costs
    })
}

pub struct Scope(Option<Kind>);

pub fn enter(kind: Kind) -> Scope {
    Scope(STATE.with(|state| {
        state.borrow_mut().as_mut().map(|state| {
            state.charge(Instant::now());
            state.costs[kind as usize].calls += 1;
            std::mem::replace(&mut state.kind, kind)
        })
    }))
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(prior) = self.0 {
            STATE.with(|state| {
                if let Some(state) = state.borrow_mut().as_mut() {
                    state.charge(Instant::now());
                    state.kind = prior;
                }
            });
        }
    }
}

/// Called by the headless test allocator; never allocate or panic here.
pub fn allocated(bytes: usize) {
    let _ = STATE.try_with(|state| {
        if let Ok(mut state) = state.try_borrow_mut()
            && let Some(state) = state.as_mut()
        {
            let cost = &mut state.costs[state.kind as usize];
            cost.allocations += 1;
            cost.bytes += bytes;
        }
    });
}
