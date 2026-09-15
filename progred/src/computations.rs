//! The editor owns memo roots; libraries compose typed computations in them.

use crate::{libraries::Libraries, sources::Sources, workspace::Root};
use gid::{Document, Step};
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    Availability::Ready(value) => Some(**value),
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
