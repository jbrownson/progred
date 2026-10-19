//! Construction inside a widget's hover continuation, for any world and
//! whatever its pointer rests on. Claims are answered now; ink and handlers
//! are returned for later execution, and a clipped-out placement draws and
//! claims nothing.

use super::frame::{Attribution, Cursor, DispatchContext, Probe};
use super::offers::Offers;
use super::{HoverContext, HoverPass, extent};
use kurbo::{Affine, Point, Rect, Stroke};
use measured::{Extent, Measured};
use peniko::{Brush, ImageData};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::LineEditDescription;
use puri::handler::{Handler, HasHandler};
use puri::text::{TextCtx, TextStyle};
use ui_events::keyboard::KeyboardEvent;
use uig::Placement;

pub struct Builder<'a, 'input, C: 'static, H: 'static> {
    pub(crate) placed: &'a mut HoverContext<'input, C, H>,
    visible: bool,
}

impl<'builder, 'input, C: 'static, H: 'static> Builder<'builder, 'input, C, H> {
    pub(crate) fn new(
        placed: &'builder mut HoverContext<'input, C, H>,
        placement: Placement,
    ) -> Self {
        Self {
            placed,
            visible: !placement.clipped_out(),
        }
    }

    /// The platform's command key, for the handlers this builder installs.
    pub fn command_modifier(&self) -> puri::keyboard::CommandModifier {
        self.placed.input.command_modifier
    }

    /// Defer painting until this frame's hover has settled.
    pub fn render(
        &mut self,
        render: impl FnOnce(&mut dyn puri::draw::CanvasSink, &Attribution<H>) + 'static,
    ) {
        if self.visible {
            self.placed.render(render);
        }
    }

    pub fn completion(&mut self) -> &mut Option<Offers<C>> {
        self.placed.completion()
    }
}

impl<C: 'static, H: Clone + PartialEq + 'static> Builder<'_, '_, C, H> {
    /// Contribute a named hover region.
    pub fn claim(&mut self, placement: Placement, target: H) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::retaining(placement, target));
        }
    }

    /// Contribute a named hover region without the ordinary air-gap
    /// retention. Useful for chrome whose pointer feedback should track
    /// its hit geometry exactly.
    pub fn claim_exact(&mut self, placement: Placement, target: H) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::exact(placement, target));
        }
    }

    /// Show `cursor` while `target` is hovered.
    pub fn cursor(&mut self, target: H, cursor: Cursor) {
        self.placed.cursor(target, cursor);
    }

    pub fn claim_dynamic(
        &mut self,
        placement: Placement,
        target_at: impl Fn(Point) -> Option<H> + 'static,
    ) {
        if !placement.clipped_out() {
            self.placed.claim(Probe::dynamic(placement, target_at));
        }
    }

    /// Contribute an unnamed region that blocks targets below it.
    pub fn occlude(&mut self, placement: Placement) {
        self.placed.occlude(placement);
    }

    /// Run `action` on a primary press over `target`.
    pub fn activate(&mut self, target: H, action: impl Fn(&mut C) -> bool + 'static) {
        if self.visible {
            self.placed
                .on_press(target, move |world, _| action(world), |_, _| false);
        }
    }

    /// Run `action` on a primary press over `target` with the command key held.
    pub fn pick(&mut self, target: H, action: impl Fn(&mut C) -> bool + 'static) {
        if self.visible {
            self.placed
                .on_press(target, |_, _| false, move |world, _| action(world));
        }
    }
}

impl<C: 'static, H: 'static> puri::draw::CanvasSink for Builder<'_, '_, C, H> {
    fn draw_mesh(&mut self, scene: puri::mesh::Scene, transform: Affine) {
        if self.visible {
            self.placed
                .render(move |cv, _| cv.draw_mesh(scene, transform));
        }
    }
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        if self.visible {
            self.placed
                .render(move |cv, _| cv.draw_image(image, transform));
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
            self.placed.render(move |cv, _| cv.draw_glyphs(run));
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

impl<C: 'static, H: 'static> HasHandler<C> for Builder<'_, '_, C, H> {
    type Input = DispatchContext<C, H>;

    fn handler(&mut self) -> &mut Handler<C, DispatchContext<C, H>> {
        self.placed.handler()
    }
}

fn built_into<C: 'static, H: 'static>(
    f: impl FnOnce(&mut Builder<'_, '_, C, H>, Placement) + 'static,
) -> impl FnOnce(Placement, &mut HoverContext<'_, C, H>) + 'static {
    move |placement, placed| f(&mut Builder::new(placed, placement), placement)
}

pub fn leaf<C: 'static, H: 'static>(
    extent: Extent,
    place: impl FnOnce(&mut Builder<'_, '_, C, H>, Placement) + 'static,
) -> Measured<HoverPass<C, H>> {
    super::leaf(extent, move |output, placement| {
        built_into(place)(placement, output)
    })
}

pub fn before<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    place_before: impl FnOnce(&mut Builder<'_, '_, C, H>, Placement) + 'static,
) -> Measured<HoverPass<C, H>> {
    super::before_place(child, built_into(place_before))
}

pub fn decorate<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    draw: impl FnOnce(&mut Builder<'_, '_, C, H>, Rect) + 'static,
) -> Measured<HoverPass<C, H>> {
    before(child, move |p, placement| draw(p, placement.rect))
}

pub fn on_key<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    action: impl Fn(&mut C, &KeyboardEvent) -> bool + 'static,
) -> Measured<HoverPass<C, H>> {
    before(child, move |p, _| {
        p.handler().on_key(action);
    })
}

/// `s` in `style`, as a leaf.
pub fn text<C: 'static, H: 'static>(
    ctx: &mut TextCtx,
    s: &str,
    style: &TextStyle,
) -> Measured<HoverPass<C, H>> {
    shaped_text(puri::text::text(ctx, s, style))
}

pub fn shaped_text<C: 'static, H: 'static>(text: puri::text::Text) -> Measured<HoverPass<C, H>> {
    leaf(extent(text.metrics()), move |canvas, placement| {
        text.place(canvas, placement)
    })
}

/// A line editor as a leaf, editing through `with`.
pub fn text_edit<C: 'static, H: 'static>(
    description: LineEditDescription<'_>,
    tcx: &mut TextCtx,
    with: impl Fn(&mut C, &puri::edit::EditOperation<'_>) -> bool + Clone + 'static,
) -> Measured<HoverPass<C, H>> {
    let edit = puri::edit::text_edit(description, tcx);
    super::leaf(extent(edit.metrics()), move |output, placement| {
        output.line_edit(edit, placement, with)
    })
}
