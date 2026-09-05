//! Progred's placement output: the frame with its pixels still
//! latent. Placement folds every leaf's contribution into one
//! [`Placed`] — hover probes, the composed handler, keyboard
//! geometry, completion offers, floating subtrees, and deferred ink
//! — combined in placement order, so the later contribution is on
//! top: painted last, asked first.

use crate::completion::Offers;
use crate::frame::Hovered;
use crate::hover::Secondary;
use crate::navigate::Descend;
use crate::workspace::Root;
use kurbo::{Affine, Point, Rect, Stroke, Vec2};
use measured::{Extent, Measured, Output};
use peniko::{Brush, Color, ImageData};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::handler::{Handler, HasHandler, ScrollOutcome};
use puri::hover::Claim;
use puri::text::TextMetrics;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent};
use uig::Placement;

pub type Render<Cv> = Box<dyn for<'a> FnOnce(&mut Cv, Ink<'a>)>;

enum ProbeTarget {
    Retains(Hovered),
    Exact(Hovered),
    Dynamic(Box<dyn Fn(Point) -> Option<Hovered>>),
    Occludes,
}

/// One settled hover region. Its real placement can establish hover;
/// ordinary named probes may also retain the same target through their
/// expanded visible rectangle.
pub struct Probe {
    root: Option<Root>,
    placement: Placement,
    target: ProbeTarget,
}

impl Probe {
    pub fn retaining(placement: Placement, target: Hovered) -> Self {
        Self {
            root: None,
            placement,
            target: ProbeTarget::Retains(target),
        }
    }

    pub fn exact(placement: Placement, target: Hovered) -> Self {
        Self {
            root: None,
            placement,
            target: ProbeTarget::Exact(target),
        }
    }

    pub fn occludes(placement: Placement) -> Self {
        Self {
            root: None,
            placement,
            target: ProbeTarget::Occludes,
        }
    }

    pub fn dynamic(
        placement: Placement,
        target_at: impl Fn(Point) -> Option<Hovered> + 'static,
    ) -> Self {
        Self {
            root: None,
            placement,
            target: ProbeTarget::Dynamic(Box::new(target_at)),
        }
    }

    fn extended_rect(&self, reach: f64) -> Option<Rect> {
        if self.placement.clipped_out() {
            return None;
        }
        let rect = self
            .placement
            .visible_rect()
            .inflate(reach, reach)
            .intersect(self.placement.clip_rect);
        (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect)
    }

    fn answer(&self, point: Point, prior: Option<&Hovered>, reach: f64) -> Option<Claim<Hovered>> {
        if self.placement.contains(point) {
            return Some(match &self.target {
                ProbeTarget::Retains(target) | ProbeTarget::Exact(target) => {
                    Claim::Direct(target.clone())
                }
                ProbeTarget::Dynamic(target_at) => {
                    return target_at(point).map(Claim::Direct);
                }
                ProbeTarget::Occludes => Claim::Occludes,
            });
        }
        match &self.target {
            ProbeTarget::Retains(target) if prior == Some(target) => self
                .extended_rect(reach)
                .filter(|rect| rect.contains(point))
                .map(|_| Claim::Extended(target.clone())),
            _ => None,
        }
    }
}

/// The settled target is an explicit dispatch input, never an input to
/// description or placement. Accepted handlers perform their own actions.
pub struct DispatchContext<C> {
    pub descends: std::rc::Rc<[Descend<C>]>,
    pub root: Option<Root>,
    pub hovered: Option<Hovered>,
    outside_view: bool,
}

impl<C> Default for DispatchContext<C> {
    fn default() -> Self {
        Self {
            descends: Default::default(),
            root: None,
            hovered: None,
            outside_view: false,
        }
    }
}

impl<C> DispatchContext<C> {
    pub fn new(root: Option<Root>, hovered: Option<Hovered>) -> Self {
        Self {
            root,
            hovered,
            ..Self::default()
        }
    }

    fn matches(&self, target: &Hovered) -> bool {
        !self.outside_view && self.hovered.as_ref() == Some(target)
    }
}

/// A nested scroll container's settled geometry, retained so
/// selection reveal can update the same view state as pointer scroll.
pub struct ViewRegion {
    pub root: Root,
    pub rect: Rect,
    pub maximum: Vec2,
}

/// What ink may condition on: the frame's RESOLVED hover, decided
/// from this same pass's geometry before any render runs.
#[derive(Clone, Copy)]
pub struct Ink<'a> {
    pub hovered: Option<&'a Hovered>,
    /// The cell-relative location the hover refers to; its other
    /// projections carry the faint secondary mark.
    pub hovered_secondary: Option<&'a Secondary>,
    /// The actual structural source under the pointer, independent of
    /// value-equivalence highlighting.
    pub hovered_trace: Option<&'a crate::hover::SourceTrace>,
    /// Draw each leaf's honest placement rectangle after its own ink.
    pub debug_geometry: bool,
}

pub struct Placed<C, Cv> {
    pub probes: Vec<Probe>,
    /// `None` until something registers: combining empty frames must
    /// not deepen the dispatch chain.
    pub handler: Option<Handler<C, DispatchContext<C>>>,
    pub descends: Vec<Descend<C>>,
    pub view_regions: Vec<ViewRegion>,
    /// A projected control may override how the nearest enclosing
    /// navigation landmark is selected. The landmark consumes this
    /// while placing, so it never leaks into an ancestor.
    pub landmark_select: Option<progred_display::ActionHandler<C>>,
    pub completion: Option<Offers>,
    /// Out-of-flow subtrees gathered during placement and raised over
    /// the completed frame before hover resolution.
    pub floaters: Vec<Box<Placed<C, Cv>>>,
    pub renders: Vec<Render<Cv>>,
}

/// Preserve the destination's ordering while avoiding an allocation
/// and element moves when an empty parent can simply take ownership of
/// its first child's buffer.
fn append<T>(base: &mut Vec<T>, mut above: Vec<T>) {
    if above.is_empty() {
        return;
    }
    if base.is_empty() {
        *base = above;
    } else {
        base.append(&mut above);
    }
}

impl<C: 'static, Cv> Output for Placed<C, Cv> {
    fn empty() -> Self {
        Self {
            probes: Vec::new(),
            handler: None,
            descends: Vec::new(),
            view_regions: Vec::new(),
            landmark_select: None,
            completion: None,
            floaters: Vec::new(),
            renders: Vec::new(),
        }
    }

    fn over(mut self, above: Self) -> Self {
        append(&mut self.probes, above.probes);
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(handler_over(base, above)),
        };
        append(&mut self.descends, above.descends);
        append(&mut self.view_regions, above.view_regions);
        self.landmark_select = above.landmark_select.or(self.landmark_select);
        self.completion = above.completion.or(self.completion);
        append(&mut self.floaters, above.floaters);
        append(&mut self.renders, above.renders);
        self
    }
}

impl<C: 'static, Cv> Placed<C, Cv> {
    pub fn raise_floaters(mut self) -> Self {
        for floater in std::mem::take(&mut self.floaters) {
            self = self.over(floater.raise_floaters());
        }
        self
    }

    fn root_navigation(&mut self, root: &Root) {
        for probe in &mut self.probes {
            probe.root = Some(root.clone());
        }
        for descend in &mut self.descends {
            descend.root = Some(root.clone());
        }
        if let Some(handler) = &mut self.handler {
            let root = root.clone();
            let dispatch = std::mem::replace(&mut handler.pointer_down, Box::new(|_, _, _| false));
            handler.pointer_down = Box::new(move |ctx, event, pointer| {
                let outside = pointer.outside_view;
                pointer.outside_view = pointer.root.as_ref() != Some(&root);
                let handled = dispatch(ctx, event, pointer);
                pointer.outside_view = outside;
                handled
            });
        }
        for floater in &mut self.floaters {
            floater.root_navigation(root);
        }
    }

    /// What the pointer at `point` rests on. A direct answer or
    /// occluder wins immediately in placement order; an extension of
    /// `prior` is remembered only in case every real region is air.
    #[cfg(test)]
    pub fn probe(
        &self,
        point: Point,
        prior: Option<&Hovered>,
        reach: f64,
    ) -> Option<Claim<Hovered>> {
        self.probe_scoped(point, prior, reach)
            .map(|(_, claim)| claim)
    }

    pub fn probe_scoped(
        &self,
        point: Point,
        prior: Option<&Hovered>,
        reach: f64,
    ) -> Option<(Option<Root>, Claim<Hovered>)> {
        let mut retained = None;
        for probe in self.probes.iter().rev() {
            match probe.answer(point, prior, reach) {
                Some(claim @ (Claim::Direct(_) | Claim::Occludes)) => {
                    return Some((probe.root.clone(), claim));
                }
                Some(Claim::Extended(target)) => {
                    retained = Some((probe.root.clone(), Claim::Extended(target)));
                }
                _ => {}
            }
        }
        retained
    }

    pub fn extended_rects(&self, target: &Hovered, reach: f64) -> Vec<Rect> {
        self.probes
            .iter()
            .filter_map(|probe| match &probe.target {
                ProbeTarget::Retains(candidate) if candidate == target => {
                    probe.extended_rect(reach)
                }
                _ => None,
            })
            .collect()
    }

    pub fn handler_mut(&mut self) -> &mut Handler<C, DispatchContext<C>> {
        self.handler.get_or_insert_with(Handler::new)
    }

    /// Run the deferred ink into a canvas, with the resolved hover.
    pub fn render(renders: Vec<Render<Cv>>, canvas: &mut Cv, ink: Ink<'_>) {
        for render in renders {
            render(canvas, ink);
        }
    }
}

impl<C: 'static, Cv> Builder<'_, C, Cv> {
    /// Install the selection transition for the navigation landmark
    /// enclosing this projected control.
    pub fn select_landmark(&mut self, action: progred_display::ActionHandler<C>) {
        self.placed.landmark_select = Some(action);
    }
}

/// Stack `above`'s dispatch over `base`'s: above tries first, declines
/// fall through — placement order as precedence, same as paint.
fn handler_over<C: 'static>(
    mut base: Handler<C, DispatchContext<C>>,
    above: Handler<C, DispatchContext<C>>,
) -> Handler<C, DispatchContext<C>> {
    base.on_pointer_down_with(above.pointer_down);
    base.on_pointer_move(above.pointer_move);
    base.on_pointer_up(above.pointer_up);
    base.on_pointer_cancel(above.pointer_cancel);
    base.on_scroll(above.scroll);
    base.on_key_with(above.key);
    base.on_ime(above.ime);
    base
}

/// The leaf-construction context: today's placement-pass interface,
/// accumulating a [`Placed`] instead of drawing and registering
/// against live machinery. Ink defers; everything else settles here.
pub struct Builder<'a, C: 'static, Cv> {
    placed: &'a mut Placed<C, Cv>,
    visible: bool,
}

impl<'builder, C: 'static, Cv> Builder<'builder, C, Cv> {
    fn new(placed: &'builder mut Placed<C, Cv>, placement: Placement) -> Self {
        Self {
            placed,
            visible: !placement.clipped_out(),
        }
    }

    /// Contribute a named hover region.
    pub fn claim(&mut self, placement: Placement, target: Hovered) {
        if !placement.clipped_out() {
            self.placed.probes.push(Probe::retaining(placement, target));
        }
    }

    /// Contribute a named hover region without the ordinary air-gap
    /// retention. Useful for chrome whose pointer feedback should track
    /// its hit geometry exactly.
    pub fn claim_exact(&mut self, placement: Placement, target: Hovered) {
        if !placement.clipped_out() {
            self.placed.probes.push(Probe::exact(placement, target));
        }
    }

    pub fn claim_dynamic(
        &mut self,
        placement: Placement,
        target_at: impl Fn(Point) -> Option<Hovered> + 'static,
    ) {
        if !placement.clipped_out() {
            self.placed
                .probes
                .push(Probe::dynamic(placement, target_at));
        }
    }

    /// Contribute an unnamed region that blocks targets below it.
    pub fn occlude(&mut self, placement: Placement) {
        if !placement.clipped_out() {
            self.placed.probes.push(Probe::occludes(placement));
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

    /// Contribute ink that reads the resolved hover — the only paint
    /// that may differ under the pointer.
    pub fn ink(&mut self, render: impl for<'ink> FnOnce(&mut Cv, Ink<'ink>) + 'static) {
        if self.visible {
            self.placed.renders.push(Box::new(render));
        }
    }
}

impl<C: 'static, Cv: Canvas + 'static> Canvas for Builder<'_, C, Cv> {
    fn image(&mut self, image: ImageData, transform: Affine) {
        if self.visible {
            self.placed
                .renders
                .push(Box::new(move |cv, _| cv.image(image, transform)));
        }
    }

    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        if self.visible {
            let (shape, brush) = (shape.into(), brush.into());
            self.placed
                .renders
                .push(Box::new(move |cv, _| cv.fill(shape, brush, transform)));
        }
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        if self.visible {
            let (shape, brush) = (shape.into(), brush.into());
            self.placed.renders.push(Box::new(move |cv, _| {
                cv.stroke(shape, style, brush, transform)
            }));
        }
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        if self.visible {
            self.placed
                .renders
                .push(Box::new(move |cv, _| cv.glyph_run(run)));
        }
    }

    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        if self.visible {
            let shape = shape.into();
            let render_start = self.placed.renders.len();
            content(self);
            let renders = self.placed.renders.split_off(render_start);
            self.placed.renders.push(Box::new(move |cv: &mut Cv, ink| {
                cv.clip(shape, transform, |cv| {
                    Placed::<C, Cv>::render(renders, cv, ink)
                })
            }));
        }
    }
}

impl<C: 'static, Cv> HasHandler<C> for Builder<'_, C, Cv> {
    type Input = DispatchContext<C>;

    fn handler(&mut self) -> &mut Handler<C, DispatchContext<C>> {
        self.placed.handler_mut()
    }
}

impl<C: 'static, Cv> Builder<'_, C, Cv> {
    pub fn descends(&mut self) -> &mut Vec<Descend<C>> {
        &mut self.placed.descends
    }

    pub fn completion(&mut self) -> &mut Option<Offers> {
        &mut self.placed.completion
    }
}

fn built_into<C: 'static, Cv: 'static>(
    f: impl FnOnce(&mut Builder<'_, C, Cv>, Placement) + 'static,
) -> impl FnOnce(Placement, &mut Placed<C, Cv>) + 'static {
    move |placement, placed| f(&mut Builder::new(placed, placement), placement)
}

pub fn leaf<C: 'static, Cv: Canvas + 'static>(
    extent: Extent,
    place: impl FnOnce(&mut Builder<'_, C, Cv>, Placement) + 'static,
) -> Measured<Placed<C, Cv>> {
    let place = built_into(place);
    measured::leaf_into(extent, move |placement, placed: &mut Placed<C, Cv>| {
        place(placement, placed);
        if !placement.clipped_out() {
            placed.renders.push(Box::new(move |cv: &mut Cv, ink| {
                if ink.debug_geometry {
                    cv.stroke(
                        placement.rect,
                        Stroke::new(0.75),
                        Color::new([0.0, 0.65, 1.0, 0.36]),
                        Affine::IDENTITY,
                    );
                }
            }));
        }
    })
}

pub fn before<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    place_before: impl FnOnce(&mut Builder<'_, C, Cv>, Placement) + 'static,
) -> Measured<Placed<C, Cv>> {
    measured::before_into(child, built_into(place_before))
}

/// Add `content` as an out-of-flow subtree without contributing its
/// extent to `base`. The completed frame raises all such subtrees
/// together.
pub fn floating<C: 'static, Cv: 'static>(
    base: Measured<Placed<C, Cv>>,
    content: Measured<Placed<C, Cv>>,
    place: impl FnOnce(Placement, Extent) -> Option<Placement> + 'static,
) -> Measured<Placed<C, Cv>> {
    let extent = content.extent;
    measured::around(base, move |placement, base| {
        let mut placed = base.place();
        if let Some(placement) = place(placement, extent) {
            placed
                .floaters
                .push(Box::new(measured::place(content, placement)));
        }
        placed
    })
}

/// Float `content` next to `trigger`. Placement's clip rectangle
/// supplies the popup bounds.
pub fn popover<C: 'static, Cv: 'static>(
    trigger: Measured<Placed<C, Cv>>,
    content: Measured<Placed<C, Cv>>,
    gap: f64,
) -> Measured<Placed<C, Cv>> {
    floating(trigger, content, move |placement, extent| {
        (!placement.clipped_out()).then(|| {
            let bounds = placement.clip_rect;
            let below = placement.rect.y1 + gap;
            let above = placement.rect.y0 - gap - extent.height();
            let y = if below + extent.height() <= bounds.y1 || above < bounds.y0 {
                below.min((bounds.y1 - extent.height()).max(bounds.y0))
            } else {
                above
            };
            let x = placement
                .rect
                .x0
                .clamp(bounds.x0, (bounds.x1 - extent.width).max(bounds.x0));
            Placement::new(extent.rect_at(Point::new(x, y)), bounds)
        })
    })
}

pub fn decorate<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    draw: impl FnOnce(&mut Builder<'_, C, Cv>, Rect) + 'static,
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

/// A scroll viewport over `child`: placed at the viewport rect, it
/// shifts the child up-left by `offset` inside a clip. The caller
/// owns and clamps the offset. Pointer-down and scroll gate on the
/// viewport (starts stay inside it); motion, release, and keys pass
/// unbounded so active gestures and the focused editor keep working
/// outside.
/// A scroll viewport tagged as an editor view. The settled region is frame
/// output, not retained widget state; input and keyboard reveal use
/// it to update the same caller-owned view.
pub fn scrolled_at<C: 'static, Cv: Canvas + 'static>(
    child: Measured<Placed<C, Cv>>,
    offset: Vec2,
    owner: Option<(Root, f64)>,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<Placed<C, Cv>> {
    measured::around(child, move |placement, inner| {
        let rect = placement.rect;
        let mut base = Placed::empty();
        if !placement.clipped_out() {
            base.handler_mut().on_scroll(move |state, event| {
                if placement.contains(Point::new(event.state.position.x, event.state.position.y)) {
                    on_scroll(state, event)
                } else {
                    ScrollOutcome::pass(event)
                }
            });
        }
        let extent = inner.extent();
        if let Some((root, scale)) = owner {
            base.view_regions.push(ViewRegion {
                root,
                rect,
                maximum: Vec2::new(
                    ((extent.width - rect.width()) / scale).max(0.0),
                    ((extent.height() - rect.height()) / scale).max(0.0),
                ),
            });
        }
        let child_rect = extent.rect_at(Point::new(rect.x0 - offset.x, rect.y0 - offset.y));
        let child_placement =
            measured::child_placement(measured::clipped_placement(placement, rect), child_rect);
        let mut placed = inner.place_at(child_placement);
        let renders = std::mem::take(&mut placed.renders);
        placed.renders.push(Box::new(move |cv: &mut Cv, ink| {
            cv.clip(rect, Affine::IDENTITY, |cv| {
                Placed::<C, Cv>::render(renders, cv, ink)
            })
        }));
        placed.handler = placed
            .handler
            .map(|handler| gate_starts(handler, placement));
        base.over(placed)
    })
}

/// Associate every navigation occurrence produced by `child` with
/// one editor view. This happens after projection and placement, so
/// the projection language remains unaware of panes.
pub fn in_view<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    root: Root,
) -> Measured<Placed<C, Cv>> {
    measured::around(child, move |placement, inner| {
        let mut placed = inner.place_at(placement);
        placed.root_navigation(&root);
        placed
    })
}

fn gate_starts<C: 'static>(
    child: Handler<C, DispatchContext<C>>,
    placement: Placement,
) -> Handler<C, DispatchContext<C>> {
    let Handler {
        pointer_down,
        pointer_move,
        pointer_up,
        pointer_cancel,
        scroll,
        key,
        ime,
    } = child;
    let mut gated = Handler::new();
    if !placement.clipped_out() {
        gated.on_pointer_down_with(move |ctx, event: &PointerButtonEvent, pointer| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && pointer_down(ctx, event, pointer)
        });
        gated.on_scroll(move |ctx, event| {
            if placement.contains(Point::new(event.state.position.x, event.state.position.y)) {
                scroll(ctx, event)
            } else {
                ScrollOutcome::pass(event)
            }
        });
    }
    gated.on_pointer_move(pointer_move);
    gated.on_pointer_up(pointer_up);
    gated.on_pointer_cancel(pointer_cancel);
    gated.on_key_with(key);
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

    impl Canvas for TestCanvas {
        fn image(&mut self, image: ImageData, transform: Affine) {
            self.0.image(image, transform);
        }

        fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
            self.0.fill(shape, brush, transform);
        }

        fn stroke(
            &mut self,
            shape: impl Into<Shape>,
            style: Stroke,
            brush: impl Into<Brush>,
            transform: Affine,
        ) {
            self.0.stroke(shape, style, brush, transform);
        }

        fn glyph_run(&mut self, run: GlyphRun) {
            self.0.glyph_run(run);
        }

        fn clip(
            &mut self,
            shape: impl Into<Shape>,
            transform: Affine,
            content: impl FnOnce(&mut Self),
        ) {
            let shape = shape.into();
            let mut inner = TestCanvas(DrawList::new());
            content(&mut inner);
            self.0.0.push(DrawCmd::Clip {
                shape,
                transform,
                children: inner.0.0,
            });
        }
    }

    fn no_ink<'a>() -> Ink<'a> {
        Ink {
            hovered: None,
            hovered_secondary: None,
            hovered_trace: None,
            debug_geometry: false,
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
        let mut placed: Placed<Vec<&'static str>, TestCanvas> = Placed::empty();
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
        let mut pointer = DispatchContext::new(None, Some(target));
        let mut log = Vec::new();
        assert!(placed.handler.unwrap().dispatch_pointer_down_with(
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
        let lower = leaf(
            extent,
            |p: &mut Builder<'_, Vec<&'static str>, TestCanvas>, _| {
                p.handler().on_pointer_down(|log, _| {
                    log.push("lower");
                    true
                });
            },
        );
        let upper = leaf(
            extent,
            |p: &mut Builder<'_, Vec<&'static str>, TestCanvas>, _| {
                p.handler().on_pointer_down(|log, _| {
                    log.push("upper");
                    false
                });
            },
        );
        let placed = measured::place_top_left(measured::layers(vec![lower, upper]), Point::ZERO);
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
            |p: &mut Builder<'_, (), TestCanvas>, placement| {
                p.fill(placement.rect, Color::BLACK, Affine::IDENTITY);
            },
        );
        let content = leaf(
            Extent {
                width: 20.0,
                ascent: 0.0,
                descent: 20.0,
            },
            |p: &mut Builder<'_, (), TestCanvas>, placement| {
                p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
            },
        );
        let popup = popover(trigger, content, 2.0);

        assert_eq!(popup.extent.width, 10.0);
        assert_eq!(popup.extent.height(), 10.0);

        let placed = measured::place(
            popup,
            Placement::new(
                Rect::new(5.0, 5.0, 15.0, 15.0),
                Rect::new(0.0, 0.0, 100.0, 100.0),
            ),
        );
        assert_eq!(placed.floaters.len(), 1);

        let mut canvas = TestCanvas(DrawList::new());
        Placed::<(), TestCanvas>::render(placed.raise_floaters().renders, &mut canvas, no_ink());
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
            |p: &mut Builder<'_, (), TestCanvas>, placement| {
                p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
            },
        );
        let rect = Rect::new(3.0, 5.0, 15.0, 15.0);
        let placed = measured::place(child, Placement::root(rect));
        let mut canvas = TestCanvas(DrawList::new());
        Placed::<(), TestCanvas>::render(
            placed.renders,
            &mut canvas,
            Ink {
                hovered: None,
                hovered_secondary: None,
                hovered_trace: None,
                debug_geometry: true,
            },
        );

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
            move |p: &mut Builder<'_, (), TestCanvas>, placement| {
                p.claim(placement, target.clone());
                p.activate(target.clone(), |_| true);
                p.pick(target, |_| true);
                p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
                p.handler().on_pointer_move(|_, _| true);
            },
        );
        let placed = measured::place(
            child,
            Placement::new(
                Rect::new(20.0, 20.0, 30.0, 30.0),
                Rect::new(0.0, 0.0, 10.0, 10.0),
            ),
        );

        assert!(placed.probes.is_empty());
        assert!(placed.renders.is_empty());
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
            |p: &mut Builder<'_, (), TestCanvas>, placement| {
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
        let placed = measured::place(
            scrolled_at(probe, Vec2::new(5.0, 40.0), None, |_, event| {
                ScrollOutcome::pass(event)
            }),
            Placement::new(
                Rect::new(10.0, 20.0, 90.0, 70.0),
                Rect::new(0.0, 0.0, 100.0, 100.0),
            ),
        );
        let mut canvas = TestCanvas(DrawList::new());
        Placed::<(), TestCanvas>::render(placed.renders, &mut canvas, no_ink());
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
            |p: &mut Builder<'_, Vec<&'static str>, TestCanvas>, _| {
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
        let placed = measured::place(
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
        );
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
        let mut placed: Placed<Vec<&'static str>, TestCanvas> = Placed::empty();
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
        let handler = placed.handler.unwrap();
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
                move |p: &mut Builder<'_, Vec<&'static str>, TestCanvas>, placement| {
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
                leaf(
                    extent,
                    |_: &mut Builder<'_, Vec<&'static str>, TestCanvas>, _| {},
                ),
                row("popup"),
                move |_, _| Some(Placement::new(popup_rect, bounds)),
            ),
            owner.clone(),
        );
        let owner_view = measured::place(
            owner_view,
            Placement::new(extent.rect_at(Point::ZERO), bounds),
        );
        let covered_view = measured::place(
            in_view(row("covered pane"), covered),
            Placement::new(popup_rect, bounds),
        );
        let placed = owner_view.over(covered_view).raise_floaters();
        let point = popup_rect.center();
        let (root, Claim::Direct(hit)) = placed.probe_scoped(point, None, 0.0).unwrap() else {
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
    fn semantic_actions_obey_scroll_clips_even_when_the_target_matches() {
        let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
        let claimed = target.clone();
        let child = leaf(
            Extent {
                width: 30.0,
                ascent: 0.0,
                descent: 30.0,
            },
            move |p: &mut Builder<'_, usize, TestCanvas>, placement| {
                p.claim(placement, claimed.clone());
                p.activate(claimed, |count| {
                    *count += 1;
                    true
                });
            },
        );
        let placed = measured::place(
            scrolled_at(child, Vec2::ZERO, None, |_, event| {
                ScrollOutcome::pass(event)
            }),
            Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
        );
        let mut pointer = DispatchContext::new(None, Some(target));
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
            let mut placed: Placed<usize, TestCanvas> = Placed::empty();
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
            let hovered = match placed.probe(Point::new(5.0, 5.0), None, 0.0) {
                Some(Claim::Direct(target)) => target,
                Some(Claim::Occludes) => Hovered::Blocked,
                _ => panic!("hit"),
            };
            let mut pointer = DispatchContext::new(None, Some(hovered));
            let mut event = down_at(5.0, 5.0);
            event.state.modifiers =
                ui_events::keyboard::Modifiers::META | ui_events::keyboard::Modifiers::CONTROL;
            let mut count = 0;
            assert!(placed.handler.unwrap().dispatch_pointer_down_with(
                &mut count,
                &event,
                &mut pointer
            ));
            assert_eq!(count, usize::from(!covered));
        }
    }
}
