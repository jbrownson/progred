//! Watching an evaluation as it runs. The evaluator offers an [`Observer`]
//! each call it enters and leaves; with none, it keeps no stack at all. A
//! [`Tracer`] keeps the calls that have a source, to capture as a
//! [`CallTrace`] whenever it's asked.

use crate::{ByNode, Definition, Expression, Host, SourceOrigin};
use gid::{CellId, Resolution};
use std::cell::RefCell;
use std::rc::Rc;

/// Told of each call an evaluation enters and leaves, in order.
pub trait Observer {
    fn entered(&self, call: &Expression);
    fn left(&self);
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

    fn left(&self) {
        self.calls.borrow_mut().pop();
    }
}

impl Tracer {
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
            let origin = self
                .origins
                .borrow_mut()
                .entry(call.expression.clone())
                .or_insert_with(|| crate::source::origin(&call.expression).map(Rc::new))
                .clone();
            if let Some(origin) = origin {
                caller = Some(CallTrace(Rc::new(CallTraceNode { origin, caller })));
            }
            call.captured = Some(caller.clone());
        }
        caller
    }
}
