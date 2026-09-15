//! Preserve ordered samples while a bounded handler consumes part of a batch.

use crate::handler::Outcome;
use std::borrow::Cow;

pub(crate) fn filter<'a, T: Clone>(
    events: Cow<'a, [T]>,
    includes: impl Fn(&T) -> bool,
    mut handle: impl for<'b> FnMut(Cow<'b, [T]>) -> Outcome<Cow<'b, [T]>>,
) -> Outcome<Cow<'a, [T]>> {
    if events.iter().all(&includes) {
        return handle(events);
    }
    if !events.iter().any(&includes) {
        return Outcome::unhandled(events);
    }
    let mut remaining = Vec::new();
    let mut handled = false;
    for run in events.chunk_by(|a, b| includes(a) == includes(b)) {
        if includes(&run[0]) {
            let result = handle(Cow::Borrowed(run));
            handled |= result.handled();
            remaining.extend(result.remaining.into_owned());
        } else {
            remaining.extend_from_slice(run);
        }
    }
    if handled {
        Outcome::with_remainder(Cow::Owned(remaining))
    } else {
        Outcome::unhandled(Cow::Owned(remaining))
    }
}
