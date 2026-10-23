//! Preserve ordered samples while a bounded handler consumes part of a batch.

use crate::handler::Outcome;
use std::borrow::Cow;

pub(crate) fn filter<'a, T: Clone>(
    events: Cow<'a, [T]>,
    includes: impl Fn(&T) -> bool,
    mut handle: impl for<'b> FnMut(Cow<'b, [T]>) -> Outcome<Cow<'b, [T]>>,
) -> Outcome<Cow<'a, [T]>> {
    if events.iter().all(&includes) {
        handle(events)
    } else if !events.iter().any(&includes) {
        Outcome::unhandled(events)
    } else {
        let mut remaining = Vec::new();
        let mut handled = false;
        for run in events.chunk_by(|a, b| includes(a) == includes(b)) {
            if run.first().is_some_and(&includes) {
                let result = handle(Cow::Borrowed(run));
                handled |= result.handled();
                remaining.extend(result.remaining.into_owned());
            } else {
                remaining.extend_from_slice(run);
            }
        }
        Outcome::new(Cow::Owned(remaining), handled)
    }
}
