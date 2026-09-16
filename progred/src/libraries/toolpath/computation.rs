//! Dependency-tracked path generation, shared by the rendering interpretations.

use super::{paths::Recording, run};
use crate::{computations::Computations, libraries::absent};
use gid::Value;
use incremental::{Input, Memo};
use std::sync::Arc;

pub(super) type Outcome<T> = Result<T, Value>;

#[derive(PartialEq)]
pub(super) struct Recorded {
    pub path: Arc<Recording>,
    pub evaluation: ::grap::Evaluation,
}

impl Recorded {
    pub fn path(&self) -> Outcome<&Recording> {
        if self.evaluation.completed && !absent::is_absent(&self.evaluation.result) {
            Ok(&self.path)
        } else {
            Err(self.evaluation.result.clone())
        }
    }
}

pub(super) fn recording(
    computations: &Computations,
    program: Input<Value>,
    fuel: Input<usize>,
) -> Memo<Recorded> {
    let definitions = computations.definitions.clone();
    computations.runtime.memo(move |read| {
        let program = program.read(read);
        let fuel = *fuel.read(read);
        let mut path = Recording::default();
        let evaluation = ::grap::memo::with_recorded_effects(&definitions, read, |host| {
            run(&mut path, |scope| {
                ::grap::apply_scoped(&program, [], host, scope, fuel)
            })
        });
        Ok(Recorded {
            path: Arc::new(path),
            evaluation,
        })
    })
}
