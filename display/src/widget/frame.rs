//! Placement produces a hover computation; running it produces ink and dispatch.
use super::{
    container::{self, Layers},
    navigation::{Landmark, Select},
    offers::Offers,
    source::{Secondary, SourceTrace},
    view::Root,
};
use measured::Output;
use puri::draw::{Canvas, CanvasSink};
use puri::handler::{Event, Handler, HasHandler};
use puri::hover::Claim;
use puri::{Affine, Placement, Point, Rect, Vec2};
pub type Render<Hover> = Box<dyn for<'ink> FnOnce(&mut dyn CanvasSink, Ink<'ink, Hover>)>;
pub use puri::hover::Probe;

pub struct HoverInput<'a, H> {
    pub pointer: Option<Point>,
    pub prior: Option<&'a H>,
    pub reach: f64,
    pub debug_geometry: bool,
    /// An upper direct claim or occluder has already won this pass.
    pub occluded: bool,
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
            occluded: false,
        }
    }
}

/// Transient access to caller-owned output during a widget's hover computation.
/// The input is not retained by the resulting paint or event continuations.
pub struct HoverContext<'a, C, H> {
    pub input: HoverInput<'a, H>,
    pub(super) output: &'a mut Fragment<C, H>,
}

impl<'a, C: 'static, H: 'static> HoverContext<'a, C, H> {
    pub fn new(input: HoverInput<'a, H>, output: &'a mut Fragment<C, H>) -> Self {
        Self { input, output }
    }
}

impl<C: 'static, H: 'static> HoverContext<'_, C, H> {
    pub fn render(&mut self, render: impl FnOnce(&mut dyn CanvasSink, Option<&H>) + 'static) {
        self.output.render(render);
    }

    pub fn ink(
        &mut self,
        render: impl for<'ink> FnOnce(&mut dyn CanvasSink, Ink<'ink, H>) + 'static,
    ) {
        self.output.renders.push(Box::new(render));
    }

    pub fn with_clip(
        &mut self,
        shape: puri::draw::Shape,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let start = self.output.renders.len();
        content(self);
        let renders = self.output.renders.split_off(start);
        self.ink(move |canvas, ink| {
            canvas.clip(shape, transform, |canvas| {
                Fragment::<C, H>::paint(renders, canvas, ink)
            })
        });
    }

    pub fn on_arrival(&mut self, select: Option<Select<C>>) {
        self.output.landmark_select = select;
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
                .filter(|_| !self.input.occluded)
                .and_then(|point| probe.answer(point, self.input.prior, self.input.reach))
                .map(|claim| (None, claim)),
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

type HoverStep<C, H> = Box<dyn FnOnce(&HoverInput<'_, H>, &mut Fragment<C, H>)>;

/// A composition of one-shot functions, opaque to measurement and placement.
/// Flat composition avoids a call-stack frame per sibling.
pub struct HoverPass<C, H> {
    steps: Vec<HoverStep<C, H>>,
    floaters: Vec<HoverPass<C, H>>,
}

impl<C: 'static, H: 'static> HoverPass<C, H> {
    pub fn new(step: impl FnOnce(&mut HoverContext<'_, C, H>) + 'static) -> Self {
        let mut pass = Self::empty();
        pass.push(step);
        pass
    }

    pub(crate) fn push(&mut self, step: impl FnOnce(&mut HoverContext<'_, C, H>) + 'static) {
        self.steps.push(Box::new(move |input, output| {
            let start = output.lengths();
            let above = output.take_controls();
            step(&mut HoverContext::new(*input, output));
            output.reverse_since(start);
            output.controls_below(above);
        }));
    }

    pub fn run(self, input: &HoverInput<'_, H>) -> Fragment<C, H> {
        let mut output = Fragment::empty();
        let mut input = *input;
        for step in self.raise_floaters().steps.into_iter().rev() {
            step(&input, &mut output);
            if matches!(output.claim, Some((_, Claim::Direct(_) | Claim::Occludes))) {
                input.occluded = true;
            }
        }
        output.reverse_since([0; 4]);
        output
    }

    pub fn map(self, map: impl FnOnce(Fragment<C, H>) -> Fragment<C, H> + 'static) -> Self {
        let Self { steps, floaters } = self;
        Self {
            steps: vec![Box::new(move |input, output| {
                let mut below = map(Self {
                    steps,
                    floaters: Vec::new(),
                }
                .run(input));
                below.reverse_since([0; 4]);
                let above = output.take_controls();
                *output = std::mem::take(output).over(below);
                output.controls_below(above);
            })],
            floaters,
        }
    }

    fn raise_floaters(mut self) -> Self {
        for floater in std::mem::take(&mut self.floaters) {
            self = self.over(floater.raise_floaters());
        }
        self
    }

    pub fn in_view(self, root: Root) -> Self {
        let Self { steps, floaters } = self;
        let children = floaters
            .into_iter()
            .map(|floater| floater.in_view(root.clone()))
            .collect();
        let mut scoped = Self {
            steps,
            floaters: Vec::new(),
        }
        .map(move |mut output| {
            output.root_navigation(&root);
            output
        });
        scoped.floaters = children;
        scoped
    }
}

impl<C: 'static, H: 'static> Output for HoverPass<C, H> {
    fn empty() -> Self {
        Self {
            steps: Vec::new(),
            floaters: Vec::new(),
        }
    }
    fn over(mut self, above: Self) -> Self {
        append(&mut self.steps, above.steps);
        append(&mut self.floaters, above.floaters);
        self
    }
}
impl<C: 'static, H: 'static> Layers for HoverPass<C, H> {
    fn clipped(self, placement: Placement) -> Self {
        self.map(move |output| output.clipped(placement))
    }
    fn float(&mut self, above: Self) {
        self.floaters.push(above);
    }
}

fn claim_over<H>(
    base: Option<(Option<Root>, Claim<H>)>,
    above: Option<(Option<Root>, Claim<H>)>,
) -> Option<(Option<Root>, Claim<H>)> {
    match (&base, &above) {
        (_, Some((_, Claim::Direct(_) | Claim::Occludes))) => above,
        (Some(_), _) => base,
        _ => above,
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

/// What ink may condition on: the frame's RESOLVED hover, decided
/// from this same pass's geometry before any render runs.
pub struct Ink<'a, Hover> {
    pub hovered: Option<&'a Hover>,
    /// The cell-relative location the hover refers to; its other
    /// projections carry the faint secondary mark.
    pub hovered_secondary: Option<&'a Secondary>,
    /// The actual structural source under the pointer, independent of
    /// value-equivalence highlighting.
    pub hovered_trace: Option<&'a SourceTrace>,
    /// Draw each leaf's honest placement rectangle after its own ink.
    pub debug_geometry: bool,
}

pub struct Fragment<C, Hover> {
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
    pub renders: Vec<Render<Hover>>,
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

impl<C: 'static, Hover: 'static> Output for Fragment<C, Hover> {
    fn empty() -> Self {
        Self {
            claim: None,
            debug_regions: Vec::new(),
            handler: None,
            descends: Vec::new(),
            view_regions: Vec::new(),
            landmark_select: None,
            completion: None,
            renders: Vec::new(),
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
        append(&mut self.renders, above.renders);
        self
    }
}

impl<C: 'static, Hover: 'static> Fragment<C, Hover> {
    // Hover runs front-to-back. Reverse each widget's appended segment, then
    // the whole stream once, to retain back-to-front paint without per-leaf buffers.
    fn lengths(&self) -> [usize; 4] {
        [
            self.renders.len(),
            self.descends.len(),
            self.view_regions.len(),
            self.debug_regions.len(),
        ]
    }

    fn reverse_since(&mut self, [renders, descends, views, debug]: [usize; 4]) {
        self.renders[renders..].reverse();
        self.descends[descends..].reverse();
        self.view_regions[views..].reverse();
        self.debug_regions[debug..].reverse();
    }

    fn take_controls(&mut self) -> Self {
        Self {
            claim: self.claim.take(),
            handler: self.handler.take(),
            landmark_select: self.landmark_select.take(),
            completion: self.completion.take(),
            ..Self::empty()
        }
    }

    fn controls_below(&mut self, above: Self) {
        self.claim = claim_over(self.claim.take(), above.claim);
        self.handler = match (self.handler.take(), above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(base.over(above)),
        };
        self.landmark_select = above.landmark_select.or(self.landmark_select.take());
        self.completion = above.completion.or(self.completion.take());
    }

    pub fn root_navigation(&mut self, root: &Root) {
        if let Some((owner, _)) = &mut self.claim {
            *owner = Some(root.clone());
        }
        for descend in &mut self.descends {
            descend.root = Some(root.clone());
        }
        if let Some(handler) = &mut self.handler {
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
    }

    pub fn handler_mut(&mut self) -> &mut Handler<C, DispatchContext<C, Hover>> {
        self.handler.get_or_insert_with(Handler::new)
    }

    /// Run the deferred ink into a canvas, with the resolved hover.
    pub fn paint(renders: Vec<Render<Hover>>, canvas: &mut dyn CanvasSink, ink: Ink<'_, Hover>) {
        for render in renders {
            render(canvas, ink);
        }
    }
}

impl<H> Copy for Ink<'_, H> {}
impl<H> Clone for Ink<'_, H> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<H> Default for Ink<'_, H> {
    fn default() -> Self {
        Self {
            hovered: None,
            hovered_secondary: None,
            hovered_trace: None,
            debug_geometry: false,
        }
    }
}
impl<C: 'static, H: 'static> Default for Fragment<C, H> {
    fn default() -> Self {
        Self::empty()
    }
}
impl<C: 'static, H: 'static> HasHandler<C> for Fragment<C, H> {
    type Input = DispatchContext<C, H>;
    fn handler(&mut self) -> &mut Handler<C, Self::Input> {
        self.handler_mut()
    }
}
impl<C: 'static, H: 'static> Fragment<C, H> {
    fn clipped(mut self, placement: Placement) -> Self {
        let renders = std::mem::take(&mut self.renders);
        self.renders.push(Box::new(move |canvas, ink| {
            canvas.clip(placement.rect, Affine::IDENTITY, |canvas| {
                for render in renders {
                    render(canvas, ink);
                }
            })
        }));
        self.handler = self
            .handler
            .map(|handler| container::gate_starts(handler, placement));
        self
    }
}
impl<C, H: 'static> Fragment<C, H> {
    pub fn render(&mut self, render: impl FnOnce(&mut dyn CanvasSink, Option<&H>) + 'static) {
        self.renders
            .push(Box::new(move |canvas, ink| render(canvas, ink.hovered)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn hover_runs_topmost_first_but_paint_stays_back_to_front() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let pointer = Point::new(5.0, 5.0);
        let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
        let widget = |id| {
            let calls = calls.clone();
            HoverPass::new(move |output: &mut HoverContext<'_, Vec<u8>, u8>| {
                calls.borrow_mut().push(id);
                assert_eq!(output.input.pointer, Some(pointer));
                assert_eq!(output.input.occluded, id == 1);
                let query_log = calls.clone();
                output.claim(Probe::dynamic(placement, move |_| {
                    query_log.borrow_mut().push(id + 10);
                    Some(id)
                }));
                let paint_log = calls.clone();
                output.render(move |_, hovered| {
                    assert_eq!(hovered, Some(&2));
                    paint_log.borrow_mut().push(id + 20);
                });
                let paint_log = calls.clone();
                output.render(move |_, _| paint_log.borrow_mut().push(id + 30));
                output.handler().on_key(move |log, _| {
                    log.push(id);
                    false
                });
            })
        };
        let pass = widget(1).over(widget(2));
        assert!(calls.borrow().is_empty());
        let ready = pass.run(&HoverInput {
            pointer: Some(pointer),
            ..Default::default()
        });
        assert_eq!(&*calls.borrow(), &[2, 12, 1]);
        assert_eq!(ready.claim, Some((None, Claim::Direct(2))));
        Fragment::<Vec<u8>, u8>::paint(
            ready.renders,
            &mut puri::DrawList::new(),
            Ink {
                hovered: Some(&2),
                ..Default::default()
            },
        );
        assert_eq!(&*calls.borrow(), &[2, 12, 1, 21, 31, 22, 32]);
        let mut log = Vec::new();
        ready
            .handler
            .unwrap()
            .dispatch_key(&mut log, &Default::default());
        assert_eq!(log, [2, 1]);
    }

    #[test]
    fn a_frame_can_discard_paint_and_still_dispatch() {
        let pass = HoverPass::<usize, ()>::new(|output| {
            output.render(|_, _| panic!("paint is optional"));
            output.handler().on_key(|count, _| {
                *count += 1;
                true
            });
        });
        let frame = pass.run(&Default::default());
        drop(frame.renders);
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
    fn debug_retention_geometry_does_not_change_the_hover_answer() {
        let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
        for debug_geometry in [false, true] {
            let pass = HoverPass::<(), u8>::new(move |output| {
                output.claim(Probe::retaining(placement, 1));
            })
            .over(HoverPass::new(move |output| {
                output.claim(Probe::retaining(placement, 2));
            }));
            let frame = pass.run(&HoverInput {
                pointer: Some(Point::new(5.0, 5.0)),
                debug_geometry,
                reach: 4.0,
                ..Default::default()
            });
            assert_eq!(frame.claim, Some((None, Claim::Direct(2))));
            assert_eq!(
                frame.debug_regions.len(),
                if debug_geometry { 2 } else { 0 }
            );
        }
    }
}
