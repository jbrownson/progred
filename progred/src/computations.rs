//! The editor owns memo roots; libraries compose typed computations in them.

use crate::occurrence::Trail;
use crate::{library::Libraries, sources::Sources};
use gid::{Document, Value};
use grap::Host;
use incremental::background::{Executor, Tasks};
use incremental::{Input, Roots, Runtime, Source};
use std::{cell::Cell, rc::Rc};
use web_time::Instant;

pub(crate) mod memo;

#[derive(Clone)]
pub struct Snapshot {
    doc: Rc<Document>,
    libraries: Libraries,
}

pub struct Computations {
    pub runtime: Runtime,
    /// Jobs run off the editor's thread, when the host has somewhere to run
    /// them: a browser page without shared memory has nowhere.
    pub tasks: Option<Tasks>,
    pub pointer_pressed: Input<bool>,
    pub frame_time: Cell<Instant>,
    snapshot: Input<Snapshot>,
    pub definitions: memo::Definitions<Snapshot>,
    /// Each occurrence's computations, by its path, which also names its
    /// view.
    roots: Roots<Trail>,
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
        Self::with_tasks(runtime, Some(tasks))
    }

    /// Computations for a host with nowhere to run background jobs.
    #[cfg(any(test, all(target_arch = "wasm32", not(target_feature = "atomics"))))]
    pub fn without_tasks() -> Self {
        Self::with_tasks(Runtime::default(), None)
    }

    fn with_tasks(runtime: Runtime, tasks: Option<Tasks>) -> Self {
        let snapshot = runtime.input(Snapshot {
            doc: Rc::new(Document::default()),
            libraries: Libraries::default(),
        });
        let definitions = Source::new(snapshot.clone(), |snapshot: &Snapshot, cell| {
            let sources = Sources {
                doc: &snapshot.doc,
                libraries: &snapshot.libraries,
            };
            memo::Resolved {
                value: sources.definition(*cell),
                implementation: sources.implementation(*cell),
            }
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
        let tasks = self.tasks.as_ref().map(|tasks| tasks.fresh(&runtime));
        *self = Self::with_tasks(runtime, tasks);
    }

    /// Takes in what background jobs reported since the last poll; whether
    /// anything changed.
    pub fn poll(&self) -> bool {
        self.tasks.as_ref().is_some_and(Tasks::poll)
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

    pub fn at<T: 'static>(&self, path: &Trail, create: impl FnOnce() -> T) -> Rc<T> {
        self.roots.get(path.clone(), create)
    }

    pub fn evaluate_runtime(
        &self,
        path: &Trail,
        expression: &grap::RuntimeValue,
        fuel: usize,
        origin: Option<grap::SourceOrigin>,
    ) -> grap::RuntimeValue {
        self.memoized(
            path,
            (expression.clone(), origin, fuel),
            |a, b| a.0.same_result(&b.0) && a.1 == b.1 && a.2 == b.2,
            |(expression, origin, fuel), host| {
                grap::evaluate_runtime_at(expression, origin.clone(), host, *fuel)
            },
        )
    }

    pub fn apply_runtime(
        &self,
        path: &Trail,
        function: &Value,
        arguments: &[(gid::CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue {
        self.memoized(
            path,
            (function.clone(), arguments.to_vec(), fuel),
            PartialEq::eq,
            |(function, arguments, fuel), host| {
                grap::apply_expression(
                    &function.into(),
                    arguments.iter().map(|(k, v)| (*k, v.into())),
                    host,
                    None,
                    *fuel,
                )
            },
        )
    }

    /// The calls under which a call at `call` answered `value` in
    /// `evaluated`, itself first, found by running it again with an observer,
    /// kept for the occurrence at `path` as its own evaluation is.
    pub fn answering(
        &self,
        path: &Trail,
        evaluated: &crate::display::Evaluated,
        call: &grap::SourceOrigin,
        value: &Value,
    ) -> Option<Vec<grap::SourceOrigin>> {
        type Question = (crate::display::Evaluated, grap::SourceOrigin, Value);
        struct Answering {
            question: Input<Question>,
            answer: incremental::Memo<Option<Vec<grap::SourceOrigin>>>,
        }
        let question: Question = (evaluated.clone(), call.clone(), value.clone());
        let answering = self.at(path, || {
            let question = self.runtime.input(question.clone());
            let answer = self.runtime.memo_by(
                {
                    let question = question.clone();
                    let definitions = self.definitions.clone();
                    move |read| {
                        let question = question.read(read);
                        let (evaluated, call, value) = &*question;
                        let answering = grap::Answering::new(call.clone(), value.clone());
                        memo::run(&definitions, read, |host| {
                            grap::evaluate_runtime_at(
                                &evaluated.expression,
                                evaluated.origin.clone(),
                                &grap::Observed::new(host, &answering),
                                evaluated.fuel,
                            )
                        });
                        Ok(answering
                            .found()
                            .map(|trace| trace.origins().cloned().collect()))
                    }
                },
                PartialEq::eq,
            );
            Answering { question, answer }
        });
        answering
            .question
            .set_by(question, |a, b| a.0.same(&b.0) && a.1 == b.1 && a.2 == b.2);
        self.runtime
            .read(&answering.answer)
            .ok()
            .and_then(|answer| (*answer).clone())
    }

    /// `run` over `input`, kept for the occurrence at `path`: it
    /// runs again only when the input changes, as `same` compares it, or a
    /// definition it resolved does. `run` is a plain function, since the
    /// occurrence's memo keeps the first call's.
    fn memoized<I: Clone + 'static>(
        &self,
        path: &Trail,
        input: I,
        same: fn(&I, &I) -> bool,
        run: fn(&I, &dyn Host) -> grap::Evaluation,
    ) -> grap::RuntimeValue {
        struct Memoized<I> {
            input: Input<I>,
            result: incremental::Memo<grap::Evaluation>,
        }
        let memoized = self.at(path, || {
            let input = self.runtime.input(input.clone());
            let result = self.runtime.memo_by(
                {
                    let input = input.clone();
                    let definitions = self.definitions.clone();
                    move |read| {
                        let input = input.read(read);
                        Ok(memo::run(&definitions, read, |host| run(&input, host)))
                    }
                },
                same_evaluation,
            );
            Memoized { input, result }
        });
        memoized.input.set_by(input, same);
        self.runtime
            .read(&memoized.result)
            .map(|evaluation| evaluation.result.clone())
            .unwrap_or_else(|error| memo::failure(error).into())
    }
}

fn same_evaluation(a: &grap::Evaluation, b: &grap::Evaluation) -> bool {
    a.halt == b.halt && a.remaining_fuel == b.remaining_fuel && a.result.same_result(&b.result)
}

#[cfg(test)]
#[path = "tests/computations.rs"]
mod tests;
