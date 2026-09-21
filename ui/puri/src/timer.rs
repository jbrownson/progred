//! Caller-supplied one-shot timers. Dropping the handle cancels delivery.

use std::{
    cell::Cell,
    rc::{Rc, Weak},
};
pub use web_time::{Duration, Instant};

pub trait Timers {
    fn schedule(&mut self, deadline: Instant) -> Timer;
}

pub struct Timer(Rc<Cell<Option<Instant>>>);

/// The shell retains only this weak delivery capability, never widget state.
pub struct Completion(Weak<Cell<Option<Instant>>>);

impl Timer {
    pub fn new(deadline: Instant) -> (Self, Completion) {
        let state = Rc::new(Cell::new(Some(deadline)));
        let completion = Completion(Rc::downgrade(&state));
        (Self(state), completion)
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.0.get()
    }
}

impl Completion {
    pub fn deadline(&self) -> Option<Instant> {
        self.0.upgrade().and_then(|state| state.get())
    }

    /// Deliver at most once. A cancelled timer has no recipient.
    pub fn fire(&self) -> bool {
        self.0.upgrade().is_some_and(|state| state.take().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_is_one_shot_and_dropping_the_handle_cancels_it() {
        let now = Instant::now();
        let (timer, completion) = Timer::new(now);
        assert!(completion.fire());
        assert!(timer.deadline().is_none());
        assert!(!completion.fire());
        let (timer, cancelled) = Timer::new(now);
        drop(timer);
        assert!(cancelled.deadline().is_none());
        assert!(!cancelled.fire());
    }
}
