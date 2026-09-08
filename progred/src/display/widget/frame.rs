//! Placement runs hover probes and returns continuations for the resolved hover.
use super::{
    container::{self, Layers},
    navigation::{Landmark, Select},
    offers::Offers,
    source::{Secondary, SourceTrace},
    view::Root,
};
use measured::{Measured, Output};
use puri::draw::{Canvas, CanvasSink};
use puri::frame::AfterHover;
pub use puri::frame::Render;
use puri::handler::{Event, Handler, HasHandler};
use puri::hover::Claim;
pub use puri::hover::Probe;
use puri::{Affine, Placement, Point, Rect, Vec2};
use std::rc::Rc;

pub struct HoverInput<'a, H> {
    pub pointer: Option<Point>,
    pub prior: Option<&'a H>,
    pub reach: f64,
    pub debug_geometry: bool,
}

impl<H> Copy for HoverInput<'_, H> {}
impl<H> Clone for HoverInput<'_, H> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<H> Default for HoverInput<'_, H> {
    fn default() -> Self {
        Self {
            pointer: None,
            prior: None,
            reach: 0.0,
            debug_geometry: false,
        }
    }
}

/// Transient access to caller-owned output during a widget's hover computation.
/// The input is not retained by the resulting paint or event continuations.
pub struct HoverContext<'a, C, H> {
    pub input: HoverInput<'a, H>,
    pub(super) output: &'a mut HoverOutput<C, H>,
    root: Option<Root>,
}

impl<'a, C, H> HoverContext<'a, C, H> {
    pub fn new(input: HoverInput<'a, H>, output: &'a mut HoverOutput<C, H>) -> Self {
        Self {
            input,
            output,
            root: None,
        }
    }
}

impl<C: 'static, H: 'static> HoverContext<'_, C, H> {
    pub fn after_hover(
        &mut self,
        next: impl FnOnce(Rc<ResolvedHover<H>>, &mut Effects<C, H>) + 'static,
    ) {
        self.output.after_hover.push(next);
    }

    pub fn render(
        &mut self,
        render: impl FnOnce(&mut dyn CanvasSink, &ResolvedHover<H>) + 'static,
    ) {
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

    pub fn on_arrival(&mut self, select: Option<Select<C>>) {
        self.output.landmark_select = select.or(self.output.landmark_select.take());
    }

    pub fn completion(&mut self) -> &mut Option<Offers<C>> {
        &mut self.output.completion
    }

    pub fn view_region(&mut self, region: ViewRegion) {
        self.output.view_regions.push(region);
    }
}
impl<C: 'static, H: Clone + PartialEq + 'static> HoverContext<'_, C, H> {
    pub fn claim(&mut self, probe: Probe<H>) {
        self.output.claim = claim_over(
            self.output.claim.take(),
            self.input
                .pointer
                .and_then(|point| probe.answer(point, self.input.prior, self.input.reach))
                .map(|claim| (self.root.clone(), claim)),
        );
        if self.input.debug_geometry {
            if let Some(region) = probe.retention_region(self.input.reach) {
                self.output.debug_regions.push(region);
            }
        }
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
    pointer: Option<Point>,
    prior: Option<H>,
    reach: f64,
    debug_geometry: bool,
    root: Option<Root>,
    output: HoverOutput<C, H>,
    floaters: Vec<Box<dyn FnOnce(&mut Self)>>,
}

impl<C: 'static, H: 'static> HoverPass<C, H> {
    pub fn new(input: &HoverInput<'_, H>) -> Self
    where
        H: Clone,
    {
        Self {
            pointer: input.pointer,
            prior: input.prior.cloned(),
            reach: input.reach,
            debug_geometry: input.debug_geometry,
            root: None,
            output: HoverOutput::empty(),
            floaters: Vec::new(),
        }
    }

    pub(crate) fn visit(&mut self, step: impl FnOnce(&mut HoverContext<'_, C, H>)) {
        let mut context = HoverContext::new(
            HoverInput {
                pointer: self.pointer,
                prior: self.prior.as_ref(),
                reach: self.reach,
                debug_geometry: self.debug_geometry,
            },
            &mut self.output,
        );
        context.root = self.root.clone();
        step(&mut context);
        if let Some(handler) = self.output.handler.take() {
            self.output.after_hover.push(move |_, output| {
                *output.handler() = std::mem::take(output.handler()).over(handler);
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
        self.scope(content, |mut output| {
            output.root_navigation(&root);
            output
        });
        self.root = parent;
    }

    fn run_floaters(&mut self) {
        for floater in std::mem::take(&mut self.floaters) {
            floater(self);
            self.run_floaters();
        }
    }

    pub fn finish(mut self) -> HoverOutput<C, H> {
        self.run_floaters();
        self.output
    }
}
impl<C: 'static, H: 'static> Layers for HoverPass<C, H> {
    fn clipped(&mut self, placement: Placement, content: impl FnOnce(&mut Self)) {
        self.scope(content, |output| output.clipped(placement));
    }
    fn float(&mut self, above: impl FnOnce(&mut Self) + 'static) {
        let root = self.root.clone();
        self.floaters.push(Box::new(move |pass| match root {
            Some(root) => pass.in_view(root, above),
            None => above(pass),
        }));
    }
}

pub fn place<C: 'static, H: Clone + 'static>(
    layout: Measured<HoverPass<C, H>>,
    placement: Placement,
    input: &HoverInput<'_, H>,
) -> HoverOutput<C, H> {
    let mut pass = HoverPass::new(input);
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

/// The settled target is an explicit dispatch input, never an input to
/// description or placement. Accepted handlers perform their own actions.
pub struct DispatchContext<C, Hover> {
    pub descends: std::rc::Rc<[Landmark<C>]>,
    pub root: Option<Root>,
    pub hovered: Option<Hover>,
    pub outside_view: bool,
}

impl<C, Hover> Default for DispatchContext<C, Hover> {
    fn default() -> Self {
        Self {
            descends: Default::default(),
            root: None,
            hovered: None,
            outside_view: false,
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

    pub fn matches(&self, target: &Hover) -> bool
    where
        Hover: PartialEq,
    {
        !self.outside_view && self.hovered.as_ref() == Some(target)
    }
}

/// An editor view's settled geometry. Fixed viewports have zero scroll
/// maxima; scrolling views use them for pointer scroll and selection reveal.
pub struct ViewRegion {
    pub root: Root,
    pub rect: Rect,
    pub maximum: Vec2,
}

/// The winner and its source attribution, freshly derived for this frame.
pub struct ResolvedHover<Hover> {
    pub hovered: Option<Hover>,
    /// The cell-relative location the hover refers to; its other
    /// projections carry the faint secondary mark.
    pub hovered_secondary: Option<Secondary>,
    /// The actual structural source under the pointer, independent of
    /// value-equivalence highlighting.
    pub hovered_trace: Option<SourceTrace>,
}

pub struct Effects<C, H> {
    pub renders: Vec<Render>,
    pub handler: Option<Handler<C, DispatchContext<C, H>>>,
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
    pub claim: Option<(Option<Root>, Claim<Hover>)>,
    pub debug_regions: Vec<(Hover, Rect)>,
    /// `None` until something registers: combining empty frames must
    /// not deepen the dispatch chain.
    pub handler: Option<Handler<C, DispatchContext<C, Hover>>>,
    pub descends: Vec<Landmark<C>>,
    pub view_regions: Vec<ViewRegion>,
    /// A projected control may override how the nearest enclosing
    /// navigation landmark is selected. The landmark consumes this
    /// while assembling this frame, so it never leaks into an ancestor.
    pub landmark_select: Option<Select<C>>,
    pub completion: Option<Offers<C>>,
    pub after_hover: AfterHover<ResolvedHover<Hover>, Effects<C, Hover>>,
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

impl<C: 'static, Hover: 'static> Output for HoverOutput<C, Hover> {
    fn empty() -> Self {
        Self {
            claim: None,
            debug_regions: Vec::new(),
            handler: None,
            descends: Vec::new(),
            view_regions: Vec::new(),
            landmark_select: None,
            completion: None,
            after_hover: AfterHover::default(),
        }
    }

    fn over(mut self, above: Self) -> Self {
        self.claim = claim_over(self.claim, above.claim);
        append(&mut self.debug_regions, above.debug_regions);
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(base.over(above)),
        };
        append(&mut self.descends, above.descends);
        append(&mut self.view_regions, above.view_regions);
        self.landmark_select = above.landmark_select.or(self.landmark_select);
        self.completion = above.completion.or(self.completion);
        self.after_hover.append(above.after_hover);
        self
    }
}

impl<C: 'static, Hover: 'static> HoverOutput<C, Hover> {
    pub fn root_navigation(&mut self, root: &Root) {
        if let Some((owner, _)) = &mut self.claim {
            *owner = Some(root.clone());
        }
        for descend in &mut self.descends {
            descend.root = Some(root.clone());
        }
        let after = std::mem::take(&mut self.after_hover);
        let root = root.clone();
        self.after_hover.push(move |hover, output| {
            let mut child = Effects::default();
            after.bind(hover, &mut child);
            output.renders.append(&mut child.renders);
            if let Some(handler) = &mut child.handler {
                let root = root.clone();
                let inner = std::mem::take(handler);
                *handler = Handler::from_function(
                    move |ctx, event, pointer: &mut DispatchContext<C, Hover>| {
                        if matches!(event, Event::PointerDown(_)) {
                            let outside = pointer.outside_view;
                            pointer.outside_view = pointer.root.as_ref() != Some(&root);
                            let outcome = inner.dispatch(ctx, event, pointer);
                            pointer.outside_view = outside;
                            outcome
                        } else {
                            inner.dispatch(ctx, event, pointer)
                        }
                    },
                );
            }
            if let Some(handler) = child.handler {
                *output.handler() = std::mem::take(output.handler()).over(handler);
            }
        });
    }

    pub fn handler_mut(&mut self) -> &mut Handler<C, DispatchContext<C, Hover>> {
        self.handler.get_or_insert_with(Handler::new)
    }

    pub fn resolve(&mut self, hover: ResolvedHover<Hover>) -> Vec<Render> {
        let mut effects = Effects {
            renders: Vec::new(),
            handler: self.handler.take(),
        };
        std::mem::take(&mut self.after_hover).bind(Rc::new(hover), &mut effects);
        self.handler = effects.handler;
        effects.renders
    }
}

impl<H> Default for ResolvedHover<H> {
    fn default() -> Self {
        Self {
            hovered: None,
            hovered_secondary: None,
            hovered_trace: None,
        }
    }
}
impl<C: 'static, H: 'static> Default for HoverOutput<C, H> {
    fn default() -> Self {
        Self::empty()
    }
}
impl<C: 'static, H: 'static> HasHandler<C> for HoverOutput<C, H> {
    type Input = DispatchContext<C, H>;
    fn handler(&mut self) -> &mut Handler<C, Self::Input> {
        self.handler_mut()
    }
}
impl<C: 'static, H: 'static> HoverOutput<C, H> {
    fn clipped(mut self, placement: Placement) -> Self {
        let after = std::mem::take(&mut self.after_hover);
        self.after_hover.push(move |hover, output| {
            let mut child = Effects::default();
            after.bind(hover, &mut child);
            output.renders.push(Box::new(move |canvas| {
                canvas.clip(placement.rect, Affine::IDENTITY, |canvas| {
                    puri::frame::render(child.renders, canvas)
                })
            }));
            if let Some(handler) = child.handler {
                *output.handler() = std::mem::take(output.handler())
                    .over(container::gate_starts(handler, placement));
            }
        });
        self.handler = self
            .handler
            .map(|handler| container::gate_starts(handler, placement));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::super::leaf;
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn hover_runs_in_paint_order_and_handlers_run_front_to_back() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
        let widget = |id| {
            let calls = calls.clone();
            leaf(
                measured::Extent {
                    width: 10.0,
                    ascent: 10.0,
                    descent: 0.0,
                },
                move |output: &mut HoverContext<'_, Vec<u8>, u8>, placement| {
                    calls.borrow_mut().push(id);
                    let log = calls.clone();
                    output.claim(Probe::dynamic(placement, move |_| {
                        log.borrow_mut().push(id + 10);
                        Some(id)
                    }));
                    output.after_hover(move |hover, effects| {
                        assert_eq!(hover.hovered, Some(2));
                        calls.borrow_mut().push(id + 20);
                        effects
                            .renders
                            .push(Box::new(move |_| calls.borrow_mut().push(id + 30)));
                        effects.handler().on_key(move |log, _| {
                            log.push(id);
                            false
                        });
                    });
                },
            )
        };
        let layout = measured::layers(vec![widget(1), widget(2)]);
        assert!(calls.borrow().is_empty());
        let mut output = place(
            layout,
            placement,
            &HoverInput {
                pointer: Some(placement.rect.center()),
                ..Default::default()
            },
        );
        assert_eq!(&*calls.borrow(), &[1, 11, 2, 12]);
        assert_eq!(output.claim, Some((None, Claim::Direct(2))));
        let renders = output.resolve(ResolvedHover {
            hovered: Some(2),
            ..Default::default()
        });
        assert_eq!(&*calls.borrow(), &[1, 11, 2, 12, 21, 22]);
        puri::frame::render(renders, &mut puri::DrawList::new());
        assert_eq!(&*calls.borrow(), &[1, 11, 2, 12, 21, 22, 31, 32]);
        let mut log = Vec::new();
        output
            .handler
            .unwrap()
            .dispatch_key(&mut log, &Default::default());
        assert_eq!(log, [2, 1]);
    }

    #[test]
    fn a_frame_can_discard_paint_and_still_dispatch() {
        let mut pass = HoverPass::<usize, ()>::new(&Default::default());
        pass.visit(|output| {
            output.render(|_, _| panic!("paint is optional"));
            output.after_hover(|_, effects| {
                effects.handler().on_key(|count, _| {
                    *count += 1;
                    true
                });
            });
        });
        let mut frame = pass.finish();
        drop(frame.resolve(Default::default()));
        let mut count = 0;
        assert!(
            frame
                .handler
                .unwrap()
                .dispatch_key(&mut count, &Default::default())
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn nested_floaters_follow_their_parent_before_later_siblings() {
        fn mark(pass: &mut HoverPass<Vec<u8>, u8>, id: u8) {
            pass.visit(move |output| {
                output.claim(Probe::exact(
                    Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                    id,
                ));
                output.handler().on_key(move |log, _| {
                    log.push(id);
                    false
                });
            });
        }
        let mut pass = HoverPass::new(&HoverInput {
            pointer: Some(Point::new(5.0, 5.0)),
            ..Default::default()
        });
        pass.float(|pass| {
            mark(pass, 1);
            pass.float(|pass| mark(pass, 2));
        });
        pass.float(|pass| mark(pass, 3));
        mark(&mut pass, 0);
        let mut output = pass.finish();
        assert_eq!(output.claim, Some((None, Claim::Direct(3))));
        output.resolve(ResolvedHover {
            hovered: Some(3),
            ..Default::default()
        });
        let mut log = Vec::new();
        output
            .handler
            .unwrap()
            .dispatch_key(&mut log, &Default::default());
        assert_eq!(log, [3, 2, 1, 0]);
    }

    #[test]
    fn debug_retention_geometry_does_not_change_the_hover_answer() {
        let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
        for debug_geometry in [false, true] {
            let mut pass = HoverPass::<(), u8>::new(&HoverInput {
                pointer: Some(placement.rect.center()),
                debug_geometry,
                reach: 4.0,
                ..Default::default()
            });
            for id in [1, 2] {
                pass.visit(|output| output.claim(Probe::retaining(placement, id)));
            }
            let frame = pass.finish();
            assert_eq!(frame.claim, Some((None, Claim::Direct(2))));
            assert_eq!(
                frame.debug_regions.len(),
                if debug_geometry { 2 } else { 0 }
            );
        }
    }
}
