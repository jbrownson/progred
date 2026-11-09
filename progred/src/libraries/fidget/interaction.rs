//! Per-preview scheduling policy, shared by camera handlers and async rendering.

use crate::computations::Computations;
use incremental::{Input, Memo};
use puri::timer::{Duration, Instant, Timers};
use puri_widgets::debounce::Debounce;
use std::{cell::RefCell, rc::Rc};

const ZOOM_QUIET: Duration = Duration::from_millis(150);

pub(crate) struct Interaction {
    zoom: RefCell<Debounce>,
    quiet: Input<bool>,
    pub permitted: Memo<bool>,
}

impl Interaction {
    pub fn new(computations: &Computations) -> Self {
        let quiet = computations.runtime.input(true);
        let permitted = computations.runtime.memo({
            let quiet = quiet.clone();
            let pressed = computations.pointer_pressed.clone();
            move |read| Ok(!*pressed.read(read) && *quiet.read(read))
        });
        Self {
            zoom: RefCell::default(),
            quiet,
            permitted,
        }
    }

    pub fn at(computations: &Computations, site: &crate::editing::Site) -> Rc<Self> {
        let state = site.memo(computations, || Self::new(computations));
        state.refresh(computations.frame_time.get());
        state
    }

    pub fn zoomed(&self, timers: &mut impl Timers, now: Instant) {
        self.zoom.borrow_mut().trigger(timers, now, ZOOM_QUIET);
        self.refresh(now);
    }

    pub fn refresh(&self, now: Instant) {
        // Time is sampled outside the memo; a missed delivery recovers on any frame.
        self.quiet.set(self.zoom.borrow().ready(now));
    }
}

#[cfg(test)]
#[path = "tests/interaction.rs"]
mod tests;
