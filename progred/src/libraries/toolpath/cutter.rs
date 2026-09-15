//! Cutter geometry shared by direct mesh display, implicit display, and subtraction.
use super::{
    fidget::{capsule, coordinate, read_radius},
    paths::{Axis, InvalidPath, Point3},
};
use fidget_engine::context::Tree;

mod profile;
mod value;
mod view;
pub use profile::{Bend, Point, Section, SectionKind, Segment, Tool};
pub use value::vocabulary;
pub(super) use value::{functions, names};
pub(super) use view::display;

#[cfg(test)]
mod tests;

/// Exact lowering for a hemisphere followed by a cylindrical profile.
/// Private to profile lowering; it is not a second authorable tool definition.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BallEnd {
    radius: f32,
    length: f32,
}

impl BallEnd {
    pub fn new(radius: f64, length: f64) -> Option<Self> {
        let radius = read_radius(radius)?;
        let length = length as f32;
        (length.is_finite() && length >= 2.0 * radius).then_some(Self { radius, length })
    }

    /// Axial coordinates are relative to the ball center, not the tip.
    pub fn flute_height(self) -> f32 {
        self.length - self.radius
    }

    pub fn sweep(&self, a: Point3, b: Point3, axis: Axis) -> Result<Tree, InvalidPath> {
        let a = coordinate(a)?;
        let b = coordinate(b)?;
        let ball = capsule(a, b, self.radius)?;
        let basis = axis.basis();
        let direction = nalgebra::Vector3::from(b) - nalgebra::Vector3::from(a);
        let d = basis.map(|v| v.dot(&direction));
        if !d.into_iter().all(f32::is_finite) {
            return Err(InvalidPath::CoordinateRange);
        }
        let world = [Tree::x() - a[0], Tree::y() - a[1], Tree::z() - a[2]];
        let p =
            basis.map(|v| world[0].clone() * v.x + world[1].clone() * v.y + world[2].clone() * v.z);
        let height = self.flute_height();
        let xy_squared = d[0] * d[0] + d[1] * d[1];

        // At this Z, only a subinterval of the motion places the finite flute
        // here. Minimize radial distance over that interval, not over the whole
        // segment (which would overcut a sloping move near the flute's ends).
        let (lo, hi) = if d[2] == 0.0 {
            (Tree::constant(0.0), Tree::constant(1.0))
        } else {
            let t0 = (p[2].clone() - height) / d[2];
            let t1 = p[2].clone() / d[2];
            (
                t0.clone().min(t1.clone()).max(0.0).min(1.0),
                t0.max(t1).max(0.0).min(1.0),
            )
        };
        let t = if xy_squared == 0.0 {
            lo
        } else {
            ((p[0].clone() * d[0] + p[1].clone() * d[1]) / xy_squared)
                .max(lo)
                .min(hi)
        };
        let radial = (p[0].clone() - t.clone() * d[0]).square()
            + (p[1].clone() - t * d[1]).square()
            - self.radius * self.radius;
        let bottom = d[2].min(0.0) - p[2].clone();
        let top = p[2].clone() - (d[2].max(0.0) + height);
        Ok(ball.min(radial.max(bottom).max(top)))
    }
}
