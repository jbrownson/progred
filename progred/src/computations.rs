//! The editor owns memo roots; libraries compose typed computations in them.

use crate::{libraries::Libraries, sources::Sources, workspace::Root};
use gid::{Document, Step, Value};
use grap::Host;
use incremental::background::{Executor, Tasks};
use incremental::{Input, Roots, Runtime, Source};
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct Snapshot {
    doc: Rc<Document>,
    libraries: Libraries,
}

pub(crate) struct Computations {
    pub runtime: Runtime,
    pub tasks: Tasks,
    snapshot: Input<Snapshot>,
    pub definitions: grap::memo::Definitions<Snapshot>,
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
            doc: Rc::new(Document {
                root: None,
                cells: gid::Cells::new(),
            }),
            libraries: Libraries::default(),
        });
        let definitions = Source::new(snapshot.clone(), |snapshot: &Snapshot, cell| {
            grap::memo::Resolved(Host::resolve(
                &Sources {
                    doc: &snapshot.doc,
                    libraries: &snapshot.libraries,
                },
                *cell,
            ))
        });
        Self {
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

    pub fn evaluate(&self, view: &Root, path: &[Step], expression: &Value, fuel: usize) -> Value {
        struct Evaluation {
            expression: Input<Value>,
            fuel: Input<usize>,
            result: incremental::Memo<grap::Evaluation>,
        }
        let evaluation = self.at(view, path, || {
            let expression = self.runtime.input(expression.clone());
            let fuel = self.runtime.input(fuel);
            let result = grap::memo::evaluate(
                &self.runtime,
                self.definitions.clone(),
                expression.clone(),
                fuel.clone(),
            );
            Evaluation {
                expression,
                fuel,
                result,
            }
        });
        evaluation.expression.set(expression.clone());
        evaluation.fuel.set(fuel);
        self.runtime
            .read(&evaluation.result)
            .map(|evaluation| evaluation.result.clone())
            .unwrap_or_else(grap::memo::failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_roots_reuse_observed_results_and_invalidate_missing_definitions() {
        use grap::{Definition, ForeignFunction};
        use std::cell::Cell;
        let (function, input, unrelated, library) = (
            gid::new_cell_id(),
            gid::new_cell_id(),
            gid::new_cell_id(),
            gid::new_cell_id(),
        );
        let runs = Rc::new(Cell::new(0));
        let mut definitions = crate::libraries::Definitions::default();
        definitions.insert(
            function,
            Definition::foreign(
                Value::record([]),
                ForeignFunction::new({
                    let runs = runs.clone();
                    move |context, call, environment| {
                        runs.set(runs.get() + 1);
                        let argument = context.field(call, input).unwrap();
                        context.eval(argument, environment)
                    }
                })
                .tracked(),
            ),
        );
        let mut libraries = Libraries::default();
        libraries.insert(library, definitions);
        let mut doc = Document {
            root: None,
            cells: gid::Cells::new(),
        };
        let computations = Computations::default();
        let view = Root::document();
        let expression = grap::call(function.into(), [(input, input.into())]);
        let run = |doc: &Document| {
            computations.begin(Rc::new(doc.clone()), libraries.clone());
            computations.evaluate(&view, &[], &expression, 100)
        };
        let missing = run(&doc);
        assert_eq!(
            grap::absent::reason(&missing),
            Some(grap::absent::MISSING_CELL)
        );
        assert_eq!(run(&doc), missing);
        doc.cells.set_value(unrelated, Value::record([]));
        assert_eq!(run(&doc), missing);
        assert_eq!(
            runs.get(),
            1,
            "cache ordinary absents and ignore unrelated changes"
        );
        let value = Value::from(vec![42]);
        doc.cells.set_value(input, value.clone());
        assert_eq!(run(&doc), value);
        assert_eq!(run(&doc), value);
        assert_eq!(runs.get(), 2);
        let other_expression = grap::call(function.into(), [(input, Value::record([]))]);
        assert_eq!(
            computations.evaluate(&view, &[], &other_expression, 100),
            Value::record([])
        );
        assert_eq!(runs.get(), 3);
        assert_eq!(
            grap::absent::reason(&computations.evaluate(&view, &[], &other_expression, 0)),
            Some(grap::absent::FUEL_EXHAUSTED)
        );
        assert_eq!(
            computations.evaluate(&view, &[], &other_expression, 100),
            Value::record([])
        );
        assert_eq!(runs.get(), 4);
    }

    #[test]
    fn locations_in_different_views_have_independent_root_lifetimes() {
        let computations = Computations::default();
        let a = Root::document();
        let b = Root::document();
        let first = computations.at(&a, &[], || computations.runtime.input(1));
        let second = computations.at(&b, &[], || computations.runtime.input(2));
        first.set(3);
        assert_eq!(*second.observed().0, 2);
        assert!(Rc::ptr_eq(
            &first,
            &computations.at(&a, &[], || unreachable!())
        ));
        let weak = Rc::downgrade(&first);
        drop(first);
        drop(computations);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn replacement_cancels_old_jobs_and_preserves_the_executor_and_wake() {
        use incremental::background::{Availability, Job};
        use std::{
            collections::VecDeque,
            sync::{
                Arc, Mutex,
                atomic::{AtomicUsize, Ordering},
            },
        };
        let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
        let wakes = Arc::new(AtomicUsize::new(0));
        let mut computations = Computations::new(
            Executor::new({
                let queue = queue.clone();
                move |job| queue.lock().unwrap().push_back(job)
            }),
            {
                let wakes = wakes.clone();
                move || {
                    wakes.fetch_add(1, Ordering::Relaxed);
                }
            },
        );
        let root = |computations: &Computations| {
            let worker = computations
                .tasks
                .memo(computations.runtime.memo(|_| Ok(7)), |v, _| Ok(v));
            computations.runtime.memo(move |read| {
                Ok(match &*worker.read(read)? {
                    Availability::Pending { .. } => None,
                    Availability::Ready(value) | Availability::Refining(value) => Some(**value),
                })
            })
        };
        let old_runtime = computations.runtime.clone();
        let old = root(&computations);
        assert_eq!(*old_runtime.read(&old).unwrap(), None);
        computations.reset();
        assert_eq!(old_runtime.read(&old), Err(incremental::Error::Cancelled));
        let fresh = root(&computations);
        assert_eq!(*computations.runtime.read(&fresh).unwrap(), None);
        loop {
            let job = queue.lock().unwrap().pop_front();
            let Some(job) = job else { break };
            job();
        }
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
        assert!(computations.tasks.poll());
        assert_eq!(*computations.runtime.read(&fresh).unwrap(), Some(7));
    }
}
