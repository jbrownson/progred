//! Watching an evaluation as it runs. The evaluator offers an [`Observer`]
//! each call it enters and leaves; with none, it keeps no stack at all. A
//! [`Tracer`] keeps the calls that have a source, to capture as a
//! [`CallTrace`] whenever it's asked, and an [`Answering`] watches for the
//! call that answered a particular value.

use crate::{ByNode, Definition, Expression, Host, RuntimeValue, SourceOrigin};
use gid::{CellId, Resolution};
use std::cell::RefCell;
use std::rc::Rc;

/// Told of each call an evaluation enters and leaves, in order, and what
/// each answered: none where it halted.
pub trait Observer {
    fn entered(&self, call: &Expression);
    fn left(&self, answer: Option<&RuntimeValue>);
}

/// `host`, its evaluations watched by `observer`.
pub struct Observed<'a> {
    host: &'a dyn Host,
    observer: &'a dyn Observer,
}

impl<'a> Observed<'a> {
    pub fn new(host: &'a dyn Host, observer: &'a dyn Observer) -> Self {
        Self { host, observer }
    }
}

impl Host for Observed<'_> {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)> {
        self.host.resolve(cell)
    }

    fn untracked(&self) {
        self.host.untracked()
    }

    fn effect(&self) {
        self.host.effect()
    }

    fn observer(&self) -> Option<&dyn Observer> {
        Some(self.observer)
    }
}

/// Captured source-bearing calls, innermost first. Nodes share caller ancestry
/// and source locations, but retain neither environments nor evaluator storage.
#[derive(Clone, Debug)]
pub struct CallTrace(pub(crate) Rc<CallTraceNode>);

#[derive(Debug)]
pub(crate) struct CallTraceNode {
    pub(crate) origin: Rc<SourceOrigin>,
    pub(crate) caller: Option<CallTrace>,
}

impl CallTrace {
    pub fn origins(&self) -> impl Iterator<Item = &SourceOrigin> {
        std::iter::successors(Some(self), |trace| trace.0.caller.as_ref())
            .map(|trace| trace.0.origin.as_ref())
    }
}

impl PartialEq for CallTrace {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.origins().eq(other.origins())
    }
}

impl Eq for CallTrace {}

/// Keeps the calls an evaluation is in, to capture them as a [`CallTrace`].
#[derive(Default)]
pub struct Tracer {
    calls: RefCell<Vec<ActiveCall>>,
    origins: RefCell<ByNode<Option<Rc<SourceOrigin>>>>,
}

struct ActiveCall {
    expression: Expression,
    // The outer option means this prefix has been captured, even if none of
    // its calls has an origin (e.g. a host-generated call).
    captured: Option<Option<CallTrace>>,
}

impl Observer for Tracer {
    fn entered(&self, call: &Expression) {
        self.calls.borrow_mut().push(ActiveCall {
            expression: call.clone(),
            captured: None,
        });
    }

    fn left(&self, _: Option<&RuntimeValue>) {
        self.calls.borrow_mut().pop();
    }
}

impl Tracer {
    /// Where the call the evaluation is innermost in is, if it has a source.
    fn innermost(&self) -> Option<Rc<SourceOrigin>> {
        let calls = self.calls.borrow();
        let call = &calls.last()?.expression;
        self.origin(call)
    }

    fn origin(&self, call: &Expression) -> Option<Rc<SourceOrigin>> {
        self.origins
            .borrow_mut()
            .entry(call.clone())
            .or_insert_with(|| crate::source::origin(call).map(Rc::new))
            .clone()
    }

    /// The calls the evaluation is in. Calls without a source are skipped,
    /// not assigned a fabricated path. Repeated captures share live prefixes.
    pub fn trace(&self) -> Option<CallTrace> {
        let mut calls = self.calls.borrow_mut();
        let (start, mut caller) = calls
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, call)| Some((index + 1, call.captured.clone()?)))
            .unwrap_or((0, None));
        for call in &mut calls[start..] {
            if let Some(origin) = self.origin(&call.expression) {
                caller = Some(CallTrace(Rc::new(CallTraceNode { origin, caller })));
            }
            call.captured = Some(caller.clone());
        }
        caller
    }
}

/// Watches for the first call at `call` that answers `value`, keeping the
/// calls it was made under, itself first: where an absent about that call
/// came from, found by running its evaluation again.
pub struct Answering {
    call: SourceOrigin,
    value: gid::Value,
    tracer: Tracer,
    found: RefCell<Option<CallTrace>>,
}

impl Answering {
    pub fn new(call: SourceOrigin, value: gid::Value) -> Self {
        Self {
            call,
            value,
            tracer: Tracer::default(),
            found: RefCell::default(),
        }
    }

    /// The calls the answer was made under, if a call at the place answered
    /// it.
    pub fn found(self) -> Option<CallTrace> {
        self.found.into_inner()
    }
}

impl Observer for Answering {
    fn entered(&self, call: &Expression) {
        self.tracer.entered(call);
    }

    fn left(&self, answer: Option<&RuntimeValue>) {
        if self.found.borrow().is_none()
            && self
                .tracer
                .innermost()
                .is_some_and(|origin| *origin == self.call)
            && answer.is_some_and(|answer| answer.to_value() == self.value)
        {
            *self.found.borrow_mut() = self.tracer.trace();
        }
        self.tracer.left(answer);
    }
}

#[cfg(test)]
#[path = "tests/trace.rs"]
mod tests;
