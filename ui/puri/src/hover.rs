//! The resting pointer's answers.
//!
//! Hover mirrors painting: settled placements are asked back-to-front
//! what the pointer rests on. A direct answer always wins; an extended
//! answer may only retain the target that was already hovered. An
//! occluder is the claim analog of an opaque background fill, covering
//! whatever lies beneath without naming a target.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim<H> {
    Direct(H),
    Extended(H),
    Occludes,
}

/// Compose in painting order: a foreground hit or occluder replaces the
/// underlying answer; retention never displaces a direct hit.
impl<H> Claim<H> {
    pub fn supersedes(&self, below: Option<&Self>) -> bool {
        match self {
            Self::Direct(_) | Self::Occludes => true,
            Self::Extended(_) => below.is_none(),
        }
    }
}

use kurbo::{Point, Rect};
use uig::Placement;

enum Target<H> {
    Retains(H),
    Exact(H),
    Dynamic(Box<dyn Fn(Point) -> Option<H>>),
    Occludes,
}

/// A hover answer over settled geometry, independent of layout and editor identity.
pub struct Probe<H> {
    placement: Placement,
    target: Target<H>,
}

impl<H> Probe<H> {
    pub fn retaining(placement: Placement, target: H) -> Self {
        Self {
            placement,
            target: Target::Retains(target),
        }
    }

    pub fn exact(placement: Placement, target: H) -> Self {
        Self {
            placement,
            target: Target::Exact(target),
        }
    }

    pub fn occludes(placement: Placement) -> Self {
        Self {
            placement,
            target: Target::Occludes,
        }
    }

    pub fn dynamic(placement: Placement, target: impl Fn(Point) -> Option<H> + 'static) -> Self {
        Self {
            placement,
            target: Target::Dynamic(Box::new(target)),
        }
    }

    pub fn map<J>(self, map: impl Fn(H) -> J + 'static) -> Probe<J>
    where
        H: 'static,
    {
        Probe {
            placement: self.placement,
            target: match self.target {
                Target::Retains(target) => Target::Retains(map(target)),
                Target::Exact(target) => Target::Exact(map(target)),
                Target::Dynamic(target) => {
                    Target::Dynamic(Box::new(move |point| target(point).map(&map)))
                }
                Target::Occludes => Target::Occludes,
            },
        }
    }
}

impl<H: Clone + PartialEq> Probe<H> {
    pub fn answer(&self, point: Point, prior: Option<&H>, reach: f64) -> Option<Claim<H>> {
        if self.placement.contains(point) {
            match &self.target {
                Target::Retains(target) | Target::Exact(target) => {
                    Some(Claim::Direct(target.clone()))
                }
                Target::Dynamic(target) => target(point).map(Claim::Direct),
                Target::Occludes => Some(Claim::Occludes),
            }
        } else {
            prior.and_then(|prior| {
                self.extended_rect(prior, reach)
                    .filter(|rect| rect.contains(point))
                    .map(|_| Claim::Extended(prior.clone()))
            })
        }
    }

    /// Where a probe that retains `prior` keeps it hovered: its visible
    /// rectangle grown by `reach`, within its clip.
    fn extended_rect(&self, prior: &H, reach: f64) -> Option<Rect> {
        match &self.target {
            Target::Retains(target) if target == prior && !self.placement.clipped_out() => {
                let rect = self
                    .placement
                    .visible_rect()
                    .inflate(reach, reach)
                    .intersect(self.placement.clip_rect);
                (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
