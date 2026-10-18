//! Caller-owned quiet period, released by delivery OR elapsed time.

use puri::timer::{Duration, Instant, Timer, Timers};

#[derive(Default)]
pub struct Debounce {
    pending: Option<Timer>,
}

impl Debounce {
    pub fn trigger(&mut self, timers: &mut impl Timers, now: Instant, delay: Duration) {
        // Cancelled first, so the scheduler can let go of it while scheduling the new one.
        self.pending = None;
        self.pending = Some(timers.schedule(now + delay));
    }

    pub fn ready(&self, now: Instant) -> bool {
        self.pending
            .as_ref()
            .and_then(Timer::deadline)
            .is_none_or(|deadline| now >= deadline)
    }
}

#[cfg(test)]
mod tests;
