//! Dependency-tracked path generation, shared by the rendering interpretations.

use super::run;
use crate::computations::{Computations, memo};
use ::grap::RuntimeValue;
use cam::paths::Recording;
use gid::Value;
use incremental::{Input, Memo};
use std::sync::Arc;

pub(super) type Outcome<T> = Result<T, Value>;

pub(super) struct Recorded {
    pub path: Arc<Recording>,
    pub evaluation: ::grap::Evaluation,
}

impl Recorded {
    pub(super) fn same_result(&self, other: &Self) -> bool {
        self.path == other.path
            && self.evaluation.completed == other.evaluation.completed
            && self.evaluation.remaining_fuel == other.evaluation.remaining_fuel
            && self.evaluation.result.same_result(&other.evaluation.result)
    }

    pub fn path(&self) -> Outcome<&Recording> {
        if self.evaluation.completed && self.evaluation.result.absent_reason().is_none() {
            Ok(&self.path)
        } else {
            Err(self.evaluation.result.to_value())
        }
    }
}

/// A path preview's program and fuel, as the inputs a frame sets, and the
/// recording they make.
pub(super) struct Program {
    pub value: Input<RuntimeValue>,
    pub fuel: Input<usize>,
    pub recording: Memo<Recorded>,
}

impl Program {
    pub fn new(computations: &Computations, program: RuntimeValue, fuel: usize) -> Self {
        let value = computations.runtime.input(program);
        let fuel = computations.runtime.input(fuel);
        Self {
            recording: recording(computations, value.clone(), fuel.clone()),
            value,
            fuel,
        }
    }

    /// Set a frame's program and fuel; a program with the same result keeps
    /// its recording.
    pub fn update(&self, program: RuntimeValue, fuel: usize) {
        self.value.set_by(program, RuntimeValue::same_result);
        self.fuel.set(fuel);
    }
}

pub(super) fn recording(
    computations: &Computations,
    program: Input<RuntimeValue>,
    fuel: Input<usize>,
) -> Memo<Recorded> {
    let definitions = computations.definitions.clone();
    computations.runtime.memo_by(
        move |read| {
            let program = program.read(read);
            let fuel = *fuel.read(read);
            let mut path = Recording::default();
            let evaluation = memo::with_recorded_effects(&definitions, read, |host| {
                record_program(&program, host, fuel, &mut path)
            });
            Ok(Recorded {
                path: Arc::new(path),
                evaluation,
            })
        },
        Recorded::same_result,
    )
}

/// Interpret an ordinary nested list of zero-argument programs. A single
/// callable remains a one-part program. Fuel and dependency observation span
/// all leaves; each leaf records independently so focus never needs to rerun it.
fn record_program(
    program: &RuntimeValue,
    host: &dyn ::grap::Host,
    fuel: usize,
    output: &mut Recording,
) -> ::grap::Evaluation {
    if program.list_len().is_none() {
        record(program, host, fuel, output)
    } else {
        let start = ::grap::Evaluation {
            result: RuntimeValue::record([]),
            remaining_fuel: fuel,
            completed: true,
        };
        let (Ok(evaluation) | Err(evaluation)) = record_parts(program, host, start, output);
        evaluation
    }
}

/// Record each leaf of `program` in order, after `evaluation`, as a part of
/// `output`; a leaf that fails stops the rest and is the error.
fn record_parts(
    program: &RuntimeValue,
    host: &dyn ::grap::Host,
    evaluation: ::grap::Evaluation,
    output: &mut Recording,
) -> Result<::grap::Evaluation, ::grap::Evaluation> {
    match program.list_values() {
        Some(mut children) => children.try_fold(evaluation, |evaluation, child| {
            record_parts(&child, host, evaluation, output)
        }),
        None => {
            let mut part = Recording::default();
            let evaluation = record(program, host, evaluation.remaining_fuel, &mut part);
            if evaluation.completed && evaluation.result.absent_reason().is_none() {
                output.append_part(part);
                Ok(evaluation)
            } else {
                Err(evaluation)
            }
        }
    }
}

/// Run the zero-argument `program`, recording what it emits into `output`.
fn record(
    program: &RuntimeValue,
    host: &dyn ::grap::Host,
    fuel: usize,
    output: &mut Recording,
) -> ::grap::Evaluation {
    run(output, |scope| {
        ::grap::apply_expression(program, [], host, Some(scope), fuel)
    })
}

#[cfg(test)]
mod tests;
