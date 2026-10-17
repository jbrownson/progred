//! Progred adapters for the placement → hover → paint/dispatch chain.
//! The shared after-hover boundary keeps paint optional and resolves targets
//! before either painting or dispatch sees the frame.

use crate::completion::Offers;
use crate::display::widget::container::{self, Layers};
use crate::frame::Hovered;
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
#[cfg(test)]
pub type HoverOutput<C> = crate::display::widget::HoverOutput<C, Hovered>;
pub use crate::display::widget::{HoverContext, HoverInput};
pub type DispatchContext<C> = crate::display::widget::frame::DispatchContext<C, Hovered>;
pub type Attribution = crate::display::widget::frame::Attribution<Hovered>;
pub use puri::frame::Render;
pub type Probe = crate::display::widget::frame::Probe<Hovered>;
pub use crate::display::widget::frame::{Cursor, ViewRegion};

impl DispatchContext<crate::Editor> {
    pub(crate) fn geometry(&self, scale: f64) -> crate::navigate::Geometry<'_> {
        crate::navigate::Geometry {
            descends: &self.descends,
            view_regions: &self.view_regions,
            scale,
        }
    }
}

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

    /// The platform's command key, for the handlers this builder installs.
    pub fn command_modifier(&self) -> puri::keyboard::CommandModifier {
        self.placed.input.command_modifier
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

    /// Show `cursor` while `target` is hovered.
    pub fn cursor(&mut self, target: Hovered, cursor: Cursor) {
        self.placed.cursor(target, cursor);
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
            let command = self.placed.input.command_modifier;
            self.handler()
                .on_pointer_down_with(move |ctx, event, pointer| {
                    puri::interact::is_primary_contact(event)
                        && command.pressed(&event.state.modifiers) == pick
                        && pointer.matches(&target)
                        && action(ctx, event)
                });
        }
    }

    /// Defer painting until this frame's hover has settled.
    pub fn render(
        &mut self,
        render: impl FnOnce(&mut dyn puri::draw::CanvasSink, &Attribution) + 'static,
    ) {
        if self.visible {
            self.placed.render(render);
        }
    }
}

impl<C: 'static> puri::draw::CanvasSink for Builder<'_, '_, C> {
    fn draw_mesh(&mut self, scene: puri::mesh::Scene, transform: Affine) {
        if self.visible {
            self.placed.render(move |cv, _| cv.mesh(scene, transform));
        }
    }
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

/// A scroll viewport, tagged as an editor view when it has an owner. The
/// settled region is frame output, not retained widget state; input and
/// keyboard reveal use it to update the same caller-owned view.
pub fn scrolled_at<C: 'static>(
    child: Measured<HoverPass<C>>,
    offset: Vec2,
    owner: Option<(Root, f64)>,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<HoverPass<C>> {
    let extent = child.extent;
    let scale = owner.as_ref().map(|(_, scale)| *scale).unwrap_or(1.0);
    let scrolled = container::scrolled(child, offset, scale, on_scroll);
    before(scrolled, move |output, placement| {
        if let Some((root, scale)) = owner {
            output.placed.view_region(ViewRegion {
                root,
                rect: placement.rect,
                maximum: Vec2::new(
                    ((extent.width - placement.rect.width()) / scale).max(0.0),
                    ((extent.height() - placement.rect.height()) / scale).max(0.0),
                ),
                content: Some(Vec2::new(extent.width, extent.height()) / scale),
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
                content: None,
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
mod tests;
