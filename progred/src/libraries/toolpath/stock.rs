//! Volumetric stock subtraction through ordinary Fidget expressions.

use super::{
    fidget::{capsule, coordinate, read_radius},
    paths::{InvalidPath, Point3},
};
use fidget_engine::context::Tree;

pub struct Stock {
    field: Tree,
}

impl Stock {
    pub fn block(min: Point3, max: Point3) -> Option<Self> {
        let min = coordinate(min).ok()?;
        let max = coordinate(max).ok()?;
        if !(0..3).all(|i| min[i] < max[i]) {
            return None;
        }
        let field = [Tree::x(), Tree::y(), Tree::z()]
            .into_iter()
            .enumerate()
            .map(|(i, p)| (min[i] - p.clone()).max(p - max[i]))
            .reduce(|a, b| a.max(b))?;
        Some(Self { field })
    }

    pub fn cut(&mut self, tool: &BallEnd, a: Point3, b: Point3) -> Result<(), InvalidPath> {
        let sweep = tool.sweep(a, b)?;
        self.field = self.field.max(-sweep);
        Ok(())
    }

    pub fn into_field(self) -> Tree {
        self.field
    }
}

/// A +Z ball-end cutter: a hemispherical tip and cylindrical flute with a flat
/// top. Paths locate the ball center; length measures from tip to top.
pub struct BallEnd {
    radius: f32,
    length: f32,
}

impl BallEnd {
    pub fn new(radius: f64, length: f64) -> Option<Self> {
        let radius = read_radius(radius)?;
        let length = length as f32;
        (length.is_finite() && length >= 2.0 * radius).then_some(Self { radius, length })
    }

    pub fn sweep(&self, a: Point3, b: Point3) -> Result<Tree, InvalidPath> {
        let a = coordinate(a)?;
        let b = coordinate(b)?;
        let ball = capsule(a, b, self.radius)?;
        let d = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let p = [Tree::x() - a[0], Tree::y() - a[1], Tree::z() - a[2]];
        let height = self.length - self.radius;
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

#[cfg(test)]
mod tests;
