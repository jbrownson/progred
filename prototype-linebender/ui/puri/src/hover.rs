//! The resting pointer: hover claims and the pointer filter that
//! steadies them.
//!
//! Hover mirrors painting: settled placements are asked back-to-front
//! what the pointer rests on, and the topmost answer wins. A claim
//! either names a target or occludes — the claim analog of an opaque
//! background fill, covering whatever lies beneath without being a
//! target itself. No answer at all is air: the question falls through
//! to whatever is below.

use kurbo::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim<H> {
    Names(H),
    Occludes,
}

impl<H> Claim<H> {
    pub fn names(self) -> Option<H> {
        match self {
            Self::Names(h) => Some(h),
            Self::Occludes => None,
        }
    }
}

/// A dead-zone filter over the pointer: a ring of radius `reach`
/// dragged by its rim, the way a fingertip slides a fixed band across
/// a table. Probing the trailing center when the pointer itself rests
/// on air is what holds a hover across small gaps; not tracking during
/// a gesture is what keeps the hover a gesture began with. Both
/// behaviors are the caller's — this is only the filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LazyPointer {
    pub center: Point,
}

impl LazyPointer {
    pub fn new(center: Point) -> Self {
        Self { center }
    }

    pub fn track(&mut self, pointer: Point, reach: f64) {
        let delta = pointer - self.center;
        let slack = delta.hypot() - reach;
        if slack > 0.0 {
            self.center += delta * (slack / delta.hypot());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_reach_the_center_holds() {
        let mut ring = LazyPointer::new(Point::new(10.0, 10.0));
        ring.track(Point::new(14.0, 10.0), 5.0);
        assert_eq!(ring.center, Point::new(10.0, 10.0));
    }

    #[test]
    fn beyond_reach_the_rim_drags_the_center() {
        let mut ring = LazyPointer::new(Point::new(10.0, 10.0));
        ring.track(Point::new(18.0, 10.0), 5.0);
        assert_eq!(ring.center, Point::new(13.0, 10.0));
        ring.track(Point::new(13.0, 2.0), 5.0);
        assert_eq!(ring.center, Point::new(13.0, 7.0));
    }

    #[test]
    fn a_stationary_pointer_never_moves_the_center() {
        let mut ring = LazyPointer::new(Point::new(3.0, 4.0));
        ring.track(Point::new(3.0, 4.0), 5.0);
        assert_eq!(ring.center, Point::new(3.0, 4.0));
    }

    #[test]
    fn claims_name_or_occlude() {
        assert_eq!(Claim::Names(7).names(), Some(7));
        assert_eq!(Claim::<u32>::Occludes.names(), None);
    }
}
