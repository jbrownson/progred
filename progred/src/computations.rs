//! The editor owns memo roots; libraries compose typed computations in them.

use crate::{library::Libraries, sources::Sources, workspace::Root};
use gid::{Document, Step, Value};
use grap::Host;
use incremental::background::{Executor, Tasks};
use incremental::{Input, Roots, Runtime, Source};
use std::{cell::Cell, rc::Rc};
use web_time::Instant;

pub(crate) mod memo;

#[derive(Clone)]
pub(crate) struct Snapshot {
    doc: Rc<Document>,
    libraries: Libraries,
}

pub(crate) struct Computations {
    pub runtime: Runtime,
    pub tasks: Tasks,
    pub pointer_pressed: Input<bool>,
    pub frame_time: Cell<Instant>,
    snapshot: Input<Snapshot>,
    pub definitions: memo::Definitions<Snapshot>,
    roots: Roots<(Root, Vec<Step>)>,
}

impl Default for Computations {
    fn default() -> Self {
        Self::new(Executor::inline(), || {})
    }
}

impl Computations {
    pub fn new(executor: Executor, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let runtime = Runtime::default();
        let tasks = Tasks::new(&runtime, executor, wake);
        Self::with_tasks(runtime, tasks)
    }

    fn with_tasks(runtime: Runtime, tasks: Tasks) -> Self {
        let snapshot = runtime.input(Snapshot {
            doc: Rc::new(Document::default()),
            libraries: Libraries::default(),
        });
        let definitions = Source::new(snapshot.clone(), |snapshot: &Snapshot, cell| {
            memo::Resolved(Host::resolve(
                &Sources {
                    doc: &snapshot.doc,
                    libraries: &snapshot.libraries,
                },
                *cell,
            ))
        });
        Self {
            pointer_pressed: runtime.input(false),
            frame_time: Cell::new(Instant::now()),
            runtime,
            tasks,
            snapshot,
            definitions,
            roots: Roots::default(),
        }
    }

    pub fn reset(&mut self) {
        let runtime = Runtime::default();
        let tasks = self.tasks.fresh(&runtime);
        *self = Self::with_tasks(runtime, tasks);
    }

    #[cfg(any(test, feature = "cam-profile"))]
    pub fn from_sources(sources: Sources<'_>) -> Self {
        let computations = Self::default();
        computations.begin(Rc::new(sources.doc.clone()), sources.libraries.clone());
        computations
    }

    pub fn begin(&self, doc: Rc<Document>, libraries: Libraries) {
        self.snapshot.set_by(Snapshot { doc, libraries }, |a, b| {
            Rc::ptr_eq(&a.doc, &b.doc) && a.libraries.same_storage(&b.libraries)
        });
        self.roots.begin();
    }

    pub fn at<T: 'static>(&self, view: &Root, path: &[Step], create: impl FnOnce() -> T) -> Rc<T> {
        self.roots.get((view.clone(), path.to_vec()), create)
    }

    #[cfg(test)]
    pub fn evaluate(&self, view: &Root, path: &[Step], expression: &Value, fuel: usize) -> Value {
        self.evaluate_runtime(view, path, &expression.into(), fuel, None)
            .into_value()
    }

    pub fn evaluate_runtime(
        &self,
        view: &Root,
        path: &[Step],
        expression: &grap::RuntimeValue,
        fuel: usize,
        origin: Option<grap::SourceOrigin>,
    ) -> grap::RuntimeValue {
        struct Evaluation {
            expression: Input<grap::RuntimeValue>,
            origin: Input<Option<grap::SourceOrigin>>,
            fuel: Input<usize>,
            result: incremental::Memo<grap::Evaluation>,
        }
        let evaluation = self.at(view, path, || {
            let expression = self.runtime.input(expression.clone());
            let origin = self.runtime.input(origin.clone());
            let fuel = self.runtime.input(fuel);
            let result = self.runtime.memo_by(
                {
                    let expression = expression.clone();
                    let origin = origin.clone();
                    let fuel = fuel.clone();
                    let definitions = self.definitions.clone();
                    move |read| {
                        let expression = expression.read(read);
                        let fuel = *fuel.read(read);
                        let origin = origin.read(read).as_ref().clone();
                        Ok(memo::run(&definitions, read, |host| {
                            grap::evaluate_runtime_at(&expression, origin, host, fuel)
                        }))
                    }
                },
                same_evaluation,
            );
            Evaluation {
                expression,
                origin,
                fuel,
                result,
            }
        });
        evaluation
            .expression
            .set_by(expression.clone(), grap::RuntimeValue::same_result);
        evaluation.origin.set(origin);
        evaluation.fuel.set(fuel);
        self.runtime
            .read(&evaluation.result)
            .map(|evaluation| evaluation.result.clone())
            .unwrap_or_else(|error| memo::failure(error).into())
    }

    #[cfg(test)]
    pub fn apply(
        &self,
        view: &Root,
        path: &[Step],
        function: &Value,
        arguments: &[(gid::CellId, Value)],
        fuel: usize,
    ) -> Value {
        self.apply_runtime(view, path, function, arguments, fuel)
            .into_value()
    }

    pub fn apply_runtime(
        &self,
        view: &Root,
        path: &[Step],
        function: &Value,
        arguments: &[(gid::CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue {
        struct Application {
            input: Input<(Value, Vec<(gid::CellId, Value)>, usize)>,
            result: incremental::Memo<grap::Evaluation>,
        }
        let input = (function.clone(), arguments.to_vec(), fuel);
        let application = self.at(view, path, || {
            let input = self.runtime.input(input.clone());
            let result = self.runtime.memo_by(
                {
                    let input = input.clone();
                    let definitions = self.definitions.clone();
                    move |read| {
                        let input = input.read(read);
                        let (function, arguments, fuel) = &*input;
                        Ok(memo::run(&definitions, read, |host| {
                            grap::apply_expression(
                                &function.into(),
                                arguments.iter().map(|(k, v)| (*k, v.into())),
                                host,
                                *fuel,
                            )
                        }))
                    }
                },
                same_evaluation,
            );
            Application { input, result }
        });
        application.input.set(input);
        self.runtime
            .read(&application.result)
            .map(|evaluation| evaluation.result.clone())
            .unwrap_or_else(|error| memo::failure(error).into())
    }
}

fn same_evaluation(a: &grap::Evaluation, b: &grap::Evaluation) -> bool {
    a.completed == b.completed
        && a.remaining_fuel == b.remaining_fuel
        && a.result.same_result(&b.result)
}

#[cfg(test)]
mod tests;
