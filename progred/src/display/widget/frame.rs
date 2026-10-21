//! Placement runs hover probes and returns continuations for the resolved hover.
use super::{
    container,
    navigation::{Collector, Landmark, Select},
    offers::Offers,
    source::{Secondary, SourceTrace},
    view::Root,
};
use kurbo::{Affine, Point, Rect, Vec2};
use measured::Measured;
use puri::draw::{Canvas, CanvasSink};
use puri::frame::{AfterHover, Render};
use puri::handler::{Handler, HasHandler};
use puri::hover::{Claim, Probe};
use puri::interact::is_primary_contact;
use std::rc::Rc;
use ui_events::pointer::PointerButtonEvent;
use uig::Placement;

pub struct HoverInput<'a, H> {
    pub command_modifier: puri::keyboard::CommandModifier,
    /// The command modifier is held, so a press picks or drags rather than
    /// edits.
    pub picking: bool,
    pub pointer: Option<Point>,
    pub prior: Option<&'a H>,
    pub reach_px: f64,
}

impl<H> Copy for HoverInput<'_, H> {}
impl<H> Clone for HoverInput<'_, H> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Transient access to caller-owned output during a widget's hover computation.
/// The input is not retained by the resulting paint or event continuations.
pub struct HoverContext<'a, C, H> {
    pub input: HoverInput<'a, H>,
    pub(super) output: &'a mut HoverOutput<C, H>,
    root: Option<Root>,
    navigation: &'a mut Collector,
}

impl<'a, C, H> HoverContext<'a, C, H> {
    pub(crate) fn new(
        input: HoverInput<'a, H>,
        output: &'a mut HoverOutput<C, H>,
        navigation: &'a mut Collector,
    ) -> Self {
        Self {
            input,
            output,
            root: None,
            navigation,
        }
    }
}

impl<C: 'static, H: 'static> HoverContext<'_, C, H> {
    pub fn after_hover(
        &mut self,
        next: impl FnOnce(Rc<Attribution<H>>, &mut Effects<C, H>) + 'static,
    ) {
        self.output.after_hover.push(next);
    }

    pub fn render(&mut self, render: impl FnOnce(&mut dyn CanvasSink, &Attribution<H>) + 'static) {
        self.after_hover(move |hover, output| {
            output
                .renders
                .push(Box::new(move |canvas| render(canvas, &hover)));
        });
    }

    pub fn with_clip(
        &mut self,
        shape: puri::draw::Shape,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let start = self.output.after_hover.len();
        content(self);
        let after = self.output.after_hover.split_off(start);
        self.after_hover(move |hover, output| {
            let start = output.renders.len();
            after.bind(hover, output);
            let renders = output.renders.split_off(start);
            output.renders.push(Box::new(move |canvas| {
                canvas.clip(shape, transform, |canvas| {
                    puri::frame::render(renders, canvas)
                })
            }));
        });
    }

    pub fn on_arrival(&mut self, path: crate::occurrence::Trail, select: Select<C>) {
        self.output.landmark_select = Some((path, select));
    }

    /// Place a line editor: its input handlers, the composition area the
    /// platform needs while it is focused, and its paint. Every editable
    /// line goes through here so none can omit the composition area.
    pub fn line_edit(
        &mut self,
        edit: puri::edit::LineEdit,
        placement: Placement,
        with: impl Fn(&mut C, &puri::edit::EditOperation<'_>) -> bool + Clone + 'static,
    ) {
        let command_modifier = self.input.command_modifier;
        edit.install(self, placement, command_modifier, with);
        if let Some(area) = edit.input_area(placement) {
            self.output.input_area = Some(area);
        }
        self.render(move |canvas, _| edit.draw(canvas, placement));
    }

    pub fn navigation_stop(&mut self, path: crate::occurrence::Trail, selected: bool) {
        self.navigation.stop(path, selected);
    }

    /// Marks this occurrence as a place to fill in, which Tab visits.
    pub fn tab_stop(&mut self, path: crate::occurrence::Trail) {
        self.output.tab_stop = Some(path);
    }

    pub fn completion(&mut self) -> &mut Option<Offers<C>> {
        &mut self.output.completion
    }

    pub fn view_region(&mut self, region: ViewRegion) {
        self.output.view_regions.push(region);
    }

    pub fn scroll_probe(&mut self, probe: super::scroll::Probe) {
        self.output.scroll_probes.push(probe);
    }
}
impl<C: 'static, H: Clone + PartialEq + 'static> HoverContext<'_, C, H> {
    /// Show `cursor` while `target`, in this view, is hovered.
    pub fn cursor(&mut self, target: H, cursor: Cursor) {
        self.output
            .hover_geometry
            .cursors
            .push((self.root.clone(), target, cursor));
    }

    pub fn claim(&mut self, probe: Probe<H>) {
        self.output.claim = claim_over(
            self.output.claim.take(),
            self.input
                .pointer
                .and_then(|point| probe.answer(point, self.input.prior, self.input.reach_px))
                .map(|claim| (self.root.clone(), claim)),
        );
        self.output
            .hover_geometry
            .probes
            .push((self.root.clone(), probe));
    }

    /// Blocks the targets below `placement`, and the presses there.
    pub fn occlude(&mut self, placement: Placement) {
        if !placement.clipped_out() {
            self.claim(Probe::occludes(placement));
            self.handler().on_pointer_down(move |_, event| {
                placement.contains(Point::new(event.state.position.x, event.state.position.y))
            });
        }
    }

    /// A primary press over `target`, as this frame's hover settled, runs
    /// `activate`, or `pick` while the command key is held. Most callers take
    /// one of the two and pass `|_, _| false` for the other.
    pub fn on_press(
        &mut self,
        target: H,
        activate: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
        pick: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
    ) {
        let command = self.input.command_modifier;
        self.handler()
            .on_pointer_down_with(move |world, event, input| {
                is_primary_contact(event)
                    && input.hovered() == Some(&target)
                    && if command.pressed(&event.state.modifiers) {
                        pick(world, event)
                    } else {
                        activate(world, event)
                    }
            });
    }
}
impl<C: 'static, H: 'static> HasHandler<C> for HoverContext<'_, C, H> {
    type Input = DispatchContext<C, H>;
    fn handler(&mut self) -> &mut Handler<C, Self::Input> {
        self.output.handler_mut()
    }
}

/// A running placement/hover pass. Ordinary leaves execute immediately;
/// only floating placements are deferred.
pub struct HoverPass<C, H> {
    command_modifier: puri::keyboard::CommandModifier,
    picking: bool,
    pointer: Option<Point>,
    prior: Option<H>,
    reach_px: f64,
    root: Option<Root>,
    output: HoverOutput<C, H>,
    floaters: Vec<Box<dyn FnOnce(&mut Self)>>,
    /// Where floating placements may go: the whole frame, not the view or
    /// clip their anchor is in.
    bounds: Rect,
    pub(super) navigation: Collector,
}

impl<C: 'static, H: 'static> HoverPass<C, H> {
    /// A pass over a frame within `bounds`.
    pub fn new(input: &HoverInput<'_, H>, bounds: Rect) -> Self
    where
        H: Clone,
    {
        Self {
            command_modifier: input.command_modifier,
            picking: input.picking,
            pointer: input.pointer,
            prior: input.prior.cloned(),
            reach_px: input.reach_px,
            root: None,
            output: HoverOutput::default(),
            floaters: Vec::new(),
            bounds,
            navigation: Collector::default(),
        }
    }

    /// Where floating placements may go.
    pub(crate) fn bounds(&self) -> Rect {
        self.bounds
    }

    pub(crate) fn visit(&mut self, step: impl FnOnce(&mut HoverContext<'_, C, H>)) {
        let mut context = HoverContext::new(
            HoverInput {
                command_modifier: self.command_modifier,
                picking: self.picking,
                pointer: self.pointer,
                prior: self.prior.as_ref(),
                reach_px: self.reach_px,
            },
            &mut self.output,
            &mut self.navigation,
        );
        context.root = self.root.clone();
        step(&mut context);
        if let Some(handler) = self.output.handler.take() {
            self.output.after_hover.push(move |_, output| {
                output.handler().stack(handler);
            });
        }
    }

    pub fn scope(
        &mut self,
        content: impl FnOnce(&mut Self),
        map: impl FnOnce(HoverOutput<C, H>) -> HoverOutput<C, H>,
    ) {
        let base = std::mem::take(&mut self.output);
        content(self);
        let child = std::mem::replace(&mut self.output, base);
        self.output = std::mem::take(&mut self.output).over(map(child));
    }

    pub fn in_view(&mut self, root: Root, content: impl FnOnce(&mut Self)) {
        let parent = self.root.replace(root.clone());
        let navigation = std::mem::take(&mut self.navigation);
        self.scope(
            |pass| {
                content(pass);
                if let Some(handlers) =
                    std::mem::take(&mut pass.navigation).finish(Some(root.clone()))
                {
                    pass.visit(|output| {
                        output.handler().stack(handlers);
                    });
                }
            },
            |mut output| {
                output.attribute_view(&root);
                output
            },
        );
        self.navigation = navigation;
        self.root = parent;
    }

    /// Place `content` clipped to `placement`, its starts and scrolls gated there.
    pub(crate) fn clipped(&mut self, placement: Placement, content: impl FnOnce(&mut Self)) {
        self.scope(content, |output| output.clipped(placement));
    }

    /// Place `above` once everything else is placed, over it, in this view.
    pub(crate) fn float(&mut self, above: impl FnOnce(&mut Self) + 'static) {
        let root = self.root.clone();
        self.floaters.push(Box::new(move |pass| match root {
            Some(root) => pass.in_view(root, above),
            None => above(pass),
        }));
    }

    fn run_floaters(&mut self) {
        for floater in std::mem::take(&mut self.floaters) {
            floater(self);
            self.run_floaters();
        }
    }

    pub fn finish(mut self) -> HoverOutput<C, H> {
        self.run_floaters();
        if let Some(handlers) = std::mem::take(&mut self.navigation).finish(self.root.clone()) {
            self.output.handler_mut().stack(handlers);
        }
        self.output
    }
}
impl<C: 'static, H: 'static> measured::ObserveLayout for HoverPass<C, H> {
    fn layout(&mut self, composition: measured::Composition, children: impl FnOnce(&mut Self)) {
        self.navigation.begin(composition);
        children(self);
        self.navigation.end();
    }

    fn layout_child(&mut self, child: impl FnOnce(&mut Self)) {
        self.navigation
            .begin(measured::Composition::Row(measured::RowAlignment::Baseline));
        child(self);
        self.navigation.end();
    }
}

pub fn place<C: 'static, H: Clone + 'static>(
    layout: Measured<HoverPass<C, H>>,
    placement: Placement,
    input: &HoverInput<'_, H>,
) -> HoverOutput<C, H> {
    let mut pass = HoverPass::new(input, placement.clip_rect);
    measured::place_into(layout, placement, &mut pass);
    pass.finish()
}

fn claim_over<H>(
    base: Option<(Option<Root>, Claim<H>)>,
    above: Option<(Option<Root>, Claim<H>)>,
) -> Option<(Option<Root>, Claim<H>)> {
    if above
        .as_ref()
        .is_some_and(|(_, claim)| claim.supersedes(base.as_ref().map(|(_, claim)| claim)))
    {
        above
    } else {
        base
    }
}

/// The installed frame's hit tests, in painting order, and the cursors their
/// targets ask for, with their owning views.
pub struct HoverGeometry<H> {
    probes: Vec<(Option<Root>, Probe<H>)>,
    cursors: Vec<(Option<Root>, H, Cursor)>,
}

impl<H> Default for HoverGeometry<H> {
    fn default() -> Self {
        Self {
            probes: Vec::new(),
            cursors: Vec::new(),
        }
    }
}

/// How the pointer looks over a target that asks; the shell draws the
/// platform's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    /// Across a divider between columns.
    ColumnResize,
    /// Across a divider between stacked panes.
    RowResize,
    /// Over a value that drags left and right.
    HorizontalDrag,
}

impl<H: Clone + PartialEq> HoverGeometry<H> {
    pub fn probe(
        &self,
        pointer: Option<Point>,
        prior: Option<&H>,
        reach_px: f64,
    ) -> Option<(Option<Root>, Claim<H>)> {
        pointer.and_then(|point| {
            self.probes.iter().fold(None, |below, (root, probe)| {
                claim_over(
                    below,
                    probe
                        .answer(point, prior, reach_px)
                        .map(|claim| (root.clone(), claim)),
                )
            })
        })
    }

    /// The cursor `hovered`, in the view `root`, asked for.
    pub fn cursor(&self, root: Option<&Root>, hovered: &H) -> Option<Cursor> {
        self.cursors
            .iter()
            .find(|(owner, target, _)| owner.as_ref() == root && target == hovered)
            .map(|(_, _, cursor)| *cursor)
    }
}

/// The settled target is an explicit dispatch input, never an input to
/// description or placement. Accepted handlers perform their own actions.
pub struct DispatchContext<C, Hover> {
    /// The frame's whole handler, so a handler can lower its input into
    /// another event that every handler sees in the usual order.
    pub handler: Rc<Handler<C, DispatchContext<C, Hover>>>,
    pub landmarks: std::rc::Rc<[Landmark<C>]>,
    pub view_regions: std::rc::Rc<[ViewRegion]>,
    pub root: Option<Root>,
    pub hovered: Option<Hover>,
    pub outside_view: bool,
    /// The pointer while it's over the window, as of this dispatch.
    pub pointer: Option<Point>,
    /// The modifier keys held as of this dispatch.
    pub modifiers: ui_events::keyboard::Modifiers,
    /// Whether a button is down, as of this dispatch.
    pub pressed: bool,
}

impl<C, Hover> Default for DispatchContext<C, Hover> {
    fn default() -> Self {
        Self {
            handler: Default::default(),
            landmarks: Default::default(),
            view_regions: Default::default(),
            root: None,
            hovered: None,
            outside_view: false,
            pointer: None,
            modifiers: Default::default(),
            pressed: false,
        }
    }
}

impl<C, Hover> DispatchContext<C, Hover> {
    pub fn new(root: Option<Root>, hovered: Option<Hover>) -> Self {
        Self {
            root,
            hovered,
            ..Self::default()
        }
    }

    pub fn hovered(&self) -> Option<&Hover> {
        if self.outside_view {
            None
        } else {
            self.hovered.as_ref()
        }
    }

    /// The geometry of the frame this dispatch runs in, at `scale`.
    pub(crate) fn geometry(&self, scale: f64) -> super::navigation::Geometry<'_, C> {
        super::navigation::Geometry {
            landmarks: &self.landmarks,
            view_regions: &self.view_regions,
            scale,
        }
    }
}

/// An editor view's settled geometry. Fixed viewports have zero scroll
/// maxima; scrolling views use them for pointer scroll and selection reveal.
pub struct ViewRegion {
    pub root: Root,
    pub rect: Rect,
    pub maximum: Vec2,
    /// A scrolling view's whole content in logical units, which an embedding
    /// page can size its frame to; fixed viewports have none.
    #[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
    pub content: Option<Vec2>,
}

/// What this frame's hover and selection refer to, derived once after
/// placement and shared by its continuations.
pub struct Attribution<Hover> {
    pub hovered: Option<Hover>,
    /// The cell-relative location the hover refers to; its other
    /// projections carry the faint secondary mark.
    pub hovered_secondary: Option<Secondary>,
    /// The same for the selected occurrence, whose other projections carry
    /// the strong mark in every view.
    pub selected_secondary: Option<Secondary>,
    /// The actual structural source under the pointer, independent of
    /// value-equivalence highlighting.
    pub hovered_trace: Option<SourceTrace>,
}

pub struct Effects<C, H> {
    pub renders: Vec<Render>,
    pub handler: Option<Handler<C, DispatchContext<C, H>>>,
}

/// Completed widget output: no hover work remains, and painting is optional.
pub struct FrameOutput<C, H> {
    pub scroll_probes: Vec<super::scroll::Probe>,
    pub renders: Vec<Render>,
    pub handler: Option<Handler<C, DispatchContext<C, H>>>,
    pub landmarks: Vec<Landmark<C>>,
    pub view_regions: Vec<ViewRegion>,
    pub hover_geometry: HoverGeometry<H>,
    pub input_area: Option<Rect>,
}

impl<C: 'static, H: 'static> Default for Effects<C, H> {
    fn default() -> Self {
        Self {
            renders: Vec::new(),
            handler: None,
        }
    }
}

impl<C: 'static, H: 'static> HasHandler<C> for Effects<C, H> {
    type Input = DispatchContext<C, H>;
    fn handler(&mut self) -> &mut Handler<C, Self::Input> {
        self.handler.get_or_insert_with(Handler::new)
    }
}

pub struct HoverOutput<C, Hover> {
    pub scroll_probes: Vec<super::scroll::Probe>,
    pub claim: Option<(Option<Root>, Claim<Hover>)>,
    pub hover_geometry: HoverGeometry<Hover>,
    /// `None` until something registers: combining empty frames must
    /// not deepen the dispatch chain.
    pub handler: Option<Handler<C, DispatchContext<C, Hover>>>,
    pub landmarks: Vec<Landmark<C>>,
    pub view_regions: Vec<ViewRegion>,
    /// A projected control may override how its occurrence's landmark is
    /// selected. The nearest enclosing landmark consumes this while
    /// assembling this frame, so it never leaks into an ancestor.
    pub landmark_select: Option<(crate::occurrence::Trail, Select<C>)>,
    /// A widget marks its occurrence as a place to fill in, which Tab
    /// visits. The nearest enclosing landmark consumes it, as it does an
    /// arrival override.
    pub tab_stop: Option<crate::occurrence::Trail>,
    pub completion: Option<Offers<C>>,
    /// The focused text editor's caret, for platform text composition.
    pub input_area: Option<Rect>,
    pub after_hover: AfterHover<Attribution<Hover>, Effects<C, Hover>>,
}

/// Preserve the destination's ordering while avoiding an allocation
/// and element moves when an empty parent can simply take ownership of
/// its first child's buffer.
fn append<T>(base: &mut Vec<T>, mut above: Vec<T>) {
    if base.is_empty() {
        *base = above;
    } else {
        base.append(&mut above);
    }
}

impl<C: 'static, Hover: 'static> Default for HoverOutput<C, Hover> {
    fn default() -> Self {
        Self {
            scroll_probes: Vec::new(),
            claim: None,
            hover_geometry: HoverGeometry::default(),
            handler: None,
            landmarks: Vec::new(),
            view_regions: Vec::new(),
            landmark_select: None,
            tab_stop: None,
            completion: None,
            input_area: None,
            after_hover: AfterHover::default(),
        }
    }
}

impl<C: 'static, Hover: 'static> HoverOutput<C, Hover> {
    /// Stack `above` on top of this output, as a later placement lands
    /// over an earlier one.
    pub fn over(mut self, above: Self) -> Self {
        append(&mut self.scroll_probes, above.scroll_probes);
        self.claim = claim_over(self.claim, above.claim);
        append(&mut self.hover_geometry.probes, above.hover_geometry.probes);
        append(
            &mut self.hover_geometry.cursors,
            above.hover_geometry.cursors,
        );
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(base.over(above)),
        };
        append(&mut self.landmarks, above.landmarks);
        append(&mut self.view_regions, above.view_regions);
        self.landmark_select = above.landmark_select.or(self.landmark_select);
        self.tab_stop = above.tab_stop.or(self.tab_stop);
        self.completion = above.completion.or(self.completion);
        self.input_area = above.input_area.or(self.input_area);
        self.after_hover.append(above.after_hover);
        self
    }

    pub fn attribute_view(&mut self, root: &Root) {
        if let Some((owner, _)) = &mut self.claim {
            *owner = Some(root.clone());
        }
        for (owner, _) in &mut self.hover_geometry.probes {
            *owner = Some(root.clone());
        }
        for (owner, _, _) in &mut self.hover_geometry.cursors {
            *owner = Some(root.clone());
        }
        for landmark in &mut self.landmarks {
            landmark.root = Some(root.clone());
        }
        let root = root.clone();
        self.map_effects(
            move |handler| view_handler(handler, root.clone()),
            |mut renders, frame| frame.append(&mut renders),
        );
    }

    pub fn handler_mut(&mut self) -> &mut Handler<C, DispatchContext<C, Hover>> {
        self.handler.get_or_insert_with(Handler::new)
    }

    pub fn bind(self, attribution: Attribution<Hover>) -> FrameOutput<C, Hover> {
        let mut effects = Effects {
            renders: Vec::new(),
            handler: self.handler,
        };
        self.after_hover.bind(Rc::new(attribution), &mut effects);
        FrameOutput {
            scroll_probes: self.scroll_probes,
            renders: effects.renders,
            handler: effects.handler,
            landmarks: self.landmarks,
            view_regions: self.view_regions,
            hover_geometry: self.hover_geometry,
            input_area: self.input_area,
        }
    }

    fn clipped(mut self, placement: Placement) -> Self {
        for probe in &mut self.scroll_probes {
            probe.placement.clip_rect = probe
                .placement
                .clip_rect
                .intersect(placement.rect)
                .intersect(placement.clip_rect);
        }
        self.map_effects(
            move |handler| container::gate_starts(handler, placement),
            move |renders, frame| {
                frame.push(Box::new(move |canvas| {
                    canvas.clip(placement.rect, Affine::IDENTITY, |canvas| {
                        puri::frame::render(renders, canvas)
                    })
                }))
            },
        );
        self
    }

    /// Passes this output's handler through `handler`, and, once hover
    /// settles, the handler its continuations add too; `renders` adds their
    /// renders to the frame's.
    fn map_effects<Map>(
        &mut self,
        handler: Map,
        renders: impl FnOnce(Vec<Render>, &mut Vec<Render>) + 'static,
    ) where
        Map: Fn(Handler<C, DispatchContext<C, Hover>>) -> Handler<C, DispatchContext<C, Hover>>,
        Map: 'static,
    {
        self.handler = self.handler.take().map(&handler);
        let after = std::mem::take(&mut self.after_hover);
        self.after_hover.push(move |hover, output| {
            let mut child = Effects::default();
            after.bind(hover, &mut child);
            renders(child.renders, &mut output.renders);
            if let Some(child) = child.handler {
                output.handler().stack(handler(child));
            }
        });
    }
}

fn view_handler<C: 'static, H: 'static>(
    inner: Handler<C, DispatchContext<C, H>>,
    root: Root,
) -> Handler<C, DispatchContext<C, H>> {
    Handler::from_function(move |ctx, event, pointer: &mut DispatchContext<C, H>| {
        let outside = pointer.outside_view;
        pointer.outside_view = pointer.root.as_ref() != Some(&root);
        let outcome = inner.dispatch(ctx, event, pointer);
        pointer.outside_view = outside;
        outcome
    })
}

impl<H> Default for Attribution<H> {
    fn default() -> Self {
        Self {
            hovered: None,
            hovered_secondary: None,
            selected_secondary: None,
            hovered_trace: None,
        }
    }
}
impl<C: 'static, H: 'static> HasHandler<C> for HoverOutput<C, H> {
    type Input = DispatchContext<C, H>;
    fn handler(&mut self) -> &mut Handler<C, Self::Input> {
        self.handler_mut()
    }
}
#[cfg(test)]
mod tests;
