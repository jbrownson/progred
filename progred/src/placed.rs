//! Progred's placement output: the frame with its pixels still
//! latent. Placement folds every leaf's contribution into one
//! [`Placed`] — hover probes, the composed handler, keyboard
//! geometry, the popup stash, and deferred ink — combined in
//! placement order, so the later contribution is on top: painted
//! last, asked first.

use crate::completion::{HasPopup, Popup};
use crate::frame::Hovered;
use crate::hover::Secondary;
use crate::navigate::{Descend, HasDescends};
use crate::workspace::Root;
use measured::{Extent, Measured, Output};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::handler::{Handler, HasHandler, ScrollOutcome};
use puri::hover::Claim;
use puri::text::TextMetrics;
use uig::Placement;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent};
use kurbo::{Affine, Point, Rect, Stroke, Vec2};
use peniko::{Brush, Color};

pub type Render<Cv> = Box<dyn for<'a> FnOnce(&mut Cv, Ink<'a>)>;
pub type EditorAction<C> = Box<dyn Fn(&mut C) -> bool>;

enum ProbeTarget {
    Retains(Hovered),
    Exact(Hovered),
    Occludes,
}

/// One settled hover region. Its real placement can establish hover;
/// ordinary named probes may also retain the same target through their
/// expanded visible rectangle.
pub struct Probe {
    placement: Placement,
    target: ProbeTarget,
}

impl Probe {
    pub fn retaining(placement: Placement, target: Hovered) -> Self {
        Self {
            placement,
            target: ProbeTarget::Retains(target),
        }
    }

    pub fn exact(placement: Placement, target: Hovered) -> Self {
        Self {
            placement,
            target: ProbeTarget::Exact(target),
        }
    }

    pub fn occludes(placement: Placement) -> Self {
        Self {
            placement,
            target: ProbeTarget::Occludes,
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

    fn answer(
        &self,
        point: Point,
        prior: Option<&Hovered>,
        reach: f64,
    ) -> Option<Claim<Hovered>> {
        if self.placement.contains(point) {
            return Some(match &self.target {
                ProbeTarget::Retains(target) | ProbeTarget::Exact(target) => {
                    Claim::Direct(target.clone())
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

/// A coordinate-free editor action addressed to the same identity
/// hover resolves. The topmost registration for a target gets the
/// first chance to handle it; false falls through to registrations
/// beneath it.
pub struct TargetAction<C> {
    root: Option<Root>,
    target: Hovered,
    action: EditorAction<C>,
}

/// A nested scroll container's settled geometry, retained so
/// selection reveal can update the same view state as pointer scroll.
pub struct ViewRegion {
    pub root: Root,
    pub rect: Rect,
    pub maximum: Vec2,
}

pub fn dispatch_target<C>(
    actions: &[TargetAction<C>],
    ctx: &mut C,
    root: Option<&Root>,
    target: &Hovered,
) -> bool {
    actions
        .iter()
        .rev()
        .any(|candidate| {
            candidate.root.as_ref().is_none_or(|candidate| Some(candidate) == root)
                && candidate.target == *target
                && (candidate.action)(ctx)
        })
}

/// What ink may condition on: the frame's RESOLVED hover, decided
/// from this same pass's geometry before any render runs.
#[derive(Clone, Copy)]
pub struct Ink<'a> {
    pub hovered: Option<&'a Hovered>,
    /// The cell-relative location the hover refers to; its other
    /// projections carry the faint secondary mark.
    pub hovered_secondary: Option<&'a Secondary>,
    /// Draw each leaf's honest placement rectangle after its own ink.
    pub debug_geometry: bool,
}

pub struct Placed<C, Cv> {
    pub probes: Vec<Probe>,
    pub activations: Vec<TargetAction<C>>,
    pub picks: Vec<TargetAction<C>>,
    /// `None` until something registers: combining empty frames must
    /// not deepen the dispatch chain.
    pub handler: Option<Handler<C>>,
    pub descends: Vec<Descend<C>>,
    pub view_regions: Vec<ViewRegion>,
    /// A projected control may override how the nearest enclosing
    /// navigation landmark is selected. The landmark consumes this
    /// while placing, so it never leaks into an ancestor.
    pub landmark_select: Option<progred_display::ActionHandler<C>>,
    pub popup: Option<Popup>,
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
            activations: Vec::new(),
            picks: Vec::new(),
            handler: None,
            descends: Vec::new(),
            view_regions: Vec::new(),
            landmark_select: None,
            popup: None,
            renders: Vec::new(),
        }
    }

    fn over(mut self, above: Self) -> Self {
        append(&mut self.probes, above.probes);
        append(&mut self.activations, above.activations);
        append(&mut self.picks, above.picks);
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(handler_over(base, above)),
        };
        append(&mut self.descends, above.descends);
        append(&mut self.view_regions, above.view_regions);
        self.landmark_select = above.landmark_select.or(self.landmark_select);
        self.popup = above.popup.or(self.popup);
        append(&mut self.renders, above.renders);
        self
    }
}

impl<C: 'static, Cv> Placed<C, Cv> {
    /// What the pointer at `point` rests on. A direct answer or
    /// occluder wins immediately in placement order; an extension of
    /// `prior` is remembered only in case every real region is air.
    pub fn probe(
        &self,
        point: Point,
        prior: Option<&Hovered>,
        reach: f64,
    ) -> Option<Claim<Hovered>> {
        let mut retained = None;
        for probe in self.probes.iter().rev() {
            match probe.answer(point, prior, reach) {
                direct @ Some(Claim::Direct(_) | Claim::Occludes) => return direct,
                Some(Claim::Extended(target)) => {
                    retained = Some(Claim::Extended(target));
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

    pub fn handler_mut(&mut self) -> &mut Handler<C> {
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
    fn chain_scroll<C>(
        base: Box<dyn Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome>,
        above: Box<dyn Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome>,
    ) -> Box<dyn Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome>
    where
        C: 'static,
    {
        Box::new(move |ctx, event| {
            let outcome = above(ctx, event);
            match outcome.event(event) {
                Some(event) => outcome.followed_by(base(ctx, &event)),
                None => outcome,
            }
        })
    }
    Handler {
        pointer_down: chain(base.pointer_down, above.pointer_down),
        pointer_move: chain(base.pointer_move, above.pointer_move),
        pointer_up: chain(base.pointer_up, above.pointer_up),
        pointer_cancel: chain(base.pointer_cancel, above.pointer_cancel),
        scroll: chain_scroll(base.scroll, above.scroll),
        key: chain(base.key, above.key),
        ime: chain(base.ime, above.ime),
    }
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
            self.placed
                .probes
                .push(Probe::retaining(placement, target));
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

    /// Contribute an unnamed region that blocks targets below it.
    pub fn occlude(&mut self, placement: Placement) {
        if !placement.clipped_out() {
            self.placed.probes.push(Probe::occludes(placement));
        }
    }

    pub fn activate(
        &mut self,
        target: Hovered,
        action: impl Fn(&mut C) -> bool + 'static,
    ) {
        if self.visible {
            self.placed.activations.push(TargetAction {
                root: None,
                target,
                action: Box::new(action),
            });
        }
    }

    pub fn pick(
        &mut self,
        target: Hovered,
        action: impl Fn(&mut C) -> bool + 'static,
    ) {
        if self.visible {
            self.placed.picks.push(TargetAction {
                root: None,
                target,
                action: Box::new(action),
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
            self.placed
                .renders
                .push(Box::new(move |cv, _| cv.stroke(shape, style, brush, transform)));
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
    fn handler(&mut self) -> &mut Handler<C> {
        self.placed.handler_mut()
    }
}

impl<C: 'static, Cv> HasDescends<C> for Builder<'_, C, Cv> {
    fn descends(&mut self) -> &mut Vec<Descend<C>> {
        &mut self.placed.descends
    }
}

impl<C: 'static, Cv> HasPopup for Builder<'_, C, Cv> {
    fn popup(&mut self) -> &mut Option<Popup> {
        &mut self.placed.popup
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

pub fn after<C: 'static, Cv: 'static>(
    child: Measured<Placed<C, Cv>>,
    place_after: impl FnOnce(&mut Builder<'_, C, Cv>, Placement) + 'static,
) -> Measured<Placed<C, Cv>> {
    measured::after_into(child, built_into(place_after))
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
        let child_rect = extent
            .rect_at(Point::new(rect.x0 - offset.x, rect.y0 - offset.y));
        let child_placement =
            measured::child_placement(measured::clipped_placement(placement, rect), child_rect);
        let mut placed = inner.place_at(child_placement);
        let renders = std::mem::take(&mut placed.renders);
        placed.renders.push(Box::new(move |cv: &mut Cv, ink| {
            cv.clip(rect, Affine::IDENTITY, |cv| {
                Placed::<C, Cv>::render(renders, cv, ink)
            })
        }));
        placed.handler = placed.handler.map(|handler| gate_starts(handler, placement));
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
        for descend in &mut placed.descends {
            descend.root = Some(root.clone());
        }
        for action in placed
            .activations
            .iter_mut()
            .chain(&mut placed.picks)
        {
            action.root = Some(root.clone());
        }
        placed
    })
}

fn gate_starts<C: 'static>(child: Handler<C>, placement: Placement) -> Handler<C> {
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
        gated.on_pointer_down(move |ctx, event: &PointerButtonEvent| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && pointer_down(ctx, event)
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

#[cfg(test)]
mod tests {
    use super::*;
    use puri::draw::{DrawCmd, DrawList};
    use ui_events::ScrollDelta;
    use ui_events::pointer::{
        PointerButton, PointerId, PointerInfo, PointerState, PointerType, PointerUpdate,
    };
    use peniko::Color;

    struct TestCanvas(DrawList);

    impl Canvas for TestCanvas {
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
    fn target_actions_follow_resolved_hover_and_visual_precedence() {
        let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
        let other = Hovered::Tree(crate::hover::Hover::Toggle(std::rc::Rc::from([])));
        let actions = vec![
            TargetAction {
                root: None,
                target: target.clone(),
                action: Box::new(|log: &mut Vec<&'static str>| {
                    log.push("lower");
                    true
                }),
            },
            TargetAction {
                root: None,
                target: other,
                action: Box::new(|log: &mut Vec<&'static str>| {
                    log.push("other");
                    true
                }),
            },
            TargetAction {
                root: None,
                target: target.clone(),
                action: Box::new(|log: &mut Vec<&'static str>| {
                    log.push("upper");
                    false
                }),
            },
        ];
        let mut log = Vec::new();

        assert!(dispatch_target(&actions, &mut log, None, &target));
        assert_eq!(log, ["upper", "lower"]);
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
        assert!(placed.activations.is_empty());
        assert!(placed.picks.is_empty());
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
            scrolled_at(child, Vec2::ZERO, None, |log: &mut Vec<&'static str>, event| {
                log.push("scroll");
                ScrollOutcome::consume(event)
            }),
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
}
