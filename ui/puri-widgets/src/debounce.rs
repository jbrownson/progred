//! Caller-owned quiet period, released by delivery OR elapsed time.

use puri::timer::{Duration, Instant, Timer, Timers};

#[derive(Default)]
pub struct Debounce {
    pending: Option<Timer>,
}

impl Debounce {
    pub fn trigger(&mut self, timers: &mut impl Timers, now: Instant, delay: Duration) {
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
mod tests {
    use super::*;
    use puri::timer::Completion;

    #[derive(Default)]
    struct FakeTimers(Vec<Completion>);

    impl Timers for FakeTimers {
        fn schedule(&mut self, deadline: Instant) -> Timer {
            let (timer, completion) = Timer::new(deadline);
            self.0.push(completion);
            timer
        }
    }

    #[test]
    fn missing_delivery_recovers_at_the_deadline() {
        let now = Instant::now();
        let delay = Duration::from_millis(150);
        let mut timers = FakeTimers::default();
        let mut debounce = Debounce::default();
        assert!(debounce.ready(now));
        debounce.trigger(&mut timers, now, delay);
        assert!(!debounce.ready(now + delay - Duration::from_nanos(1)));
        assert!(debounce.ready(now + delay));
        assert!(debounce.ready(now + delay + delay));
    }

    #[test]
    fn current_delivery_releases_the_constraint_even_if_the_clock_is_early() {
        let now = Instant::now();
        let mut timers = FakeTimers::default();
        let mut debounce = Debounce::default();
        debounce.trigger(&mut timers, now, Duration::from_secs(1));
        assert!(!debounce.ready(now));
        assert!(timers.0[0].fire());
        assert!(debounce.ready(now));
        assert!(!timers.0[0].fire());
    }

    #[test]
    fn replacement_and_destruction_cancel_old_deliveries() {
        let now = Instant::now();
        let delay = Duration::from_millis(150);
        let mut timers = FakeTimers::default();
        let mut debounce = Debounce::default();
        debounce.trigger(&mut timers, now, delay);
        debounce.trigger(&mut timers, now + delay, delay);
        assert!(!timers.0[0].fire());
        assert!(!debounce.ready(now + delay));
        assert!(debounce.ready(now + delay + delay));
        drop(debounce);
        assert!(!timers.0[1].fire());
    }
}
