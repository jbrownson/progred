//! Caller-owned values staged across speculative function calls.

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

/// Cloning T must preserve an independent snapshot of its mutable contents.
/// Checkpoints share T; only a subsequent write needs to clone it.
pub struct Effects<T> {
    value: RefCell<Rc<T>>,
    checkpoints: RefCell<Vec<Rc<T>>>,
}

impl<T: Clone> Effects<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: RefCell::new(Rc::new(value)),
            checkpoints: RefCell::new(Vec::new()),
        }
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        Ref::map(self.value.borrow(), Rc::as_ref)
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        RefMut::map(self.value.borrow_mut(), Rc::make_mut)
    }

    pub fn into_inner(self) -> T {
        Rc::unwrap_or_clone(self.value.into_inner())
    }
}

pub(crate) trait Scope {
    fn begin(&self);
    fn finish(&self, accepted: bool);
}

impl<T: Clone> Scope for Effects<T> {
    fn begin(&self) {
        self.checkpoints
            .borrow_mut()
            .push(self.value.borrow().clone());
    }

    fn finish(&self, accepted: bool) {
        let before = self.checkpoints.borrow_mut().pop().unwrap();
        if !accepted {
            *self.value.borrow_mut() = before;
        }
    }
}
