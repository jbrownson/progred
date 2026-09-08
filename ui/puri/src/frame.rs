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
mod tests {
    use super::*;
    use crate::{
        DrawList,
        handler::{Handler, KeyboardEvent},
    };

    #[test]
    fn binding_can_build_handlers_without_painting_or_a_layout_system() {
        let mut after = AfterHover::default();
        for id in [1, 2] {
            let mut layer = AfterHover::default();
            layer.push(
                move |hover: Rc<u8>, output: &mut (Vec<Render>, Handler<Vec<u8>>)| {
                    let chosen = *hover == id;
                    output.0.push(Box::new(|_| panic!("painting is optional")));
                    output.1.on_key(move |events, _| {
                        events.push(id);
                        chosen
                    });
                },
            );
            after.append(layer);
        }
        let mut output = (Vec::new(), Handler::new());
        after.bind(Rc::new(1), &mut output);
        let mut events = Vec::new();
        assert!(
            output
                .1
                .dispatch_key(&mut events, &KeyboardEvent::default())
        );
        assert_eq!(events, [2, 1]);
        drop(output.0);
        render(Vec::new(), &mut DrawList::new());
    }
}
