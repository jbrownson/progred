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

use crate::{Placement, Point, Rect};

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

impl<H: PartialEq> Probe<H> {
    pub fn extended_rect(&self, prior: &H, reach: f64) -> Option<Rect> {
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

impl<H: Clone + PartialEq> Probe<H> {
    pub fn retention_region(&self, reach: f64) -> Option<(H, Rect)> {
        match &self.target {
            Target::Retains(target) => self
                .extended_rect(target, reach)
                .map(|rect| (target.clone(), rect)),
            _ => None,
        }
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painter_order_claim_composition_is_associative() {
        let choices = [
            None,
            Some(Claim::Direct(1)),
            Some(Claim::Direct(2)),
            Some(Claim::Extended(1)),
            Some(Claim::Extended(2)),
            Some(Claim::Occludes),
        ];
        let over = |base: Option<Claim<u8>>, above: Option<Claim<u8>>| {
            if above
                .as_ref()
                .is_some_and(|claim| claim.supersedes(base.as_ref()))
            {
                above
            } else {
                base
            }
        };
        for a in choices {
            for b in choices {
                for c in choices {
                    assert_eq!(over(over(a, b), c), over(a, over(b, c)));
                }
            }
        }
        assert_eq!(
            over(Some(Claim::Direct(1)), Some(Claim::Extended(2))),
            Some(Claim::Direct(1))
        );
        assert_eq!(
            over(Some(Claim::Occludes), Some(Claim::Direct(2))),
            Some(Claim::Direct(2))
        );
    }

    #[test]
    fn claims_distinguish_establishing_retaining_and_occluding() {
        assert_eq!(Claim::Direct(7), Claim::Direct(7));
        assert_eq!(Claim::Extended(7), Claim::Extended(7));
        assert_eq!(Claim::<u32>::Occludes, Claim::Occludes);
    }

    #[test]
    fn probes_preserve_their_policy_when_targets_are_mapped() {
        let placement = Placement::new(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(0.0, 0.0, 12.0, 12.0),
        );
        for (probe, extends) in [
            (Probe::retaining(placement, 3), true),
            (Probe::exact(placement, 3), false),
            (
                Probe::dynamic(placement, |point| (point.x > 5.0).then_some(3)),
                false,
            ),
        ] {
            let probe = probe.map(|value| value + 1);
            assert_eq!(
                probe.answer(Point::new(7.0, 5.0), None, 4.0),
                Some(Claim::Direct(4))
            );
            assert_eq!(probe.answer(Point::new(11.0, 5.0), None, 4.0), None);
            assert_eq!(
                probe.answer(Point::new(11.0, 5.0), Some(&4), 4.0),
                extends.then_some(Claim::Extended(4))
            );
            assert_eq!(probe.answer(Point::new(13.0, 5.0), Some(&4), 4.0), None);
        }
        let occluder = Probe::<u32>::occludes(placement).map(|v| v + 1);
        assert_eq!(
            occluder.answer(Point::new(5.0, 5.0), Some(&4), 4.0),
            Some(Claim::Occludes)
        );
        assert_eq!(occluder.answer(Point::new(11.0, 5.0), Some(&4), 4.0), None);
    }
}
