//! Dependency-tracked path generation, shared by the rendering interpretations.

use super::{paths::Recording, run};
use crate::computations::{Computations, memo};
use ::grap::RuntimeValue;
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
        if self.evaluation.completed && !self.evaluation.result.is_absent() {
            Ok(&self.path)
        } else {
            Err(self.evaluation.result.to_value())
        }
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
        return run(output, |scope| {
            ::grap::apply_expression(program, [], host, Some(scope), fuel)
        });
    }
    let mut evaluation = ::grap::Evaluation {
        result: RuntimeValue::record([]),
        remaining_fuel: fuel,
        completed: true,
    };
    let mut pending = vec![program.clone()];
    while let Some(program) = pending.pop() {
        if let Some(list) = program.list_values() {
            pending.extend(list.collect::<Vec<_>>().into_iter().rev());
        } else {
            let mut part = Recording::default();
            evaluation = run(&mut part, |scope| {
                ::grap::apply_expression(&program, [], host, Some(scope), evaluation.remaining_fuel)
            });
            if !evaluation.completed || evaluation.result.is_absent() {
                return evaluation;
            }
            output.append_part(part);
        }
    }
    evaluation
}

#[cfg(test)]
mod tests;
