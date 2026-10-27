//! Optional phase composition for widgets whose output depends on settled hover.
//! The caller supplies traversal, hover identities, and the final output type.

use crate::draw::CanvasSink;
use std::rc::Rc;

pub type Render = Box<dyn FnOnce(&mut dyn CanvasSink)>;

type Continuation<H, O> = Box<dyn FnOnce(Rc<H>, &mut O)>;

pub struct AfterHover<H, O> {
    steps: Vec<Continuation<H, O>>,
}

impl<H, O> Default for AfterHover<H, O> {
    fn default() -> Self {
        Self { steps: Vec::new() }
    }
}

impl<H, O> AfterHover<H, O> {
    pub fn len(&self) -> usize {
        self.steps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
    pub fn split_off(&mut self, at: usize) -> Self {
        Self {
            steps: self.steps.split_off(at),
        }
    }
    pub fn push(&mut self, next: impl FnOnce(Rc<H>, &mut O) + 'static) {
        self.steps.push(Box::new(next));
    }

    pub fn append(&mut self, mut above: Self) {
        if self.steps.is_empty() {
            *self = above;
        } else {
            self.steps.append(&mut above.steps);
        }
    }

    pub fn bind(self, hover: Rc<H>, output: &mut O) {
        for next in self.steps {
            next(hover.clone(), output);
        }
    }
}

pub fn render(renders: Vec<Render>, canvas: &mut dyn CanvasSink) {
    for render in renders {
        render(canvas);
    }
}

#[cfg(test)]
#[path = "tests/frame.rs"]
mod tests;
