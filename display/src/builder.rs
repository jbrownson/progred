//! The layout operation interface. Interpreters may prepare boxes or record calls.
use crate::{FloatingPosition, Layout, Paint, RowAlignment, widget};
use peniko::kurbo::Insets;
use puri::Leaf;

/// An interpreter-local result, consumed once by a parent operation.
#[derive(Clone, Copy)]
pub struct Node(pub usize);

pub trait Builder<W, H> {
    fn leaf(&mut self, leaf: &Leaf<Paint>) -> Node;
    fn widget(&mut self, widget: widget::Widget<W, H>) -> Node;
    fn program(&mut self, program: widget::Program<W, H>) -> Node;
    fn before(&mut self, child: Node, before: widget::Decoration<W, H>) -> Node;
    fn after(&mut self, child: Node, after: widget::Decoration<W, H>) -> Node;
    fn row(&mut self, alignment: RowAlignment, gap: f64, children: Vec<Node>) -> Node;
    fn col(&mut self, baseline: usize, gap: f64, children: Vec<Node>) -> Node;
    fn overlay(&mut self, children: Vec<Node>) -> Node;
    fn pad(&mut self, insets: Insets, child: Node) -> Node;
    fn floating(&mut self, base: Node, content: Node, position: FloatingPosition) -> Node;
    fn shared(&mut self, id: usize, child: &Layout<W, H>) -> Node;
    fn alternatives(&mut self, options: Vec<Node>) -> Node;
}
