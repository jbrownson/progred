//! Progred adapters for the placement → hover → paint/dispatch chain.
//! The shared after-hover boundary keeps paint optional and resolves targets
//! before either painting or dispatch sees the frame.

use crate::completion::Offers;
use crate::display::widget::container::{self, Layers};
use crate::frame::Hovered;
use crate::navigate::Descend;
use crate::workspace::Root;
use kurbo::{Affine, Point, Rect, Stroke, Vec2};
use measured::{Extent, Measured};
use peniko::{Brush, ImageData};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::handler::{Handler, HasHandler, ScrollOutcome};
#[cfg(test)]
use puri::hover::Claim;
use puri::text::TextMetrics;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent};
use uig::Placement;

pub type HoverPass<C> = crate::display::widget::HoverPass<C, Hovered>;
pub type HoverOutput<C> = crate::display::widget::HoverOutput<C, Hovered>;
pub use crate::display::widget::frame::place;
pub use crate::display::widget::{HoverContext, HoverInput};
pub type DispatchContext<C> = crate::display::widget::frame::DispatchContext<C, Hovered>;
pub type ResolvedHover = crate::display::widget::frame::ResolvedHover<Hovered>;
pub use puri::frame::Render;
pub type Probe = crate::display::widget::frame::Probe<Hovered>;
pub use crate::display::widget::frame::ViewRegion;

/// App-facing construction inside the hover continuation. Claims are
/// answered now; ink and handlers are returned for later execution.
pub(crate) struct Builder<'a, 'input, C: 'static> {
    placed: &'a mut HoverContext<'input, C, Hovered>,
    visible: bool,
}

impl<'builder, 'input, C: 'static> Builder<'builder, 'input, C> {
    fn new(placed: &'builder mut HoverContext<'input, C, Hovered>, placement: Placement) -> Self {
        Self {
            placed,
            visible: !placement.clipped_out(),
        }
    }

    /// Contribute a named hover region.
    pub fn claim(&mut self, placement: Placement, target: Hovered) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::retaining(placement, target));
        }
    }

    /// Contribute a named hover region without the ordinary air-gap
    /// retention. Useful for chrome whose pointer feedback should track
    /// its hit geometry exactly.
    pub fn claim_exact(&mut self, placement: Placement, target: Hovered) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::exact(placement, target));
        }
    }

    pub fn claim_dynamic(
        &mut self,
        placement: Placement,
        target_at: impl Fn(Point) -> Option<Hovered> + 'static,
    ) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::dynamic(placement, target_at));
        }
    }

    /// Contribute an unnamed region that blocks targets below it.
    pub fn occlude(&mut self, placement: Placement) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::occludes(placement));
            self.handler().on_pointer_down(move |_, event| {
                placement.contains(Point::new(event.state.position.x, event.state.position.y))
            });
        }
    }

    pub fn activate(&mut self, target: Hovered, action: impl Fn(&mut C) -> bool + 'static) {
        self.activate_with(target, move |ctx, _| action(ctx));
    }

    pub fn pick(&mut self, target: Hovered, action: impl Fn(&mut C) -> bool + 'static) {
        self.pick_with(target, move |ctx, _| action(ctx));
    }

    pub fn pick_with(
        &mut self,
        target: Hovered,
        action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) {
        self.target_action(target, true, action);
    }

    pub fn activate_with(
        &mut self,
        target: Hovered,
        action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) {
        self.target_action(target, false, action);
    }

    fn target_action(
        &mut self,
        target: Hovered,
        pick: bool,
        action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) {
        if self.visible {
            self.handler()
                .on_pointer_down_with(move |ctx, event, pointer| {
                    puri::interact::is_primary_contact(event)
                        && crate::modifiers::pick(&event.state.modifiers) == pick
                        && pointer.matches(&target)
                        && action(ctx, event)
                });
        }
    }

    pub fn pick_dynamic(
        &mut self,
        placement: Placement,
        action: impl Fn(&mut C, &Hovered, &[Descend<C>]) -> bool + 'static,
    ) {
        if self.visible {
            self.handler()
                .on_pointer_down_with(move |ctx, event, pointer| {
                    puri::interact::is_primary_contact(event)
                        && crate::modifiers::pick(&event.state.modifiers)
                        && !pointer.outside_view
                        && placement
                            .contains(Point::new(event.state.position.x, event.state.position.y))
                        && pointer
                            .hovered
                            .as_ref()
                            .is_some_and(|target| action(ctx, target, &pointer.descends))
                });
        }
    }

    /// Defer painting until this frame's hover has settled.
    pub fn render(
        &mut self,
        render: impl FnOnce(&mut dyn puri::draw::CanvasSink, &ResolvedHover) + 'static,
    ) {
        if self.visible {
            self.placed.render(render);
        }
    }
}

impl<C: 'static> puri::draw::CanvasSink for Builder<'_, '_, C> {
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        if self.visible {
            self.placed.render(move |cv, _| cv.image(image, transform));
        }
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        if self.visible {
            self.placed
                .render(move |cv, _| cv.fill(shape, brush, transform));
        }
    }

    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        if self.visible {
            self.placed
                .render(move |cv, _| cv.stroke(shape, style, brush, transform));
        }
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
        if self.visible {
            self.placed.render(move |cv, _| cv.glyph_run(run));
        }
    }

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
    ) {
        if self.visible {
            self.placed.with_clip(shape, transform, |output| {
                content(&mut Builder {
                    placed: output,
                    visible: true,
                });
            });
        }
    }
}

impl<C: 'static> HasHandler<C> for Builder<'_, '_, C> {
    type Input = DispatchContext<C>;

    fn handler(&mut self) -> &mut Handler<C, DispatchContext<C>> {
        self.placed.handler()
    }
}

impl<C: 'static> Builder<'_, '_, C> {
    pub fn completion(&mut self) -> &mut Option<Offers<C>> {
        self.placed.completion()
    }
}

fn built_into<C: 'static>(
    f: impl FnOnce(&mut Builder<'_, '_, C>, Placement) + 'static,
) -> impl FnOnce(Placement, &mut HoverContext<'_, C, Hovered>) + 'static {
    move |placement, placed| f(&mut Builder::new(placed, placement), placement)
}

pub fn leaf<C: 'static>(
    extent: Extent,
    place: impl FnOnce(&mut Builder<'_, '_, C>, Placement) + 'static,
) -> Measured<HoverPass<C>> {
    crate::display::widget::leaf(extent, move |output, placement| {
        built_into(place)(placement, output)
    })
}

pub fn before<C: 'static>(
    child: Measured<HoverPass<C>>,
    place_before: impl FnOnce(&mut Builder<'_, '_, C>, Placement) + 'static,
) -> Measured<HoverPass<C>> {
    crate::display::widget::before_place(child, built_into(place_before))
}

/// Add `content` as an out-of-flow subtree without contributing its
/// extent to `base`. The completed frame raises all such subtrees
/// together.
pub use crate::display::widget::container::floating;

pub fn decorate<C: 'static>(
    child: Measured<HoverPass<C>>,
    draw: impl FnOnce(&mut Builder<'_, '_, C>, Rect) + 'static,
) -> Measured<HoverPass<C>> {
    before(child, move |p, placement| draw(p, placement.rect))
}

pub fn on_key<C: 'static>(
    child: Measured<HoverPass<C>>,
    action: impl Fn(&mut C, &KeyboardEvent) -> bool + 'static,
) -> Measured<HoverPass<C>> {
    before(child, move |p, _| {
        p.handler().on_key(action);
    })
}

/// A scroll viewport over `child`: placed at the viewport rect, it
/// shifts the child up-left by `offset` inside a clip. The caller
/// owns and clamps the offset. Pointer-down and scroll gate on the
/// viewport (starts stay inside it); motion, release, and keys pass
/// unbounded so active gestures and the focused editor keep working
/// outside.
/// A scroll viewport tagged as an editor view. The settled region is frame
/// output, not retained widget state; input and keyboard reveal use
/// it to update the same caller-owned view.
pub fn scrolled_at<C: 'static>(
    child: Measured<HoverPass<C>>,
    offset: Vec2,
    owner: Option<(Root, f64)>,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<HoverPass<C>> {
    let extent = child.extent;
    let scrolled = container::scrolled(child, offset, on_scroll);
    before(scrolled, move |output, placement| {
        if let Some((root, scale)) = owner {
            output.placed.view_region(ViewRegion {
                root,
                rect: placement.rect,
                maximum: Vec2::new(
                    ((extent.width - placement.rect.width()) / scale).max(0.0),
                    ((extent.height() - placement.rect.height()) / scale).max(0.0),
                ),
            });
        }
    })
}

/// An assigned-size editor view, with no content scrolling or padding.
pub fn viewport<C: 'static>(child: Measured<HoverPass<C>>, root: Root) -> Measured<HoverPass<C>> {
    measured::around_into(child, move |placement, inner, pass| {
        let child_rect = inner.extent().rect_at(placement.rect.origin());
        let child_placement = measured::child_placement(
            measured::clipped_placement(placement, placement.rect),
            child_rect,
        );
        pass.clipped(placement, |pass| inner.place_at_into(child_placement, pass));
        pass.visit(|placed| {
            placed.view_region(ViewRegion {
                root,
                rect: placement.rect,
                maximum: Vec2::ZERO,
            });
        });
    })
}

/// Associate every navigation occurrence produced by `child` with
/// one editor view. This happens after projection and placement, so
/// the projection language remains unaware of panes.
pub fn in_view<C: 'static>(child: Measured<HoverPass<C>>, root: Root) -> Measured<HoverPass<C>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.in_view(root, |pass| inner.place_at_into(placement, pass));
    })
}

pub fn metrics_extent(metrics: TextMetrics) -> Extent {
    Extent {
        width: metrics.width,
        ascent: metrics.ascent,
        descent: metrics.descent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::Color;
    use puri::draw::{DrawCmd, DrawList};
    use ui_events::ScrollDelta;
    use ui_events::pointer::{
        PointerButton, PointerId, PointerInfo, PointerState, PointerType, PointerUpdate,
    };

    struct TestCanvas(DrawList);

    impl puri::draw::CanvasSink for TestCanvas {
        fn draw_image(&mut self, image: ImageData, transform: Affine) {
            self.0.image(image, transform);
        }

        fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
            self.0.fill(shape, brush, transform);
        }

        fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
            self.0.stroke(shape, style, brush, transform);
        }

        fn draw_glyphs(&mut self, run: GlyphRun) {
            self.0.glyph_run(run);
        }

        fn with_clip(
            &mut self,
            shape: Shape,
            transform: Affine,
            content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
        ) {
            let mut inner = TestCanvas(DrawList::new());
            content(&mut inner);
            self.0.0.push(DrawCmd::Clip {
                shape,
                transform,
                children: inner.0.0,
            });
        }
    }

    fn no_ink() -> ResolvedHover {
        ResolvedHover {
            hovered: None,
            hovered_secondary: None,
            hovered_trace: None,
        }
    }

    fn pointer() -> PointerInfo {
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn state_at(x: f64, y: f64) -> PointerState {
        let mut state = PointerState::default();
        state.position.x = x;
        state.position.y = y;
        state
    }

    fn down_at(x: f64, y: f64) -> PointerButtonEvent {
        PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: pointer(),
            state: state_at(x, y),
        }
    }

    fn native_delimiter(
        interactive: bool,
        scale: f64,
        span: Extent,
        side: puri::delim::Side,
    ) -> Measured<HoverPass<crate::Editor>> {
        use crate::display::widget;
        let value = gid::Value::record([]);
        let side = widget::delimiter::side(puri::Delim::Bracket, side);
        let side = if interactive {
            widget::selectable_widget(side)
        } else {
            side
        };
        let native = crate::display::test_support::with_context(
            &crate::display::test_support::NoProject,
            |context| {
                let styles = crate::styles::editor(scale);
                let mut inputs = context.inputs.clone();
                inputs.styles = &styles;
                widget::fill_height(side)(&mut widget::Context {
                    value: Some(&value),
                    inputs: &inputs,
                    project: context.project,
                    path: context.path,
                    text: &mut *context.text,
                })
            },
        );
        let extent = Extent {
            width: native.extent.width,
            ascent: span.ascent.max(native.extent.ascent),
            descent: span.descent.max(native.extent.descent),
        };
        measured::leaf_into(extent, move |placement, pass| {
            measured::place_into(native, placement, pass)
        })
    }

    #[test]
    fn delimiter_interaction_is_opt_in_without_changing_ink_or_extent() {
        use kurbo::Shape as _;
        for scale in [1.0, 2.0] {
            for span in [
                Extent::default(),
                Extent {
                    width: 40.0,
                    ascent: 80.0,
                    descent: 50.0,
                },
            ] {
                for side in [puri::delim::Side::Open, puri::delim::Side::Close] {
                    let inert = native_delimiter(false, scale, span, side);
                    let interactive = native_delimiter(true, scale, span, side);
                    assert_eq!(inert.extent, interactive.extent);
                    let placement = Placement::root(inert.extent.rect_at(Point::new(20.0, 30.0)));
                    let inert =
                        crate::display::widget::frame::place(inert, placement, &Default::default());
                    let interactive = crate::display::widget::frame::place(
                        interactive,
                        placement,
                        &HoverInput {
                            pointer: Some(placement.rect.center()),
                            ..Default::default()
                        },
                    );
                    assert!(inert.claim.is_none() && inert.handler.is_none());
                    assert!(interactive.claim.is_some());
                    assert_eq!(
                        interactive.claim.clone().map(|(_, claim)| claim),
                        Some(Claim::Direct(Hovered::Tree(crate::hover::Hover::Value(
                            std::rc::Rc::from([])
                        ))))
                    );
                    let outlines = [inert, interactive].map(|mut placed| {
                        let mut canvas = TestCanvas(DrawList::new());
                        puri::frame::render(placed.resolve(no_ink()), &mut canvas);
                        assert_eq!(placed.handler.is_some(), placed.claim.is_some());
                        let [
                            DrawCmd::Fill {
                                shape: Shape::Path(path),
                                transform,
                                ..
                            },
                        ] = &canvas.0.0[..]
                        else {
                            panic!("delimiter outline")
                        };
                        let bounds = transform.transform_rect_bbox(path.bounding_box());
                        assert!(bounds.x0 >= placement.rect.x0 && bounds.x1 <= placement.rect.x1);
                        assert!((bounds.y0 - placement.rect.y0).abs() < 1e-6);
                        assert!((bounds.y1 - placement.rect.y1).abs() < 1e-6);
                        (path.clone(), *transform)
                    });
                    assert_eq!(outlines[0], outlines[1]);
                }
            }
        }
    }

    #[test]
    fn native_delimiter_actions_follow_retained_hover_and_view_ownership() {
        let owner = Root::document();
        let other = Root::document();
        let measured = in_view(
            native_delimiter(true, 1.0, Extent::default(), puri::delim::Side::Open),
            owner.clone(),
        );
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let rect = measured.extent.rect_at(Point::new(20.0, 30.0));
        let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
        let point = Point::new(rect.x1 + 1.0, rect.center().y);
        let mut placed = crate::display::widget::frame::place(
            measured,
            Placement::new(rect, bounds),
            &HoverInput {
                pointer: Some(point),
                prior: Some(&target),
                reach: 3.0,
                debug_geometry: false,
            },
        );
        assert!(matches!(
            placed.claim.clone().map(|(_, claim)| claim),
            Some(Claim::Extended(_))
        ));
        placed.resolve(Default::default());
        for (root, hovered, picking, expected) in [
            (
                Some(owner.clone()),
                Some(target.clone()),
                false,
                Some("select"),
            ),
            (
                Some(owner.clone()),
                Some(target.clone()),
                true,
                Some("pick"),
            ),
            (Some(other), Some(target.clone()), false, None),
            (Some(owner), None, false, None),
        ] {
            let mut event = down_at(point.x, point.y);
            if picking {
                event.state.modifiers =
                    ui_events::keyboard::Modifiers::META | ui_events::keyboard::Modifiers::CONTROL;
            }
            let mut world = crate::test_editor(gid::Document {
                root: None,
                cells: gid::Cells::new(),
            });
            if picking {
                world.model.selection = Some(crate::selection::pending_value(
                    world.model.workspace.document_root(),
                    vec![],
                ));
            }
            let handled = placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
                &mut world,
                &event,
                &mut DispatchContext::new(root, hovered),
            );
            assert_eq!(handled, expected.is_some());
            match expected {
                Some("pick") => assert_eq!(world.model.doc.root, Some(gid::Value::record([]))),
                Some("select") => assert!(world.model.selection.is_some()),
                _ => {}
            }
        }
    }

    #[test]
    fn clipped_native_delimiters_add_no_ink_hover_or_actions() {
        let measured = native_delimiter(true, 1.0, Extent::default(), puri::delim::Side::Close);
        let rect = measured.extent.rect_at(Point::new(20.0, 30.0));
        let mut placed = crate::display::widget::frame::place(
            measured,
            Placement::new(rect, Rect::ZERO),
            &Default::default(),
        );
        assert!(placed.resolve(Default::default()).is_empty());
        assert!(placed.claim.is_none());
        assert!(placed.handler.is_none());
    }

    fn move_at(x: f64, y: f64) -> PointerUpdate {
        PointerUpdate {
            pointer: pointer(),
            current: state_at(x, y),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        }
    }

    fn scroll_at(x: f64, y: f64) -> PointerScrollEvent {
        PointerScrollEvent {
            pointer: pointer(),
            delta: ScrollDelta::LineDelta(0.0, 1.0),
            state: state_at(x, y),
        }
    }

    #[test]
    fn target_actions_and_raw_handlers_share_visual_precedence() {
        let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
        let other = Hovered::Tree(crate::hover::Hover::Toggle(std::rc::Rc::from([])));
        let placement = Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0));
        let mut frame = HoverOutput::default();
        let mut placed: HoverContext<'_, Vec<&'static str>, Hovered> = HoverContext::new(
            HoverInput {
                pointer: Some(Point::new(5.0, 5.0)),
                ..Default::default()
            },
            &mut frame,
        );
        let mut p = Builder::new(&mut placed, placement);
        p.handler().on_pointer_down(|log, _| {
            log.push("raw below");
            true
        });
        p.activate(target.clone(), |log| {
            log.push("activation");
            true
        });
        p.activate(other, |_| panic!("wrong target"));
        p.handler().on_pointer_down(|log, _| {
            log.push("raw above declined");
            false
        });
        let mut placed = frame;
        let mut pointer = DispatchContext::new(None, Some(target));
        let mut log = Vec::new();
        assert!(placed.handler.take().unwrap().dispatch_pointer_down_with(
            &mut log,
            &down_at(5.0, 5.0),
            &mut pointer
        ));
        assert_eq!(log, ["raw above declined", "activation"]);
    }

    #[test]
    fn direct_placement_preserves_handler_precedence() {
        let extent = Extent {
            width: 10.0,
            ascent: 5.0,
            descent: 5.0,
        };
        let lower = leaf(extent, |p: &mut Builder<'_, '_, Vec<&'static str>>, _| {
            p.handler().on_pointer_down(|log, _| {
                log.push("lower");
                true
            });
        });
        let upper = leaf(extent, |p: &mut Builder<'_, '_, Vec<&'static str>>, _| {
            p.handler().on_pointer_down(|log, _| {
                log.push("upper");
                false
            });
        });
        let mut placed = {
            let layout = measured::layers(vec![lower, upper]);
            let placement = puri::Placement::root(layout.extent.rect_at(Point::ZERO));
            crate::display::widget::frame::place(layout, placement, &Default::default())
        };
        placed.resolve(Default::default());
        let handler = placed.handler.expect("registrations");
        let mut log = Vec::new();

        assert!(handler.dispatch_pointer_down(&mut log, &down_at(5.0, 5.0)));
        assert_eq!(log, ["upper", "lower"]);
    }

    #[test]
    fn popover_content_is_out_of_flow_and_raised_over_the_frame() {
        let trigger = leaf(
            Extent {
                width: 10.0,
                ascent: 0.0,
                descent: 10.0,
            },
            |p: &mut Builder<'_, '_, ()>, placement| {
                p.fill(placement.rect, Color::BLACK, Affine::IDENTITY);
            },
        );
        let content = leaf(
            Extent {
                width: 20.0,
                ascent: 0.0,
                descent: 20.0,
            },
            |p: &mut Builder<'_, '_, ()>, placement| {
                p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
            },
        );
        let popup = floating(trigger, content, |placement, extent| {
            crate::display::widget::popover::position(placement, extent, 2.0)
        });

        assert_eq!(popup.extent.width, 10.0);
        assert_eq!(popup.extent.height(), 10.0);

        let mut placed = crate::display::widget::frame::place(
            popup,
            Placement::new(
                Rect::new(5.0, 5.0, 15.0, 15.0),
                Rect::new(0.0, 0.0, 100.0, 100.0),
            ),
            &Default::default(),
        );

        let mut canvas = TestCanvas(DrawList::new());
        puri::frame::render(placed.resolve(no_ink()), &mut canvas);
        assert!(matches!(
            &canvas.0.0[..],
            [
                DrawCmd::Fill {
                    shape: Shape::Rect(trigger),
                    ..
                },
                DrawCmd::Fill {
                    shape: Shape::Rect(content),
                    ..
                }
            ] if *trigger == Rect::new(5.0, 5.0, 15.0, 15.0)
                && *content == Rect::new(5.0, 17.0, 25.0, 37.0)
        ));
    }

    #[test]
    fn debug_geometry_outlines_the_leafs_placement() {
        let child = leaf(
            Extent {
                width: 12.0,
                ascent: 4.0,
                descent: 6.0,
            },
            |p: &mut Builder<'_, '_, ()>, placement| {
                p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
            },
        );
        let rect = Rect::new(3.0, 5.0, 15.0, 15.0);
        let mut placed = crate::display::widget::frame::place(
            child,
            Placement::root(rect),
            &HoverInput {
                debug_geometry: true,
                ..Default::default()
            },
        );
        let mut canvas = TestCanvas(DrawList::new());
        puri::frame::render(placed.resolve(Default::default()), &mut canvas);

        assert!(matches!(
            &canvas.0.0[..],
            [
                DrawCmd::Fill {
                    shape: Shape::Rect(fill),
                    ..
                },
                DrawCmd::Stroke {
                    shape: Shape::Rect(outline),
                    ..
                }
            ] if *fill == rect && *outline == rect
        ));
    }

    #[test]
    fn clipped_leaves_keep_handlers_but_contribute_no_visible_work() {
        let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
        let child = leaf(
            Extent {
                width: 10.0,
                ascent: 5.0,
                descent: 5.0,
            },
            move |p: &mut Builder<'_, '_, ()>, placement| {
                p.claim(placement, target.clone());
                p.activate(target.clone(), |_| true);
                p.pick(target, |_| true);
                p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
                p.handler().on_pointer_move(|_, _| true);
            },
        );
        let mut placed = crate::display::widget::frame::place(
            child,
            Placement::new(
                Rect::new(20.0, 20.0, 30.0, 30.0),
                Rect::new(0.0, 0.0, 10.0, 10.0),
            ),
            &Default::default(),
        );

        assert!(placed.claim.is_none());
        assert!(placed.resolve(Default::default()).is_empty());
        assert!(placed.handler.is_some());
    }

    #[test]
    fn scrolled_content_shifts_inside_the_viewport_clip() {
        let probe = leaf(
            Extent {
                width: 100.0,
                ascent: 0.0,
                descent: 300.0,
            },
            |p: &mut Builder<'_, '_, ()>, placement| {
                assert_eq!(placement.clip_rect, Rect::new(10.0, 20.0, 90.0, 70.0));
                p.fill(
                    Rect::new(
                        placement.rect.x0,
                        placement.rect.y0,
                        placement.rect.x0 + 1.0,
                        placement.rect.y0 + 1.0,
                    ),
                    Color::WHITE,
                    Affine::IDENTITY,
                );
            },
        );
        let mut placed = crate::display::widget::frame::place(
            scrolled_at(probe, Vec2::new(5.0, 40.0), None, |_, event| {
                ScrollOutcome::pass(event)
            }),
            Placement::new(
                Rect::new(10.0, 20.0, 90.0, 70.0),
                Rect::new(0.0, 0.0, 100.0, 100.0),
            ),
            &Default::default(),
        );
        let mut canvas = TestCanvas(DrawList::new());
        puri::frame::render(placed.resolve(no_ink()), &mut canvas);
        let [
            DrawCmd::Clip {
                shape: Shape::Rect(clip),
                children,
                ..
            },
        ] = &canvas.0.0[..]
        else {
            panic!("expected one clip");
        };
        assert_eq!(*clip, Rect::new(10.0, 20.0, 90.0, 70.0));
        let [
            DrawCmd::Fill {
                shape: Shape::Rect(dot),
                ..
            },
        ] = &children[..]
        else {
            panic!("expected the probe inside the clip");
        };
        assert_eq!((dot.x0, dot.y0), (5.0, -20.0));
    }

    #[test]
    fn scroll_viewport_bounds_starts_and_not_active_motion_or_release() {
        let child = leaf(
            Extent {
                width: 30.0,
                ascent: 0.0,
                descent: 30.0,
            },
            |p: &mut Builder<'_, '_, Vec<&'static str>>, _| {
                p.handler().on_pointer_down(|log, _| {
                    log.push("down");
                    true
                });
                p.handler().on_pointer_move(|log, _| {
                    log.push("move");
                    true
                });
                p.handler().on_pointer_up(|log, _| {
                    log.push("up");
                    true
                });
            },
        );
        let mut placed = crate::display::widget::frame::place(
            scrolled_at(
                child,
                Vec2::ZERO,
                None,
                |log: &mut Vec<&'static str>, event| {
                    log.push("scroll");
                    ScrollOutcome::consume(event)
                },
            ),
            Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
            &Default::default(),
        );
        placed.resolve(Default::default());
        let handler = placed.handler.expect("registrations");
        let mut log = Vec::new();
        assert!(!handler.dispatch_pointer_down(&mut log, &down_at(20.0, 5.0)));
        assert!(
            !handler
                .dispatch_scroll(&mut log, &scroll_at(20.0, 5.0))
                .handled()
        );
        assert!(handler.dispatch_pointer_down(&mut log, &down_at(5.0, 5.0)));
        assert!(
            handler
                .dispatch_scroll(&mut log, &scroll_at(5.0, 5.0))
                .handled()
        );
        assert!(handler.dispatch_pointer_move(&mut log, &move_at(20.0, 5.0)));
        assert!(handler.dispatch_pointer_up(&mut log, &down_at(20.0, 5.0)));
        assert_eq!(log, ["down", "scroll", "move", "up"]);
    }
    #[test]
    fn occlusion_blocks_clicks_in_its_clip_but_not_active_gestures() {
        let mut frame = HoverOutput::default();
        let mut placed: HoverContext<'_, Vec<&'static str>, Hovered> = HoverContext::new(
            HoverInput {
                pointer: Some(Point::new(5.0, 5.0)),
                ..Default::default()
            },
            &mut frame,
        );
        let full = Placement::root(Rect::new(0.0, 0.0, 100.0, 100.0));
        let mut p = Builder::new(&mut placed, full);
        p.handler().on_pointer_down(|log, _| {
            log.push("covered");
            true
        });
        p.handler().on_pointer_move(|log, _| {
            log.push("move");
            true
        });
        p.handler().on_pointer_up(|log, _| {
            log.push("up");
            true
        });
        p.occlude(Placement::new(full.rect, Rect::new(10.0, 10.0, 20.0, 20.0)));
        let mut placed = frame;
        placed.resolve(Default::default());
        let handler = placed.handler.take().unwrap();
        let mut log = Vec::new();
        for button in [PointerButton::Primary, PointerButton::Secondary] {
            let mut event = down_at(15.0, 15.0);
            event.button = Some(button);
            assert!(handler.dispatch_pointer_down(&mut log, &event));
            assert!(log.is_empty());
        }
        assert!(handler.dispatch_pointer_down(&mut log, &down_at(25.0, 15.0)));
        assert!(handler.dispatch_pointer_move(&mut log, &move_at(150.0, 150.0)));
        assert!(handler.dispatch_pointer_up(&mut log, &down_at(150.0, 150.0)));
        assert_eq!(log, ["covered", "move", "up"]);
    }

    #[test]
    fn a_floater_keeps_its_view_when_it_covers_another_view() {
        let owner = Root::document();
        let covered = Root::document();
        let target = Hovered::Tree(crate::hover::Hover::Entry(0));
        let extent = Extent {
            width: 10.0,
            ascent: 0.0,
            descent: 10.0,
        };
        let row = |label: &'static str| {
            let target = target.clone();
            leaf(
                extent,
                move |p: &mut Builder<'_, '_, Vec<&'static str>>, placement| {
                    p.claim(placement, target.clone());
                    p.activate(target.clone(), move |log| {
                        log.push(label);
                        true
                    });
                    p.pick(target, move |log| {
                        log.push(label);
                        true
                    });
                },
            )
        };
        let popup_rect = Rect::new(50.0, 0.0, 60.0, 10.0);
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let owner_view = in_view(
            floating(
                leaf(extent, |_: &mut Builder<'_, '_, Vec<&'static str>>, _| {}),
                row("popup"),
                move |_, _| Some(Placement::new(popup_rect, bounds)),
            ),
            owner.clone(),
        );
        let mut pass = HoverPass::new(&HoverInput {
            pointer: Some(popup_rect.center()),
            ..Default::default()
        });
        measured::place_into(
            owner_view,
            Placement::new(extent.rect_at(Point::ZERO), bounds),
            &mut pass,
        );
        measured::place_into(
            in_view(row("covered pane"), covered),
            Placement::new(popup_rect, bounds),
            &mut pass,
        );
        let mut placed = pass.finish();
        placed.resolve(Default::default());
        let point = popup_rect.center();
        let (root, Claim::Direct(hit)) = placed.claim.clone().unwrap() else {
            panic!("popup hover")
        };
        assert!(root.as_ref() == Some(&owner));
        for modifiers in [
            ui_events::keyboard::Modifiers::empty(),
            ui_events::keyboard::Modifiers::META | ui_events::keyboard::Modifiers::CONTROL,
        ] {
            let mut pointer = DispatchContext::new(root.clone(), Some(hit.clone()));
            let mut event = down_at(point.x, point.y);
            event.state.modifiers = modifiers;
            let mut log = Vec::new();
            assert!(placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
                &mut log,
                &event,
                &mut pointer
            ));
            assert_eq!(log, ["popup"]);
        }
    }

    #[test]
    fn native_floaters_keep_their_view_and_raise_above_later_content() {
        use crate::display::widget;
        let owner = Root::document();
        let other = Root::pane(vec![]);
        let target = Hovered::Tree(crate::hover::Hover::Entry(0));
        let expected = target.clone();
        let rect = Rect::new(50.0, 0.0, 70.0, 20.0);
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let popup = widget::leaf(
            Extent {
                width: 20.0,
                ascent: 0.0,
                descent: 20.0,
            },
            move |output: &mut widget::HoverContext<'_, usize, _>, placement| {
                output.claim(crate::display::widget::frame::Probe::retaining(
                    placement,
                    target.clone(),
                ));
                output
                    .handler()
                    .on_pointer_down_with(move |count, _, hovered| {
                        hovered.hovered() == Some(&target) && {
                            *count += 1;
                            true
                        }
                    });
            },
        );
        let popup = widget::navigation::landmark(
            popup,
            std::rc::Rc::from([gid::Step::Follow(gid::Resolution::Document)]),
            std::rc::Rc::new(|count, _| {
                *count += 10;
                true
            }),
        );
        let widget = container::floating(
            widget::leaf(Extent::default(), |_, _| {}),
            popup,
            move |_, _| Some(Placement::new(rect, bounds)),
        );
        let native = measured::leaf_into(widget.extent, move |placement, pass| {
            measured::place_into(widget, placement, pass)
        });
        let mut pass = HoverPass::new(&HoverInput {
            pointer: Some(rect.center()),
            ..Default::default()
        });
        measured::place_into(
            in_view(native, owner.clone()),
            Placement::root(bounds),
            &mut pass,
        );
        measured::place_into(
            in_view(
                leaf(Extent::default(), |output, placement| {
                    output.occlude(placement)
                }),
                other.clone(),
            ),
            Placement::root(bounds),
            &mut pass,
        );
        let mut output = pass.finish();
        output.resolve(Default::default());
        let [landmark] = output.descends.as_slice() else {
            panic!("native popup must contribute exactly one landmark");
        };
        assert_eq!(landmark.root, Some(owner.clone()));
        assert_eq!(landmark.rect, rect);
        assert_eq!(
            landmark.path.as_ref(),
            &[gid::Step::Follow(gid::Resolution::Document)]
        );
        let mut count = 0;
        assert!((landmark.select)(
            &mut count,
            Some(crate::navigate::Direction::Left)
        ));
        assert_eq!(count, 10);
        let (root, claim) = output.claim.clone().unwrap();
        assert_eq!(root, Some(owner.clone()));
        assert_eq!(claim, Claim::Direct(expected.clone()));
        let handler = output.handler.unwrap();
        let mut count = 0;
        assert!(handler.dispatch_pointer_down_with(
            &mut count,
            &down_at(55.0, 5.0),
            &mut DispatchContext::new(Some(owner), Some(expected))
        ));
        assert_eq!(count, 1);
    }

    #[test]
    fn semantic_actions_obey_scroll_clips_even_when_the_target_matches() {
        let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
        let claimed = target.clone();
        let child = leaf(
            Extent {
                width: 30.0,
                ascent: 0.0,
                descent: 30.0,
            },
            move |p: &mut Builder<'_, '_, usize>, placement| {
                p.claim(placement, claimed.clone());
                p.activate(claimed, |count| {
                    *count += 1;
                    true
                });
            },
        );
        let mut placed = crate::display::widget::frame::place(
            scrolled_at(child, Vec2::ZERO, None, |_, event| {
                ScrollOutcome::pass(event)
            }),
            Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
            &Default::default(),
        );
        let mut pointer = DispatchContext::new(None, Some(target));
        placed.resolve(Default::default());
        let handler = placed.handler.unwrap();
        let mut count = 0;
        assert!(!handler.dispatch_pointer_down_with(&mut count, &down_at(20.0, 5.0), &mut pointer));
        assert_eq!(count, 0);
        assert!(handler.dispatch_pointer_down_with(&mut count, &down_at(5.0, 5.0), &mut pointer));
        assert_eq!(count, 1);
    }
    #[test]
    fn dynamic_picks_share_pointer_order_and_respect_occlusion() {
        let target = Hovered::Tree(crate::hover::Hover::Drawing(
            crate::hover::SourceTrace::Stored(std::rc::Rc::from([])),
        ));
        let placement = Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0));
        for covered in [false, true] {
            let mut frame = HoverOutput::default();
            let mut placed: HoverContext<'_, usize, Hovered> = HoverContext::new(
                HoverInput {
                    pointer: Some(Point::new(5.0, 5.0)),
                    ..Default::default()
                },
                &mut frame,
            );
            let mut p = Builder::new(&mut placed, placement);
            p.handler()
                .on_pointer_down(|_, _| panic!("covered raw handler"));
            p.occlude(placement);
            let claimed = target.clone();
            p.claim_dynamic(placement, move |_| Some(claimed.clone()));
            p.pick_dynamic(placement, |count, target, _| {
                if matches!(target, Hovered::Tree(crate::hover::Hover::Drawing(_))) {
                    *count += 1;
                    true
                } else {
                    false
                }
            });
            if covered {
                p.occlude(placement);
            }
            let mut placed = frame;
            let hovered = match placed.claim.take().map(|(_, claim)| claim) {
                Some(Claim::Direct(target)) => target,
                Some(Claim::Occludes) => Hovered::Blocked,
                _ => panic!("hit"),
            };
            let mut pointer = DispatchContext::new(None, Some(hovered));
            let mut event = down_at(5.0, 5.0);
            event.state.modifiers =
                ui_events::keyboard::Modifiers::META | ui_events::keyboard::Modifiers::CONTROL;
            let mut count = 0;
            assert!(placed.handler.take().unwrap().dispatch_pointer_down_with(
                &mut count,
                &event,
                &mut pointer
            ));
            assert_eq!(count, usize::from(!covered));
        }
    }
}
