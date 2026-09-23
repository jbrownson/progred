//! Whole-evaluation memo boundaries. Each execution owns a fresh interpreter,
//! so prepared calls cannot bypass an enclosing computation's observations.

use super::*;
use incremental::{Input, Memo, Observation, Read, Runtime, Source};

#[cfg(test)]
mod tests;

pub const CYCLE: CellId = CellId::from_u128(0xd32978a2ca119d053a8d326e116375d3);
pub const INPUTS_CHANGED: CellId = CellId::from_u128(0x4f2d7df8e896810c4548a19499287fd6);
pub const DIFFERENT_RUNTIME: CellId = CellId::from_u128(0xc1002346d2d5952691235144fe988434);
pub const CANCELLED: CellId = CellId::from_u128(0xa57cb003db4a1653f9f554ec8dd76821);

pub fn failure(error: incremental::Error) -> Value {
    absent::value(match error {
        incremental::Error::Cycle => CYCLE,
        incremental::Error::InputsChanged => INPUTS_CHANGED,
        incremental::Error::DifferentRuntime => DIFFERENT_RUNTIME,
        incremental::Error::Cancelled => CANCELLED,
    })
}

#[derive(Clone)]
pub struct Resolved(pub Option<(Resolution, Definition)>);

impl PartialEq for Resolved {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some((a, Definition::Value(x))), Some((b, Definition::Value(y)))) => a == b && x == y,
            (Some((a, Definition::Foreign(x))), Some((b, Definition::Foreign(y)))) => {
                a == b && Rc::ptr_eq(x, y)
            }
            _ => false,
        }
    }
}

pub type Definitions<S> = Source<S, CellId, Resolved>;

struct ObservingHost<'a, S> {
    definitions: &'a Definitions<S>,
    read: RefCell<&'a mut Read>,
    recorded_effects: bool,
}

impl<S: 'static> Host for ObservingHost<'_, S> {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)> {
        self.definitions.read(cell, &mut self.read.borrow_mut()).0
    }
    fn observe(&self, observation: Observation) {
        self.read.borrow_mut().record(observation);
    }
    fn untracked(&self) {
        self.read.borrow_mut().untracked();
    }
    fn effect(&self) {
        if !self.recorded_effects {
            self.read.borrow_mut().untracked();
        }
    }
}

pub fn run<S: 'static, T>(
    definitions: &Definitions<S>,
    read: &mut Read,
    evaluate: impl FnOnce(&dyn Host) -> Evaluation<T>,
) -> Evaluation<T> {
    observed(definitions, read, false, evaluate)
}

/// The caller owns and returns all effects as part of the memo result. This
/// permits local recording, never unrecorded writes into an enclosing sink.
pub fn with_recorded_effects<S: 'static, T>(
    definitions: &Definitions<S>,
    read: &mut Read,
    evaluate: impl FnOnce(&dyn Host) -> Evaluation<T>,
) -> Evaluation<T> {
    observed(definitions, read, true, evaluate)
}

fn observed<S: 'static, T>(
    definitions: &Definitions<S>,
    read: &mut Read,
    recorded_effects: bool,
    evaluate: impl FnOnce(&dyn Host) -> Evaluation<T>,
) -> Evaluation<T> {
    let result = evaluate(&ObservingHost {
        definitions,
        read: RefCell::new(read),
        recorded_effects,
    });
    if !result.completed {
        read.untracked();
    }
    result
}

pub fn evaluate<S: 'static>(
    runtime: &Runtime,
    definitions: Definitions<S>,
    expression: Input<Value>,
    fuel: Input<usize>,
) -> Memo<Evaluation<Value>> {
    runtime.memo(move |read| {
        let expression = expression.read(read);
        let fuel = *fuel.read(read);
        Ok(run(&definitions, read, |host| {
            super::evaluate_value(&expression, host, fuel)
        }))
    })
}
