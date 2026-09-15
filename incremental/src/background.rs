//! Tracked main-thread preparation, owned worker inputs, and explicit readiness.

use super::*;
use std::sync::{Arc, Mutex, mpsc};

#[cfg(test)]
mod tests;

pub type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone)]
pub struct Executor(Arc<dyn Fn(Job) + Send + Sync>);

impl Executor {
    pub fn new(submit: impl Fn(Job) + Send + Sync + 'static) -> Self {
        Self(Arc::new(submit))
    }

    pub fn inline() -> Self {
        Self::new(|job| job())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn threaded(threads: std::num::NonZeroUsize) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..threads.get() {
            let receiver = receiver.clone();
            std::thread::Builder::new()
                .name("computation".into())
                .spawn(move || {
                    loop {
                        let job = receiver.lock().unwrap().recv();
                        let Ok(job) = job else { break };
                        job();
                    }
                })?;
        }
        Ok(Self::new(move |job| {
            sender.send(job).expect("computation workers are alive");
        }))
    }

    fn submit(&self, job: Job) {
        (self.0)(job);
    }
}

struct Work {
    scheduled: bool,
    pending: Option<Job>,
}

struct Slot {
    executor: Executor,
    work: Mutex<Work>,
}

impl Slot {
    fn submit(self: &Arc<Self>, job: Job) {
        let mut work = self.work.lock().unwrap();
        work.pending = Some(job);
        if !work.scheduled {
            work.scheduled = true;
            drop(work);
            self.enqueue();
        }
    }

    fn enqueue(self: &Arc<Self>) {
        let slot = self.clone();
        self.executor.submit(Box::new(move || {
            let job = slot.work.lock().unwrap().pending.take();
            if let Some(job) = job {
                job();
            }
            let mut work = slot.work.lock().unwrap();
            if work.pending.is_some() {
                drop(work);
                slot.enqueue();
            } else {
                work.scheduled = false;
            }
        }));
    }

    fn clear(&self) {
        self.work.lock().unwrap().pending = None;
    }
}

pub enum Availability<T> {
    Pending { previous: Option<Arc<T>> },
    Ready(Arc<T>),
}

trait Completions {
    fn collect(&self) -> bool;
    fn close(&self);
}

pub struct Tasks {
    runtime: Runtime,
    executor: Executor,
    wake: Arc<dyn Fn() + Send + Sync>,
    nodes: RefCell<Vec<Weak<dyn Completions>>>,
}

impl Tasks {
    pub fn new(
        runtime: &Runtime,
        executor: Executor,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            runtime: runtime.clone(),
            executor,
            wake: Arc::new(wake),
            nodes: RefCell::default(),
        }
    }

    pub fn fresh(&self, runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            executor: self.executor.clone(),
            wake: self.wake.clone(),
            nodes: RefCell::default(),
        }
    }

    /// Import completions between graph reads, never while evaluating a recipe.
    pub fn poll(&self) -> bool {
        let mut changed = false;
        self.nodes.borrow_mut().retain(|node| {
            if let Some(node) = node.upgrade() {
                changed |= node.collect();
                true
            } else {
                false
            }
        });
        if changed {
            self.runtime.advance();
        }
        changed
    }

    pub fn memo<I: Clone + Send + 'static, T: Send + Sync + 'static>(
        &self,
        prepare: Memo<I>,
        compute: impl Fn(I, &Cancellation) -> Result<T, Error> + Send + Sync + 'static,
    ) -> AsyncMemo<I, T> {
        let (sender, receiver) = mpsc::channel();
        let node = Rc::new(AsyncNode {
            runtime: self.runtime.clone(),
            prepare,
            compute: Arc::new(compute),
            wake: self.wake.clone(),
            slot: Arc::new(Slot {
                executor: self.executor.clone(),
                work: Mutex::new(Work {
                    scheduled: false,
                    pending: None,
                }),
            }),
            sender,
            receiver: RefCell::new(receiver),
            state: RefCell::new(State {
                input: None,
                generation: 0,
                cancel: Cancellation::default(),
                value: Rc::new(Availability::Pending { previous: None }),
                completed: None,
                failure: None,
                changed: self.runtime.0.get(),
                reusable: true,
                closed: false,
            }),
        });
        let source: Rc<dyn Completions> = node.clone();
        let mut nodes = self.nodes.borrow_mut();
        nodes.retain(|node| node.strong_count() != 0);
        nodes.push(Rc::downgrade(&source));
        AsyncMemo(node)
    }
}

impl Drop for Tasks {
    fn drop(&mut self) {
        for node in self.nodes.get_mut().iter().filter_map(Weak::upgrade) {
            node.close();
        }
        self.runtime.advance();
    }
}

type Completion<T> = (u64, std::thread::Result<Result<T, Error>>);

struct State<I, T> {
    input: Option<Rc<I>>,
    generation: u64,
    cancel: Cancellation,
    value: Rc<Availability<T>>,
    completed: Option<Completion<T>>,
    failure: Option<Error>,
    changed: u64,
    reusable: bool,
    closed: bool,
}

struct AsyncNode<I, T> {
    runtime: Runtime,
    prepare: Memo<I>,
    compute: Arc<dyn Fn(I, &Cancellation) -> Result<T, Error> + Send + Sync>,
    wake: Arc<dyn Fn() + Send + Sync>,
    slot: Arc<Slot>,
    sender: mpsc::Sender<Completion<T>>,
    receiver: RefCell<mpsc::Receiver<Completion<T>>>,
    state: RefCell<State<I, T>>,
}

pub struct AsyncMemo<I, T>(Rc<AsyncNode<I, T>>);

impl<I, T> Clone for AsyncMemo<I, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<I: Clone + Send + 'static, T: Send + Sync + 'static> AsyncMemo<I, T> {
    pub fn read(&self, read: &mut Read) -> Result<Rc<Availability<T>>, Error> {
        if let Err(error) = self.0.refresh(read) {
            read.untracked();
            return Err(error);
        }
        let state = self.0.state.borrow();
        read.observe(self.0.clone(), state.changed);
        read.reusable &= state.reusable;
        Ok(state.value.clone())
    }
}

impl<I: Clone + Send + 'static, T: Send + Sync + 'static> AsyncNode<I, T> {
    fn refresh(&self, read: &Read) -> Result<(), Error> {
        read.check()?;
        if !Rc::ptr_eq(&self.runtime.0, &read.runtime.0)
            || !Rc::ptr_eq(&self.prepare.0.runtime.0, &read.runtime.0)
        {
            return Err(Error::DifferentRuntime);
        }
        if self.state.borrow().closed {
            return Err(Error::Cancelled);
        }
        if let Err(error) = self.prepare.0.refresh(read) {
            let mut state = self.state.borrow_mut();
            state.cancel.cancel();
            state.input = None;
            self.slot.clear();
            return Err(error);
        }
        let (input, reusable) = {
            let prepared = self.prepare.0.state.borrow();
            let prepared = prepared.as_ref().unwrap();
            (prepared.value.clone(), prepared.reusable)
        };
        let mut state = self.state.borrow_mut();
        state.reusable = reusable;
        let new_request = state
            .input
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &input));
        if new_request {
            state.cancel.cancel();
            state.cancel = Cancellation::default();
            state.generation = state
                .generation
                .checked_add(1)
                .expect("request generation exhausted");
            state.input = Some(input.clone());
            state.completed = None;
            state.failure = None;
            let previous = match &*state.value {
                Availability::Pending { previous } => previous.clone(),
                Availability::Ready(value) => Some(value.clone()),
            };
            state.value = Rc::new(Availability::Pending { previous });
            state.changed = read.revision;
            let generation = state.generation;
            let cancel = state.cancel.clone();
            let compute = self.compute.clone();
            let sender = self.sender.clone();
            let wake = self.wake.clone();
            let input = (*input).clone();
            self.slot.submit(Box::new(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    cancel.check()?;
                    let value = compute(input, &cancel)?;
                    cancel.check()?;
                    Ok(value)
                }));
                if sender.send((generation, result)).is_ok() {
                    wake();
                }
            }));
        }
        drop(state);
        // A newly submitted job may complete inline before its first read.
        // Later completions enter only through poll, at a new graph revision.
        if new_request {
            self.collect();
        }
        let mut state = self.state.borrow_mut();
        if let Some((generation, completed)) = state.completed.take() {
            if generation == state.generation {
                state.changed = read.revision;
                let completed = completed.unwrap_or_else(|panic| {
                    state.input = None;
                    std::panic::resume_unwind(panic)
                });
                match completed {
                    Ok(value) => state.value = Rc::new(Availability::Ready(Arc::new(value))),
                    Err(error) => state.failure = Some(error),
                }
            }
        }
        match state.failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl<I, T> Completions for AsyncNode<I, T> {
    fn collect(&self) -> bool {
        let mut state = self.state.borrow_mut();
        let mut changed = false;
        for result in self.receiver.borrow_mut().try_iter() {
            if !state.closed && result.0 == state.generation {
                state.completed = Some(result);
                changed = true;
            }
        }
        changed
    }

    fn close(&self) {
        let mut state = self.state.borrow_mut();
        state.closed = true;
        state.cancel.cancel();
        self.slot.clear();
    }
}

impl<I, T> Drop for AsyncNode<I, T> {
    fn drop(&mut self) {
        self.state.get_mut().cancel.cancel();
        self.slot.clear();
    }
}

impl<I: Clone + Send + 'static, T: Send + Sync + 'static> Dependency for AsyncNode<I, T> {
    fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    fn version(&self, read: &Read) -> Result<u64, Error> {
        self.refresh(read)?;
        Ok(self.state.borrow().changed)
    }
    fn reusable(&self) -> bool {
        self.state.borrow().reusable
    }
}
