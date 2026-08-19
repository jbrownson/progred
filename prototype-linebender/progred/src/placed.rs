//! Progred's placement output: the frame with its pixels still
//! latent. Placement folds every leaf's contribution into one
//! [`Placed`] — hover probes, the composed handler, keyboard
//! geometry, the popup stash, and deferred ink — combined in
//! placement order, so the later contribution is on top: painted
//! last, asked first.

use crate::completion::{HasPopup, Popup};
use crate::frame::Hovered;
use crate::navigate::{Descend, HasDescends};
use measured::{Extent, Measured, Output};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::handler::{Handler, HasHandler};
use puri::hover::Claim;
use puri::text::TextMetrics;
use uig::Placement;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent};
use vello::kurbo::{Affine, Point, Rect, Stroke, Vec2};
use vello::peniko::Brush;

pub type Probe = Box<dyn Fn(Point) -> Option<Claim<Hovered>>>;
pub type Render<Cv> = Box<dyn FnOnce(&mut Cv)>;

pub struct Placed<C, Cv> {
    pub probes: Vec<Probe>,
    /// `None` until something registers: combining empty frames must
    /// not deepen the dispatch chain.
    pub handler: Option<Handler<C>>,
    pub descends: Vec<Descend>,
    pub popup: Option<Popup>,
    pub renders: Vec<Render<Cv>>,
}

impl<C: 'static, Cv> Output for Placed<C, Cv> {
    fn empty() -> Self {
        Self {
            probes: Vec::new(),
            handler: None,
            descends: Vec::new(),
            popup: None,
            renders: Vec::new(),
        }
    }

    fn over(mut self, above: Self) -> Self {
        self.probes.extend(above.probes);
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(handler_over(base, above)),
        };
        self.descends.extend(above.descends);
        self.popup = above.popup.or(self.popup);
        self.renders.extend(above.renders);
        self
    }
}

impl<C: 'static, Cv> Placed<C, Cv> {
    /// What the pointer at `point` rests on: the topmost claim.
    pub fn probe(&self, point: Point) -> Option<Claim<Hovered>> {
        self.probes.iter().rev().find_map(|probe| probe(point))
    }

    pub fn handler_mut(&mut self) -> &mut Handler<C> {
        self.handler.get_or_insert_with(Handler::new)
    }

    /// Run the deferred ink into a canvas.
    pub fn render(renders: Vec<Render<Cv>>, canvas: &mut Cv) {
        for render in renders {
            render(canvas);
        }
    }
}

/// Stack `above`'s dispatch over `base`'s: above tries first, declines
/// fall through — placement order as precedence, same as paint.
fn handler_over<C: 'static>(base: Handler<C>, above: Handler<C>) -> Handler<C> {
    fn chain<C, E>(
        base: Box<dyn Fn(&mut C, &E) -> bool>,
        above: Box<dyn Fn(&mut C, &E) -> bool>,
    ) -> Box<dyn Fn(&mut C, &E) -> bool>
    where
        C: 'static,
        E: 'static,
    {
        Box::new(move |ctx, event| above(ctx, event) || base(ctx, event))
    }
    Handler {
        pointer_down: chain(base.pointer_down, above.pointer_down),
        pointer_move: chain(base.pointer_move, above.pointer_move),
        pointer_up: chain(base.pointer_up, above.pointer_up),
        scroll: chain(base.scroll, above.scroll),
        key: chain(base.key, above.key),
        ime: chain(base.ime, above.ime),
    }
}

/// The leaf-construction context: today's placement-pass interface,
/// accumulating a [`Placed`] instead of drawing and registering
/// against live machinery. Ink defers; everything else settles here.
pub struct Builder<C: 'static, Cv> {
    placed: Placed<C, Cv>,
}

impl<C: 'static, Cv> Builder<C, Cv> {
    fn new() -> Self {
        Self {
            placed: Placed::empty(),
        }
    }

    /// Contribute a hover probe: asked when the frame wants to know
    /// what the pointer rests on, topmost contribution first.
    pub fn claim(&mut self, probe: impl Fn(Point) -> Option<Claim<Hovered>> + 'static) {
        self.placed.probes.push(Box::new(probe));
    }
}

impl<C: 'static, Cv: Canvas + 'static> Canvas for Builder<C, Cv> {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        let (shape, brush) = (shape.into(), brush.into());
        self.placed
            .renders
            .push(Box::new(move |cv| cv.fill(shape, brush, transform)));
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        let (shape, brush) = (shape.into(), brush.into());
        self.placed
            .renders
            .push(Box::new(move |cv| cv.stroke(shape, style, brush, transform)));
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        self.placed
            .renders
            .push(Box::new(move |cv| cv.glyph_run(run)));
    }

    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let shape = shape.into();
        let mut inner = Self::new();
        content(&mut inner);
        let mut placed = inner.placed;
        let renders = std::mem::take(&mut placed.renders);
        placed.renders.push(Box::new(move |cv: &mut Cv| {
            cv.clip(shape, transform, |cv| Placed::<C, Cv>::render(renders, cv))
        }));
        let base = std::mem::replace(&mut self.placed, Placed::empty());
        self.placed = base.over(placed);
    }
}

impl<C: 'static, Cv> HasHandler<C> for Builder<C, Cv> {
    fn handler(&mut self) -> &mut Handler<C> {
        self.placed.handler_mut()
    }
}

impl<C: 'static, Cv> HasDescends for Builder<C, Cv> {
    fn descends(&mut self) -> &mut Vec<Descend> {
        &mut self.placed.descends
    }
}

impl<C: 'static, Cv> HasPopup for Builder<C, Cv> {
    fn popup(&mut self) -> &mut Option<Popup> {
        &mut self.placed.popup
    }
}

/// Adapt an imperative leaf body to a staged placement continuation.
pub fn built<C: 'static, Cv: 'static>(
    f: impl FnOnce(&mut Builder<C, Cv>, Placement) + 'static,
) -> impl FnOnce(Placement) -> Placed<C, Cv> + 'static {
    move |placement| {
        let mut builder = Builder::new();
        f(&mut builder, placement);
        builder.placed
    }
}

pub fn leaf<C: 'static, Cv: 'static>(
    extent: Extent,
    place: impl FnOnce(&mut Builder<C, Cv>, Placement) + 'static,
) -> Measured<Placed<C, Cv>> {
    measured::leaf(extent, built(place))
}

pub fn before<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    place_before: impl FnOnce(&mut Builder<C, Cv>, Placement) + 'static,
) -> Measured<Placed<C, Cv>> {
    measured::before(child, built(place_before))
}

pub fn after<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    place_after: impl FnOnce(&mut Builder<C, Cv>, Placement) + 'static,
) -> Measured<Placed<C, Cv>> {
    measured::after(child, built(place_after))
}

pub fn decorate<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    draw: impl FnOnce(&mut Builder<C, Cv>, Rect) + 'static,
) -> Measured<Placed<C, Cv>> {
    before(child, move |p, placement| draw(p, placement.rect))
}

pub fn on_key<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    action: impl Fn(&mut C, &KeyboardEvent) -> bool + 'static,
) -> Measured<Placed<C, Cv>> {
    before(child, move |p, _| {
        p.handler().on_key(action);
    })
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn on_primary_pointer_down<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    accepts: impl Fn(&PointerButtonEvent) -> bool + 'static,
    action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
) -> Measured<Placed<C, Cv>> {
    before(child, move |p, placement| {
        puri::interact::on_primary_pointer_down(p, placement, accepts, action);
    })
}

/// Place `child` shifted up-left by `offset` inside a clipped
/// viewport. The caller owns and clamps the offset. Pointer-down and
/// scroll gate on the viewport (starts stay inside it); motion,
/// release, and keys pass unbounded so active gestures and the
/// focused editor keep working outside.
pub fn place_scrolled<C: 'static, Cv: Canvas + 'static>(
    child: Measured<Placed<C, Cv>>,
    placement: Placement,
    offset: Vec2,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> bool + 'static,
) -> Placed<C, Cv> {
    let rect = placement.rect;
    let mut base = Placed::empty();
    if !placement.clipped_out() {
        base.handler_mut().on_scroll(move |state, event| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && on_scroll(state, event)
        });
    }
    let child_rect = child
        .extent
        .rect_at(Point::new(rect.x0 - offset.x, rect.y0 - offset.y));
    let child_placement =
        measured::child_placement(measured::clipped_placement(placement, rect), child_rect);
    let mut placed = measured::place(child, child_placement);
    let renders = std::mem::take(&mut placed.renders);
    placed.renders.push(Box::new(move |cv: &mut Cv| {
        cv.clip(rect, Affine::IDENTITY, |cv| Placed::<C, Cv>::render(renders, cv))
    }));
    placed.handler = placed.handler.map(|handler| gate_starts(handler, placement));
    base.over(placed)
}

fn gate_starts<C: 'static>(child: Handler<C>, placement: Placement) -> Handler<C> {
    let Handler {
        pointer_down,
        pointer_move,
        pointer_up,
        scroll,
        key,
        ime,
    } = child;
    let mut gated = Handler::new();
    if !placement.clipped_out() {
        gated.on_pointer_down(move |ctx, event: &PointerButtonEvent| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && pointer_down(ctx, event)
        });
        gated.on_scroll(move |ctx, event| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && scroll(ctx, event)
        });
    }
    gated.on_pointer_move(pointer_move);
    gated.on_pointer_up(pointer_up);
    gated.on_key(key);
    gated.on_ime(ime);
    gated
}

pub fn metrics_extent(metrics: TextMetrics) -> Extent {
    Extent {
        width: metrics.width,
        ascent: metrics.ascent,
        descent: metrics.descent,
    }
}
