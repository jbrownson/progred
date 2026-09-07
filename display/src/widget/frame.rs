//! One placement output shared by native controls and the editor shell.
use super::{
    container::{self, Layers},
    navigation::{Landmark, Navigation, Select},
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
/// A native hover region scoped to its editor view.
pub struct Probe<Hover> {
    root: Option<Root>,
    region: puri::hover::Probe<Hover>,
}

impl<Hover> Probe<Hover> {
    pub fn new(region: puri::hover::Probe<Hover>) -> Self {
        Self { root: None, region }
    }

    pub fn answer(&self, point: Point, prior: Option<&Hover>, reach: f64) -> Option<Claim<Hover>>
    where
        Hover: Clone + PartialEq,
    {
        self.region.answer(point, prior, reach)
    }
    pub fn retaining(placement: Placement, target: Hover) -> Self {
        Self::new(puri::hover::Probe::retaining(placement, target))
    }

    pub fn exact(placement: Placement, target: Hover) -> Self {
        Self::new(puri::hover::Probe::exact(placement, target))
    }

    pub fn occludes(placement: Placement) -> Self {
        Self::new(puri::hover::Probe::occludes(placement))
    }

    pub fn dynamic(
        placement: Placement,
        target: impl Fn(Point) -> Option<Hover> + 'static,
    ) -> Self {
        Self::new(puri::hover::Probe::dynamic(placement, target))
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
    pub probes: Vec<Probe<Hover>>,
    /// `None` until something registers: combining empty frames must
    /// not deepen the dispatch chain.
    pub handler: Option<Handler<C, DispatchContext<C, Hover>>>,
    pub descends: Vec<Landmark<C>>,
    pub view_regions: Vec<ViewRegion>,
    /// A projected control may override how the nearest enclosing
    /// navigation landmark is selected. The landmark consumes this
    /// while placing, so it never leaks into an ancestor.
    pub landmark_select: Option<Select<C>>,
    pub completion: Option<Offers<C>>,
    /// Out-of-flow subtrees gathered during placement and raised over
    /// the completed frame before hover resolution.
    pub floaters: Vec<Box<Fragment<C, Hover>>>,
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
            (Some(base), Some(above)) => Some(base.over(above)),
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

impl<C: 'static, Hover: 'static> Fragment<C, Hover> {
    pub fn raise_floaters(mut self) -> Self {
        for floater in std::mem::take(&mut self.floaters) {
            self = self.over(floater.raise_floaters());
        }
        self
    }

    pub fn root_navigation(&mut self, root: &Root) {
        for probe in &mut self.probes {
            probe.root = Some(root.clone());
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
        for floater in &mut self.floaters {
            floater.root_navigation(root);
        }
    }

    /// What the pointer at `point` rests on. A direct answer or
    /// occluder wins immediately in placement order; an extension of
    /// `prior` is remembered only in case every real region is air.
    pub fn probe(&self, point: Point, prior: Option<&Hover>, reach: f64) -> Option<Claim<Hover>>
    where
        Hover: Clone + PartialEq,
    {
        self.probe_scoped(point, prior, reach)
            .map(|(_, claim)| claim)
    }

    pub fn probe_scoped(
        &self,
        point: Point,
        prior: Option<&Hover>,
        reach: f64,
    ) -> Option<(Option<Root>, Claim<Hover>)>
    where
        Hover: Clone + PartialEq,
    {
        let mut retained = None;
        for probe in self.probes.iter().rev() {
            match probe.region.answer(point, prior, reach) {
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

    pub fn extended_rects(&self, target: &Hover, reach: f64) -> Vec<Rect>
    where
        Hover: PartialEq,
    {
        self.probes
            .iter()
            .filter_map(|probe| probe.region.extended_rect(target, reach))
            .collect()
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
impl<C: 'static, H: 'static> Navigation<C> for Fragment<C, H> {
    fn landmark_select(&mut self) -> &mut Option<Select<C>> {
        &mut self.landmark_select
    }
    fn push_landmark(&mut self, landmark: Landmark<C>) {
        self.descends.push(landmark);
    }
}
impl<C: 'static, H: 'static> HasHandler<C> for Fragment<C, H> {
    type Input = DispatchContext<C, H>;
    fn handler(&mut self) -> &mut Handler<C, Self::Input> {
        self.handler_mut()
    }
}
impl<C: 'static, H: 'static> Layers for Fragment<C, H> {
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
    fn float(&mut self, above: Self) {
        self.floaters.push(Box::new(above));
    }
}
impl<C, H: 'static> Fragment<C, H> {
    pub fn render(&mut self, render: impl FnOnce(&mut dyn CanvasSink, Option<&H>) + 'static) {
        self.renders
            .push(Box::new(move |canvas, ink| render(canvas, ink.hovered)));
    }
}
