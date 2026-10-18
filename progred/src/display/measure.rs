//! Production interpretation of layout calls into one choice graph.
use crate::display::builder::{Builder, Node};
use crate::display::{FloatingPosition, Layout, Paint, RowAlignment, widget};
use measured::choices::{ChoiceBuild, ChoiceLayout};
use peniko::kurbo::Insets;
use puri::Leaf;
use widget::{Context, HoverPass};

struct Prepare<'a, 'cx, 'fonts, W, H> {
    context: &'a mut Context<'cx, 'fonts, W, H>,
    build: &'a mut ChoiceBuild<HoverPass<W, H>>,
}

impl<W: 'static, H: 'static> Layout<W, H> {
    pub fn measure(
        &self,
        context: &mut Context<'_, '_, W, H>,
        build: &mut ChoiceBuild<HoverPass<W, H>>,
    ) -> ChoiceLayout<HoverPass<W, H>> {
        let node = self.run(&mut Prepare { context, build });
        build.take(node.0)
    }
}

impl<W: 'static, H: 'static> Prepare<'_, '_, '_, W, H> {
    fn push(&mut self, layout: ChoiceLayout<HoverPass<W, H>>) -> Node {
        Node(self.build.push(layout))
    }

    fn children(&mut self, nodes: Vec<Node>) -> Vec<ChoiceLayout<HoverPass<W, H>>> {
        nodes
            .into_iter()
            .map(|node| self.build.take(node.0))
            .collect()
    }
}

impl<W: 'static, H: 'static> Builder<W, H> for Prepare<'_, '_, '_, W, H> {
    fn leaf(&mut self, leaf: &Leaf<Paint>) -> Node {
        let measured = widget::drawing::prepare(self.context, leaf);
        self.push(ChoiceLayout::fixed(measured))
    }
    fn widget(&mut self, widget: widget::Widget<W, H>) -> Node {
        let measured = widget(self.context);
        self.push(ChoiceLayout::fixed(measured))
    }
    fn program(&mut self, program: widget::Program<W, H>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Program);
        let layout = program(self.context, self.build);
        self.push(layout)
    }
    fn before(&mut self, child: Node, before: widget::Decoration<W, H>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Decoration);
        let before = before(self.context);
        let child = self.build.take(child.0);
        self.push(ChoiceLayout::map(child, move |child| {
            widget::before_place(child, move |placement, output| before(output, placement))
        }))
    }
    fn after(&mut self, child: Node, after: widget::Decoration<W, H>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Decoration);
        let after = after(self.context);
        let child = self.build.take(child.0);
        self.push(ChoiceLayout::map(child, move |child| {
            widget::after_place(child, move |placement, output| after(output, placement))
        }))
    }
    fn around(&mut self, child: Node, wrap: widget::Around<W, H>) -> Node {
        let wrap = wrap(self.context);
        let child = self.build.take(child.0);
        self.push(ChoiceLayout::map(child, wrap))
    }
    fn row(&mut self, alignment: RowAlignment, gap: f64, children: Vec<Node>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Row);
        let children = self.children(children);
        let row = ChoiceLayout::aligned_row(
            alignment,
            gap * self.context.toolkit.styles().scale,
            children,
        );
        self.push(row)
    }
    fn col(&mut self, baseline: usize, gap: f64, children: Vec<Node>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Column);
        let children = self.children(children);
        let col = ChoiceLayout::col(
            baseline,
            gap * self.context.toolkit.styles().scale,
            children,
        );
        self.push(col)
    }
    fn overlay(&mut self, children: Vec<Node>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Overlay);
        let children = self.children(children);
        self.push(ChoiceLayout::overlay(children))
    }
    fn pad(&mut self, insets: Insets, child: Node) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Padding);
        let child = self.build.take(child.0);
        let scale = self.context.toolkit.styles().scale;
        self.push(ChoiceLayout::pad(
            Insets::new(
                insets.x0 * scale,
                insets.y0 * scale,
                insets.x1 * scale,
                insets.y1 * scale,
            ),
            child,
        ))
    }
    fn floating(&mut self, base: Node, content: Node, position: FloatingPosition) -> Node {
        let base = self.build.take(base.0);
        let content = self.build.take(content.0);
        let scale = self.context.toolkit.styles().scale;
        self.push(ChoiceLayout::attach(base, content, move |base, content| {
            widget::container::floating(base, content, move |placement, extent| {
                position(scale, placement, extent)
            })
        }))
    }
    fn shared(&mut self, id: usize, child: &Layout<W, H>) -> Node {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Shared);
        let context = &mut *self.context;
        let layout = self.build.shared(id, |build| child.measure(context, build));
        self.push(layout)
    }
    fn alternatives(&mut self, options: Vec<Node>) -> Node {
        let options = self.children(options);
        let layout = self.build.alternatives(options);
        self.push(layout)
    }
}

#[cfg(test)]
mod tests;
