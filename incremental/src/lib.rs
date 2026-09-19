//! Caller-owned inputs and demand-driven computations. No global graph or
//! reconciliation: cloning a handle shares the computation it already names.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub mod background;

#[cfg(test)]
mod tests;

#[derive(Clone, Default)]
pub struct Runtime(Rc<Cell<u64>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Cycle,
    InputsChanged,
    DifferentRuntime,
    Cancelled,
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Share cancellation with another library. The flag is one-way: once
    /// cancelled, never reset it; create a new token for a new request.
    /// This flag does not synchronize publication of any other data.
    pub fn shared_flag(&self) -> &Arc<AtomicBool> {
        &self.0
    }

    pub fn check(&self) -> Result<(), Error> {
        if self.0.load(Ordering::Relaxed) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl Runtime {
    fn advance(&self) -> u64 {
        let revision = self.0.get().checked_add(1).expect("revision exhausted");
        self.0.set(revision);
        revision
    }

    pub fn input<T: 'static>(&self, value: T) -> Input<T> {
        Input(Rc::new(InputNode {
            runtime: self.clone(),
            value: RefCell::new(Rc::new(value)),
            changed: Cell::new(self.0.get()),
        }))
    }

    pub fn memo<T: PartialEq + 'static>(
        &self,
        compute: impl Fn(&mut Read) -> Result<T, Error> + 'static,
    ) -> Memo<T> {
        self.memo_by(compute, PartialEq::eq)
    }

    pub fn memo_by<T: 'static>(
        &self,
        compute: impl Fn(&mut Read) -> Result<T, Error> + 'static,
        equal: impl Fn(&T, &T) -> bool + 'static,
    ) -> Memo<T> {
        Memo(Rc::new(MemoNode {
            runtime: self.clone(),
            compute: Box::new(compute),
            equal: Box::new(equal),
            state: RefCell::new(None),
            active: Cell::new(false),
        }))
    }

    pub fn read<T: 'static>(&self, memo: &Memo<T>) -> Result<Rc<T>, Error> {
        self.read_with(memo, &Cancellation::default())
    }

    pub fn read_with<T: 'static>(
        &self,
        memo: &Memo<T>,
        cancellation: &Cancellation,
    ) -> Result<Rc<T>, Error> {
        memo.read(&mut Read::new(self, cancellation))
    }
}

struct InputNode<T> {
    runtime: Runtime,
    value: RefCell<Rc<T>>,
    changed: Cell<u64>,
}

pub struct Input<T>(Rc<InputNode<T>>);

impl<T> Clone for Input<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static> Input<T> {
    pub fn read(&self, read: &mut Read) -> Rc<T> {
        let (value, observation) = self.observed();
        read.record(observation);
        value
    }

    pub fn observed(&self) -> (Rc<T>, Observation) {
        (
            self.0.value.borrow().clone(),
            Observation(Observed {
                node: self.0.clone(),
                version: self.0.changed.get(),
            }),
        )
    }

    pub fn set(&self, value: T)
    where
        T: PartialEq,
    {
        self.set_by(value, PartialEq::eq);
    }

    pub fn set_by(&self, value: T, equal: impl FnOnce(&T, &T) -> bool) {
        if !equal(&self.0.value.borrow(), &value) {
            *self.0.value.borrow_mut() = Rc::new(value);
            self.0.changed.set(self.0.runtime.advance());
        }
    }
}

trait Dependency {
    fn runtime(&self) -> &Runtime;
    fn version(&self, read: &Read) -> Result<u64, Error>;
    fn reusable(&self) -> bool {
        true
    }
}

impl<T> Dependency for InputNode<T> {
    fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    fn version(&self, _: &Read) -> Result<u64, Error> {
        Ok(self.changed.get())
    }
}

#[derive(Clone)]
struct Observed {
    node: Rc<dyn Dependency>,
    version: u64,
}

pub struct Observation(Observed);

pub struct Read {
    runtime: Runtime,
    revision: u64,
    cancellation: Cancellation,
    dependencies: Vec<Observed>,
    reusable: bool,
    same_runtime: bool,
}

impl Read {
    pub fn record(&mut self, observation: Observation) {
        self.observe(observation.0.node, observation.0.version);
    }
    fn new(runtime: &Runtime, cancellation: &Cancellation) -> Self {
        Self {
            runtime: runtime.clone(),
            revision: runtime.0.get(),
            cancellation: cancellation.clone(),
            dependencies: Vec::new(),
            reusable: true,
            same_runtime: true,
        }
    }

    fn observe(&mut self, node: Rc<dyn Dependency>, version: u64) {
        self.same_runtime &= Rc::ptr_eq(&self.runtime.0, &node.runtime().0);
        if !self.dependencies.iter().any(|d| Rc::ptr_eq(&d.node, &node)) {
            self.dependencies.push(Observed { node, version });
        }
    }

    pub fn untracked(&mut self) {
        self.reusable = false;
    }

    pub fn check(&self) -> Result<(), Error> {
        self.cancellation.check()?;
        if !self.same_runtime {
            Err(Error::DifferentRuntime)
        } else if self.revision != self.runtime.0.get() {
            Err(Error::InputsChanged)
        } else {
            Ok(())
        }
    }
}

struct Cached<T> {
    value: Rc<T>,
    dependencies: Vec<Observed>,
    verified: u64,
    changed: u64,
    reusable: bool,
}

struct MemoNode<T> {
    runtime: Runtime,
    compute: Box<dyn Fn(&mut Read) -> Result<T, Error>>,
    equal: Box<dyn Fn(&T, &T) -> bool>,
    state: RefCell<Option<Cached<T>>>,
    active: Cell<bool>,
}

pub struct Memo<T>(Rc<MemoNode<T>>);

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static> Memo<T> {
    pub fn read(&self, read: &mut Read) -> Result<Rc<T>, Error> {
        let refreshed = read.check().and_then(|()| {
            if Rc::ptr_eq(&self.0.runtime.0, &read.runtime.0) {
                self.0.refresh(read)
            } else {
                Err(Error::DifferentRuntime)
            }
        });
        if let Err(error) = refreshed {
            read.untracked();
            return Err(error);
        }
        let state = self.0.state.borrow();
        let state = state.as_ref().expect("a completed memo has a value");
        read.observe(self.0.clone(), state.changed);
        read.reusable &= state.reusable;
        Ok(state.value.clone())
    }
}

struct Active<'a>(&'a Cell<bool>);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl<T: 'static> MemoNode<T> {
    fn refresh(&self, parent: &Read) -> Result<(), Error> {
        parent.check()?;
        if self.active.replace(true) {
            return Err(Error::Cycle);
        }
        let _active = Active(&self.active);
        let dependencies = {
            let state = self.state.borrow();
            match state.as_ref() {
                Some(state) if state.reusable && state.verified == parent.revision => return Ok(()),
                Some(state) if state.reusable => Some(state.dependencies.clone()),
                _ => None,
            }
        };
        if let Some(dependencies) = dependencies {
            let mut unchanged = true;
            for dependency in dependencies {
                if dependency.node.version(parent) != Ok(dependency.version)
                    || !dependency.node.reusable()
                {
                    unchanged = false;
                    break;
                }
            }
            if unchanged {
                self.state.borrow_mut().as_mut().unwrap().verified = parent.revision;
                return Ok(());
            }
        }
        parent.check()?;
        let mut read = Read::new(&self.runtime, &parent.cancellation);
        let value = (self.compute)(&mut read)?;
        read.check()?;
        let mut state = self.state.borrow_mut();
        let (value, changed) = match state.as_ref() {
            Some(previous) if (self.equal)(&previous.value, &value) => {
                (previous.value.clone(), previous.changed)
            }
            _ => (Rc::new(value), parent.revision),
        };
        *state = Some(Cached {
            value,
            changed,
            verified: parent.revision,
            dependencies: read.dependencies,
            reusable: read.reusable,
        });
        Ok(())
    }
}

impl<T: 'static> Dependency for MemoNode<T> {
    fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    fn version(&self, read: &Read) -> Result<u64, Error> {
        self.refresh(read)?;
        Ok(self.state.borrow().as_ref().unwrap().changed)
    }
    fn reusable(&self) -> bool {
        self.state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.reusable)
    }
}

/// A keyed read from an immutable snapshot. Each observed key retains its
/// own value/version, so unrelated snapshot edits do not invalidate readers.
pub struct Source<S, K, V> {
    snapshot: Input<S>,
    select: Rc<dyn Fn(&S, &K) -> V>,
    entries: Rc<RefCell<Vec<(K, Weak<Selected<S, K, V>>)>>>,
}

impl<S, K, V> Clone for Source<S, K, V> {
    fn clone(&self) -> Self {
        Self {
            snapshot: self.snapshot.clone(),
            select: self.select.clone(),
            entries: self.entries.clone(),
        }
    }
}

struct Selected<S, K, V> {
    snapshot: Input<S>,
    key: K,
    select: Rc<dyn Fn(&S, &K) -> V>,
    value: RefCell<V>,
    verified: Cell<u64>,
    changed: Cell<u64>,
}

impl<S: 'static, K: Clone + Eq + 'static, V: Clone + PartialEq + 'static> Source<S, K, V> {
    pub fn new(snapshot: Input<S>, select: impl Fn(&S, &K) -> V + 'static) -> Self {
        Self {
            snapshot,
            select: Rc::new(select),
            entries: Rc::default(),
        }
    }

    pub fn read(&self, key: K, read: &mut Read) -> V {
        let mut entries = self.entries.borrow_mut();
        entries.retain(|(_, entry)| entry.strong_count() != 0);
        let node = entries
            .iter()
            .find_map(|(k, entry)| (k == &key).then(|| entry.upgrade()).flatten())
            .unwrap_or_else(|| {
                let revision = self.snapshot.0.changed.get();
                let node = Rc::new(Selected {
                    snapshot: self.snapshot.clone(),
                    value: RefCell::new((self.select)(&self.snapshot.0.value.borrow(), &key)),
                    key: key.clone(),
                    select: self.select.clone(),
                    verified: Cell::new(revision),
                    changed: Cell::new(revision),
                });
                entries.push((key, Rc::downgrade(&node)));
                node
            });
        node.refresh();
        read.observe(node.clone(), node.changed.get());
        node.value.borrow().clone()
    }
}

impl<S, K, V: PartialEq> Selected<S, K, V> {
    fn refresh(&self) {
        let revision = self.snapshot.0.changed.get();
        if self.verified.get() != revision {
            let value = (self.select)(&self.snapshot.0.value.borrow(), &self.key);
            if *self.value.borrow() != value {
                *self.value.borrow_mut() = value;
                self.changed.set(revision);
            }
            self.verified.set(revision);
        }
    }
}

impl<S, K, V: PartialEq> Dependency for Selected<S, K, V> {
    fn runtime(&self) -> &Runtime {
        &self.snapshot.0.runtime
    }
    fn version(&self, _: &Read) -> Result<u64, Error> {
        self.refresh();
        Ok(self.changed.get())
    }
}

/// Caller-keyed roots. A pass keeps the roots it demands; unused roots are
/// released at the next pass. Dependency ownership retains reachable children.
pub struct Roots<K> {
    entries: RefCell<Vec<(K, TypeId, Rc<dyn Any>, bool)>>,
}

impl<K> Default for Roots<K> {
    fn default() -> Self {
        Self {
            entries: RefCell::new(Vec::new()),
        }
    }
}

impl<K: Eq> Roots<K> {
    pub fn begin(&self) {
        self.entries.borrow_mut().retain_mut(|(_, _, _, used)| {
            let keep = *used;
            *used = false;
            keep
        });
    }

    pub fn get<T: 'static>(&self, key: K, create: impl FnOnce() -> T) -> Rc<T> {
        let mut entries = self.entries.borrow_mut();
        if let Some((_, _, value, used)) = entries
            .iter_mut()
            .find(|(k, ty, _, _)| k == &key && *ty == TypeId::of::<T>())
        {
            *used = true;
            value
                .clone()
                .downcast()
                .ok()
                .expect("the stored type matches its key")
        } else {
            drop(entries);
            let value = Rc::new(create());
            self.entries
                .borrow_mut()
                .push((key, TypeId::of::<T>(), value.clone(), true));
            value
        }
    }
}
