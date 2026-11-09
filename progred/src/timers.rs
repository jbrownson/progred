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
#[path = "tests/timers.rs"]
mod tests;
