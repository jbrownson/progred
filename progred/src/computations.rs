//! The editor owns memo roots; libraries compose typed computations in them.

use crate::{display::widget::view::Root, library::Libraries, sources::Sources};
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

    pub fn evaluate_runtime(
        &self,
        view: &Root,
        path: &[Step],
        expression: &grap::RuntimeValue,
        fuel: usize,
        origin: Option<grap::SourceOrigin>,
    ) -> grap::RuntimeValue {
        self.memoized(
            view,
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
        view: &Root,
        path: &[Step],
        function: &Value,
        arguments: &[(gid::CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue {
        self.memoized(
            view,
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

    /// `run` over `input`, kept for the occurrence at `path` in `view`: it
    /// runs again only when the input changes, as `same` compares it, or a
    /// definition it resolved does.
    fn memoized<I: Clone + 'static>(
        &self,
        view: &Root,
        path: &[Step],
        input: I,
        same: fn(&I, &I) -> bool,
        run: impl Fn(&I, &dyn Host) -> grap::Evaluation + 'static,
    ) -> grap::RuntimeValue {
        struct Memoized<I> {
            input: Input<I>,
            result: incremental::Memo<grap::Evaluation>,
        }
        let memoized = self.at(view, path, || {
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
    a.completed == b.completed
        && a.remaining_fuel == b.remaining_fuel
        && a.result.same_result(&b.result)
}

#[cfg(test)]
mod tests;
