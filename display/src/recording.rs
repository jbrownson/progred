//! Structural recording of the ordinary builder calls, used only by tests.
use crate::builder::{Builder, Node};
use crate::{FloatingPosition, Layout, Paint, RowAlignment, widget};
use puri::Leaf;
use std::{collections::HashMap, rc::Rc};

pub enum Recorded<World, Hover> {
    Leaf(Leaf<Paint>),
    /// Retain an opaque preparation function without executing it.
    Program(widget::Program<World, Hover>),
    /// Retain an opaque widget without measuring it.
    Widget(widget::Widget<World, Hover>),
    /// Add ordinary placement outputs before a child, without changing its geometry.
    Before {
        child: Box<Recorded<World, Hover>>,
        before: widget::Decoration<World, Hover>,
    },
    /// Add ordinary placement outputs after a child, without changing its geometry.
    After {
        child: Box<Recorded<World, Hover>>,
        after: widget::Decoration<World, Hover>,
    },
    Row {
        alignment: RowAlignment,
        gap: f64,
        children: Vec<Recorded<World, Hover>>,
    },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<Recorded<World, Hover>>,
    },
    /// Children share a left edge and baseline, in back-to-front
    /// order. Its extent is the component-wise maximum.
    Overlay {
        children: Vec<Recorded<World, Hover>>,
    },
    /// Only `base` contributes to surrounding layout. An explicit geometry
    /// function places or omits the floating content; it supplies its own ink
    /// and interaction, just like any other box.
    Floating {
        base: Box<Recorded<World, Hover>>,
        content: Box<Recorded<World, Hover>>,
        position: FloatingPosition,
    },
    Pad {
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        child: Box<Recorded<World, Hover>>,
    },
    /// One projected child used by mutually exclusive layout forms.
    /// The editor measures the shared child once and the selected
    /// form consumes it once. This is local layout sharing, not value
    /// identity and not a cross-frame cache.
    Shared {
        id: usize,
        child: Rc<Recorded<World, Hover>>,
    },
    /// Ordered forms of the same content. Non-final forms use their
    /// natural preferred widths; the first that fits wins. Otherwise
    /// the final accommodating form receives the real allocation.
    /// When nothing fits, the narrowest form wins; earlier forms win
    /// ties.
    Alternatives(Vec<Recorded<World, Hover>>),
}

impl<World, Hover> Clone for Recorded<World, Hover> {
    fn clone(&self) -> Self {
        match self {
            Self::Leaf(display) => Self::Leaf(display.clone()),
            Self::Program(program) => Self::Program(program.clone()),
            Self::Widget(widget) => Self::Widget(widget.clone()),
            Self::Before { child, before } => Self::Before {
                child: child.clone(),
                before: before.clone(),
            },
            Self::After { child, after } => Self::After {
                child: child.clone(),
                after: after.clone(),
            },
            Self::Row {
                alignment,
                gap,
                children,
            } => Self::Row {
                alignment: *alignment,
                gap: *gap,
                children: children.clone(),
            },
            Self::Col {
                baseline,
                gap,
                children,
            } => Self::Col {
                baseline: *baseline,
                gap: *gap,
                children: children.clone(),
            },
            Self::Overlay { children } => Self::Overlay {
                children: children.clone(),
            },
            Self::Floating {
                base,
                content,
                position,
            } => Self::Floating {
                base: base.clone(),
                content: content.clone(),
                position: position.clone(),
            },
            Self::Pad {
                left,
                top,
                right,
                bottom,
                child,
            } => Self::Pad {
                left: *left,
                top: *top,
                right: *right,
                bottom: *bottom,
                child: child.clone(),
            },
            Self::Shared { id, child } => Self::Shared {
                id: *id,
                child: child.clone(),
            },
            Self::Alternatives(options) => Self::Alternatives(options.clone()),
        }
    }
}

pub trait Recordable<W, H> {
    fn record(&self) -> Recorded<W, H>;
}

impl<W, H, R: Recordable<W, H> + ?Sized> Recordable<W, H> for &R {
    fn record(&self) -> Recorded<W, H> {
        (**self).record()
    }
}

impl<W: 'static, H: 'static> Recordable<W, H> for Layout<W, H> {
    fn record(&self) -> Recorded<W, H> {
        let mut recorder = Recording {
            nodes: Vec::new(),
            shared: HashMap::new(),
        };
        let node = self.run(&mut recorder);
        recorder.take(node)
    }
}
impl<W, H> Recordable<W, H> for Recorded<W, H> {
    fn record(&self) -> Recorded<W, H> {
        self.clone()
    }
}

pub fn record<W, H>(layout: &impl Recordable<W, H>) -> Recorded<W, H> {
    layout.record()
}

struct Recording<W, H> {
    nodes: Vec<Option<Recorded<W, H>>>,
    shared: HashMap<usize, Rc<Recorded<W, H>>>,
}
impl<W, H> Recording<W, H> {
    fn push(&mut self, value: Recorded<W, H>) -> Node {
        let node = Node(self.nodes.len());
        self.nodes.push(Some(value));
        node
    }
    fn take(&mut self, node: Node) -> Recorded<W, H> {
        self.nodes[node.0]
            .take()
            .expect("one use of a recorded result")
    }
    fn children(&mut self, children: Vec<Node>) -> Vec<Recorded<W, H>> {
        children.into_iter().map(|node| self.take(node)).collect()
    }
}
impl<W: 'static, H: 'static> Builder<W, H> for Recording<W, H> {
    fn leaf(&mut self, leaf: &Leaf<Paint>) -> Node {
        self.push(Recorded::Leaf(leaf.clone()))
    }
    fn widget(&mut self, widget: widget::Widget<W, H>) -> Node {
        self.push(Recorded::Widget(widget))
    }
    fn program(&mut self, program: widget::Program<W, H>) -> Node {
        self.push(Recorded::Program(program))
    }
    fn before(&mut self, child: Node, before: widget::Decoration<W, H>) -> Node {
        let child = Box::new(self.take(child));
        self.push(Recorded::Before { child, before })
    }
    fn after(&mut self, child: Node, after: widget::Decoration<W, H>) -> Node {
        let child = Box::new(self.take(child));
        self.push(Recorded::After { child, after })
    }
    fn row(&mut self, alignment: RowAlignment, gap: f64, children: Vec<Node>) -> Node {
        let children = self.children(children);
        self.push(Recorded::Row {
            alignment,
            gap,
            children,
        })
    }
    fn col(&mut self, baseline: usize, gap: f64, children: Vec<Node>) -> Node {
        let children = self.children(children);
        self.push(Recorded::Col {
            baseline,
            gap,
            children,
        })
    }
    fn overlay(&mut self, children: Vec<Node>) -> Node {
        let children = self.children(children);
        self.push(Recorded::Overlay { children })
    }
    fn pad(&mut self, insets: peniko::kurbo::Insets, child: Node) -> Node {
        let child = Box::new(self.take(child));
        self.push(Recorded::Pad {
            left: insets.x0,
            top: insets.y0,
            right: insets.x1,
            bottom: insets.y1,
            child,
        })
    }
    fn floating(&mut self, base: Node, content: Node, position: FloatingPosition) -> Node {
        let base = Box::new(self.take(base));
        let content = Box::new(self.take(content));
        self.push(Recorded::Floating {
            base,
            content,
            position,
        })
    }
    fn shared(&mut self, id: usize, child: &Layout<W, H>) -> Node {
        let child = match self.shared.get(&id) {
            Some(child) => child.clone(),
            None => {
                let node = child.run(self);
                let child = Rc::new(self.take(node));
                self.shared.insert(id, child.clone());
                child
            }
        };
        self.push(Recorded::Shared { id, child })
    }
    fn alternatives(&mut self, options: Vec<Node>) -> Node {
        let options = self.children(options);
        self.push(Recorded::Alternatives(options))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_keeps_shared_programs_opaque() {
        let child = crate::shared(Layout::<(), ()>::program(Rc::new(|_, _| {
            panic!("recording must not execute a widget program")
        })));
        let layout = crate::alternatives([child.clone(), child]);
        for _ in 0..2 {
            let Recorded::Alternatives(options) = layout.record() else {
                panic!("expected alternatives")
            };
            match options.as_slice() {
                [
                    Recorded::Shared { id: a, child: left },
                    Recorded::Shared {
                        id: b,
                        child: right,
                    },
                ] => {
                    assert_eq!(a, b);
                    assert!(Rc::ptr_eq(left, right));
                    assert!(matches!(left.as_ref(), Recorded::Program(_)));
                }
                _ => panic!("expected two references to one shared program"),
            }
        }
    }
}
