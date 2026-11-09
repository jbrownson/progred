//! The fields cutters and stock are built from, in Fidget's `f32`
//! coordinates.

use crate::paths::{InvalidPath, Point3};
use fidget_engine::context::Tree;

/// A positive radius whose square `f32` can hold.
pub fn read_radius(radius: f64) -> Option<f32> {
    let radius = radius as f32;
    (radius > 0.0 && (radius * radius).is_finite() && radius * radius > 0.0).then_some(radius)
}

/// `point` in `f32`, unless a coordinate isn't finite or doesn't fit.
pub fn coordinate(point: Point3) -> Result<[f32; 3], InvalidPath> {
    if !point.into_iter().all(f64::is_finite) {
        return Err(InvalidPath::NonFinitePoint);
    }
    let point = point.map(|n| n as f32);
    point
        .into_iter()
        .all(f32::is_finite)
        .then_some(point)
        .ok_or(InvalidPath::CoordinateRange)
}

/// `v·(p - origin)` for the evaluated point `p`.
///
/// Fidget merges equal subexpressions. Written as `v·p - v·origin`, sweeps
/// along the same vector share only `v·p`. The equivalent `v·(p - origin)`
/// shares terms like `x - origin.x` between any sweeps with a coordinate in
/// common, however far apart; each stays live from its first use to its last,
/// and in the CAM example's stock that spilled tens of thousands of registers.
pub fn along(v: nalgebra::Vector3<f32>, origin: [f32; 3]) -> Tree {
    Tree::x() * v.x + Tree::y() * v.y + Tree::z() * v.z - v.dot(&nalgebra::Vector3::from(origin))
}

/// Distance-like field of a capsule around segment `a`-`b`. It stays in world
/// coordinates, where its interval bounds over the axis-aligned cells of
/// meshing and rendering are tightest, and offsets the closest point rather
/// than the evaluated point, sharing nothing with other segments (see `along`).
pub fn capsule(a: [f32; 3], b: [f32; 3], radius: f32) -> Result<Tree, InvalidPath> {
    let direction = nalgebra::Vector3::from(b) - nalgebra::Vector3::from(a);
    let length_squared = direction.norm_squared();
    if !length_squared.is_finite() {
        return Err(InvalidPath::CoordinateRange);
    }
    let t = if length_squared == 0.0 {
        Tree::constant(0.0)
    } else {
        (along(direction, a) / length_squared).max(0.0).min(1.0)
    };
    let axes = [Tree::x(), Tree::y(), Tree::z()];
    let q = std::array::from_fn::<_, 3, _>(|i| axes[i].clone() - (t.clone() * direction[i] + a[i]));
    Ok(q[0].square() + q[1].square() + q[2].square() - radius * radius)
}

#[cfg(test)]
#[path = "tests/field.rs"]
mod tests;
