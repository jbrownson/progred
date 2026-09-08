//! Production interpretation of layout calls into one choice graph.
use crate::builder::{Builder, Node};
use crate::{FloatingPosition, Layout, Paint, RowAlignment, widget};
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
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Program);
        let layout = program(self.context, self.build);
        self.push(layout)
    }
    fn before(&mut self, child: Node, before: widget::Decoration<W, H>) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Decoration);
        let before = before(self.context);
        let child = self.build.take(child.0);
        self.push(ChoiceLayout::map(child, 0.0, move |child| {
            widget::before_hover(child, move |placement, output| before(output, placement))
        }))
    }
    fn after(&mut self, child: Node, after: widget::Decoration<W, H>) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Decoration);
        let after = after(self.context);
        let child = self.build.take(child.0);
        self.push(ChoiceLayout::map(child, 0.0, move |child| {
            widget::after_hover(child, move |placement, output| after(output, placement))
        }))
    }
    fn row(&mut self, alignment: RowAlignment, gap: f64, children: Vec<Node>) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Row);
        let children = self.children(children);
        self.push(ChoiceLayout::aligned_row(
            alignment,
            gap * self.context.styles.scale,
            children,
        ))
    }
    fn col(&mut self, baseline: usize, gap: f64, children: Vec<Node>) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Column);
        let children = self.children(children);
        self.push(ChoiceLayout::col(
            baseline,
            gap * self.context.styles.scale,
            children,
        ))
    }
    fn overlay(&mut self, children: Vec<Node>) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Overlay);
        let children = self.children(children);
        self.push(ChoiceLayout::overlay(children))
    }
    fn pad(&mut self, insets: Insets, child: Node) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Padding);
        let child = self.build.take(child.0);
        let scale = self.context.styles.scale;
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
        let scale = self.context.styles.scale;
        self.push(ChoiceLayout::attach(base, content, move |base, content| {
            widget::container::floating(base, content, move |placement, extent| {
                position(scale, placement, extent)
            })
        }))
    }
    fn shared(&mut self, id: usize, child: &Layout<W, H>) -> Node {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Shared);
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
mod tests {
    use super::*;
    use crate::test_support::{NoProject, with_context};
    use crate::widget::{Fragment, HoverContext};
    use measured::choices::resolve_choices;
    use measured::{Extent, place};
    use puri::{DrawCmd, DrawList, Placement, Rect};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn shared_program_prepares_once_per_frame_and_places_only_the_chosen_use() {
        let preparations = Rc::new(Cell::new(0));
        let placements = Rc::new(Cell::new(0));
        let (prepared, placed) = (preparations.clone(), placements.clone());
        let child = crate::shared(Layout::program(Rc::new(move |_, _| {
            prepared.set(prepared.get() + 1);
            let placed = placed.clone();
            ChoiceLayout::fixed(widget::leaf(
                Extent {
                    width: 10.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                move |_: &mut HoverContext<'_, (), ()>, _| placed.set(placed.get() + 1),
            ))
        })));
        let layout = crate::alternatives([crate::pad(100.0, child.clone()), child]);
        for (frame, width, expected_width) in [(1, 120.0, 110.0), (2, 20.0, 10.0)] {
            let mut build = ChoiceBuild::default();
            let prepared = with_context(&NoProject, |context| {
                layout.clone().measure(context, &mut build)
            });
            assert_eq!(preparations.get(), frame);
            assert_eq!(placements.get(), frame - 1);
            let measured = resolve_choices(build.finish(prepared), width, false);
            assert_eq!(measured.extent.width, expected_width);
            place(measured, Placement::root(Rect::new(0.0, 0.0, width, 10.0)))
                .run(&Default::default());
            assert_eq!(placements.get(), frame);
        }
    }

    #[test]
    fn native_leaf_debug_ink_uses_its_full_settled_rectangle() {
        let rect = Rect::new(20.0, 30.0, 33.0, 47.0);
        for debug in [false, true] {
            let layout = Layout::widget(Rc::new(|_| {
                widget::leaf(
                    Extent {
                        width: 13.0,
                        ascent: 0.0,
                        descent: 17.0,
                    },
                    |_: &mut HoverContext<'_, (), ()>, _| {},
                )
            }));
            let mut build = ChoiceBuild::default();
            let prepared = with_context(&NoProject, |context| layout.measure(context, &mut build));
            let measured = resolve_choices(build.finish(prepared), 13.0, false);
            let output = place(measured, Placement::root(rect)).run(&widget::HoverInput {
                debug_geometry: debug,
                ..Default::default()
            });
            let mut canvas = DrawList::new();
            Fragment::<(), ()>::paint(
                output.renders,
                &mut canvas,
                widget::Ink {
                    debug_geometry: debug,
                    ..Default::default()
                },
            );
            if debug {
                assert!(
                    matches!(&canvas.0[..],[DrawCmd::Stroke {shape:puri::Shape::Rect(drawn),..}] if *drawn==rect)
                );
            } else {
                assert!(canvas.0.is_empty());
            }
        }
    }
}
