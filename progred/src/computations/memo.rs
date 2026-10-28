//! Whole-evaluation memo boundaries. Each execution owns a fresh interpreter,
//! so prepared calls cannot bypass an enclosing computation's observations.
//! The evaluation's host records each cell it resolves, and gives up
//! reuse when Grap reports an untracked call or an effect the memo doesn't own.

use gid::{CellId, Value};
use grap::{Evaluation, ForeignFunction, Host, absent};
use incremental::{Read, Source};
use std::cell::RefCell;

#[cfg(test)]
#[path = "tests/memo.rs"]
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

/// What the host says of a cell: its definition, and the code the foreign
/// function implemented under it runs.
#[derive(Clone, PartialEq)]
pub struct Resolved {
    pub value: Option<Value>,
    pub implementation: Option<ForeignFunction>,
}

pub type Definitions<S> = Source<S, CellId, Resolved>;

struct ObservingHost<'a, S> {
    definitions: &'a Definitions<S>,
    read: RefCell<&'a mut Read>,
    recorded_effects: bool,
}

impl<S: 'static> Host for ObservingHost<'_, S> {
    fn definition(&self, cell: CellId) -> Option<Value> {
        self.definitions
            .read(cell, &mut self.read.borrow_mut())
            .value
    }
    fn implementation(&self, cell: CellId) -> Option<ForeignFunction> {
        self.definitions
            .read(cell, &mut self.read.borrow_mut())
            .implementation
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
