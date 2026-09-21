//! Document-scoped frame wakeups; there are no retained document callbacks.

use puri::timer::{Completion, Instant, Timer};

#[derive(Default)]
pub(crate) struct Timers(Vec<Completion>);

impl puri::timer::Timers for Timers {
    fn schedule(&mut self, deadline: Instant) -> Timer {
        self.0.retain(|completion| completion.deadline().is_some());
        let (timer, completion) = Timer::new(deadline);
        self.0.push(completion);
        timer
    }
}

impl Timers {
    pub fn deadline(&self) -> Option<Instant> {
        self.0.iter().filter_map(Completion::deadline).min()
    }

    pub fn fire_due(&mut self, now: Instant) -> bool {
        let mut fired = false;
        self.0.retain(|completion| match completion.deadline() {
            Some(deadline) if now >= deadline => {
                fired |= completion.fire();
                false
            }
            Some(_) => true,
            None => false,
        });
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::timer::{Duration, Timers as _};

    #[test]
    fn early_wakeup_retains_the_earliest_live_deadline() {
        let now = Instant::now();
        let mut timers = Timers::default();
        let early = timers.schedule(now + Duration::from_millis(100));
        let late = timers.schedule(now + Duration::from_millis(200));
        assert!(!timers.fire_due(now + Duration::from_millis(99)));
        assert_eq!(timers.deadline(), early.deadline());
        assert!(timers.fire_due(now + Duration::from_millis(100)));
        assert!(early.deadline().is_none());
        assert_eq!(timers.deadline(), late.deadline());
        assert!(timers.fire_due(now + Duration::from_millis(300)));
        assert!(late.deadline().is_none());
        assert!(timers.deadline().is_none());
        assert!(!timers.fire_due(now + Duration::from_millis(300)));
    }

    #[test]
    fn replacing_requests_does_not_accumulate_or_deliver_cancelled_timers() {
        let now = Instant::now();
        let mut timers = Timers::default();
        let mut debounce = puri_widgets::debounce::Debounce::default();
        for n in 0..1000 {
            debounce.trigger(
                &mut timers,
                now + Duration::from_millis(n),
                Duration::from_millis(150),
            );
            assert_eq!(timers.0.len(), 1);
        }
        assert!(!timers.fire_due(now + Duration::from_millis(150)));
        drop(debounce);
        assert!(timers.deadline().is_none());
        assert!(!timers.fire_due(now + Duration::from_secs(2)));
        assert!(timers.0.is_empty());
    }
}
