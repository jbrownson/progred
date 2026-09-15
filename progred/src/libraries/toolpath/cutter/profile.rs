//! Revolved, filled-to-axis profiles. Each section is connected; shoulders are
//! explicit radial steps within it. Axial coordinates start at the tip.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub radius: f64,
    pub axial: f64,
}

impl Point {
    pub fn new(radius: f64, axial: f64) -> Self {
        Self { radius, axial }
    }
    pub(super) fn valid(self) -> bool {
        self.radius >= 0.0
            && self.radius.is_finite()
            && self.axial.is_finite()
            && (self.radius == 0.0 || read_radius(self.radius).is_some())
            && (self.axial as f32).is_finite()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Line(Point),
    /// Change radius at the preceding endpoint's axial position.
    Shoulder(f64),
    Arc {
        end: Point,
        radius: f64,
        bend: Bend,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bend {
    Convex,
    Concave,
}

/// Geometric facts about an arc, independent of any sampling accuracy.
struct ArcGeometry {
    center: Point,
    radius: f64,
    angle: f64,
    delta: f64,
}

impl ArcGeometry {
    fn new(start: Point, end: Point, radius: f64, bend: Bend) -> Option<Self> {
        if !start.valid() || !end.valid() || !radius.is_finite() || radius <= 0.0 {
            return None;
        }
        let dr = end.radius - start.radius;
        let dz = end.axial - start.axial;
        let chord = dr.hypot(dz);
        let half = chord / 2.0;
        if chord == 0.0 || !chord.is_finite() || half > radius {
            return None;
        }
        let sign = match bend {
            Bend::Convex => 1.0,
            Bend::Concave => -1.0,
        };
        // The minor-arc center lies on the chosen side of the chord. Factoring
        // the difference of squares keeps near-semicircles well conditioned
        // as far as their inherently sensitive center calculation allows.
        let height = (radius - half).sqrt() * (radius + half).sqrt();
        let center = Point::new(
            start.radius + dr / 2.0 - sign * dz / chord * height,
            start.axial + dz / 2.0 + sign * dr / chord * height,
        );
        if !center.radius.is_finite() || !center.axial.is_finite() {
            return None;
        }
        let angle = (start.axial - center.axial).atan2(start.radius - center.radius);
        let delta = sign * 2.0 * (half / radius).asin();
        if delta == 0.0 {
            return None;
        }
        // A single-valued radius at each axial position, checked on the arc
        // itself, not a polygonal approximation of it.
        for t in [0.0, 0.5, 1.0] {
            if delta.signum() * (angle + delta * t).cos() < -1e-12 {
                return None;
            }
        }
        let arc = Self {
            center,
            radius,
            angle,
            delta,
        };
        if arc.contains_angle(std::f64::consts::PI) && center.radius < radius {
            return None;
        }
        Some(arc)
    }

    fn contains_angle(&self, angle: f64) -> bool {
        let low = self.angle.min(self.angle + self.delta);
        let high = self.angle.max(self.angle + self.delta);
        let next = angle + ((low - angle) / std::f64::consts::TAU).ceil() * std::f64::consts::TAU;
        next <= high
    }
}

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub radius: f64,
    pub min_axial: f64,
    pub max_axial: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub kind: SectionKind,
    pub start: Point,
    pub profile: Vec<Segment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    Cutting,
    NonCutting,
}

/// Polygonal outline plus the explicit radial edges. Keeping their indices
/// avoids rediscovering shoulders by comparing sampled axial coordinates.
struct Sampled {
    points: Vec<Point>,
    shoulders: Vec<usize>,
}

impl Section {
    pub fn taper(start: Point, end: Point, kind: SectionKind) -> Self {
        Self {
            kind,
            start,
            profile: vec![Segment::Line(end)],
        }
    }

    pub(super) fn bounds(&self) -> Option<Bounds> {
        if !self.start.valid() || self.profile.is_empty() {
            return None;
        }
        let mut start = self.start;
        let mut radius = start.radius;
        for (i, segment) in self.profile.iter().enumerate() {
            let end = match *segment {
                Segment::Line(end) => end,
                Segment::Shoulder(r) => {
                    // An end cap already closes each end. Interior shoulders
                    // join two advancing edges; repeated radial edges would
                    // introduce redundant or backtracking contour pieces.
                    if i == 0
                        || i + 1 == self.profile.len()
                        || matches!(self.profile[i + 1], Segment::Shoulder(_))
                    {
                        return None;
                    }
                    let end = Point::new(r, start.axial);
                    if !end.valid() {
                        return None;
                    }
                    radius = radius.max(r);
                    start = end;
                    continue;
                }
                Segment::Arc {
                    end,
                    radius: r,
                    bend,
                } => {
                    let arc = ArcGeometry::new(start, end, r, bend)?;
                    if arc.contains_angle(0.0) {
                        radius = radius.max(arc.center.radius + arc.radius);
                    }
                    end
                }
            };
            if !end.valid() || end.axial as f32 <= start.axial as f32 {
                return None;
            }
            radius = radius.max(end.radius);
            start = end;
        }
        read_radius(radius)?;
        Some(Bounds {
            radius,
            min_axial: self.start.axial,
            max_axial: start.axial,
        })
    }

    /// Sample only the circular profile edges. Motion is never sampled.
    /// The tolerance bounds chord error in the tool's radius/axial plane.
    pub fn outline(&self, tolerance: f64) -> Option<Vec<Point>> {
        Some(self.sample(tolerance)?.points)
    }

    fn sample(&self, tolerance: f64) -> Option<Sampled> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return None;
        }
        self.bounds()?;
        let mut points = vec![self.start];
        let mut shoulders = Vec::new();
        for segment in &self.profile {
            let start = *points.last()?;
            let previous = points.len() - 1;
            match *segment {
                Segment::Line(end) => {
                    points.push(end);
                }
                Segment::Shoulder(radius) => {
                    shoulders.push(previous);
                    points.push(Point::new(radius, start.axial));
                }
                Segment::Arc { end, radius, bend } => {
                    let ArcGeometry {
                        radius: r,
                        center,
                        angle,
                        delta,
                    } = ArcGeometry::new(start, end, radius, bend)?;
                    let step = 2.0 * (1.0 - (tolerance / r).min(1.0)).acos();
                    let count = (delta.abs() / step).ceil();
                    if !count.is_finite() || count > 4096.0 {
                        return None;
                    }
                    for i in 1..count as usize {
                        let (sin, cos) = (angle + delta * i as f64 / count).sin_cos();
                        points.push(Point::new(center.radius + r * cos, center.axial + r * sin));
                    }
                    points.push(end);
                }
            }
            if points.len() > 8192 {
                return None;
            }
            if !matches!(segment, Segment::Shoulder(_))
                && !points[previous..]
                    .windows(2)
                    .all(|p| p[1].axial as f32 > p[0].axial as f32)
            {
                return None;
            }
        }
        if !points.iter().all(|p| p.valid()) {
            return None;
        }
        Some(Sampled { points, shoulders })
    }

    pub(crate) fn sweep(
        &self,
        tolerance: f64,
        a: Point3,
        b: Point3,
        axis: Axis,
    ) -> Result<Tree, InvalidPath> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(InvalidPath::CoordinateRange);
        }
        // Exact lowering of the familiar hemisphere followed by a cylinder.
        // This inspects the actual profile, never its name or constructor.
        if let [
            Segment::Arc {
                end,
                radius,
                bend: Bend::Convex,
            },
            Segment::Line(top),
        ] = self.profile.as_slice()
        {
            let r = end.radius;
            if self.start.radius == 0.0
                && *radius == r
                && end.axial == self.start.axial + r
                && top.radius == r
                && top.axial - self.start.axial >= 2.0 * r
            {
                let shift =
                    |p: Point3| std::array::from_fn(|i| p[i] + axis.vector()[i] * end.axial);
                return BallEnd::new(r, top.axial - self.start.axial)
                    .ok_or(InvalidPath::CoordinateRange)?
                    .sweep(shift(a), shift(b), axis);
            }
        }
        let Sampled { points, shoulders } =
            self.sample(tolerance).ok_or(InvalidPath::CoordinateRange)?;
        let mut field: Option<Tree> = None;
        let mut start = 0;
        for end in shoulders
            .iter()
            .copied()
            .chain(std::iter::once(points.len() - 1))
        {
            let next = sweep_outline(&points[start..=end], a, b, axis)?;
            field = Some(match field {
                Some(f) => f.min(next),
                None => next,
            });
            start = end + 1;
        }
        let mut field = field.ok_or(InvalidPath::CoordinateRange)?;
        for i in shoulders {
            let radius = points[i].radius.min(points[i + 1].radius);
            if radius == 0.0 {
                continue;
            }
            // A contained radial tent joins the interiors on both sides of
            // the shoulder. Its radius is <= each neighboring linear band at
            // both endpoints, hence throughout that band. Sweeping preserves
            // that containment: no extra material or epsilon overlap is added.
            // This removes the shared cap disk, leaving the exposed annulus.
            let connector = [
                Point::new(0.0, points[i - 1].axial),
                Point::new(radius, points[i].axial),
                Point::new(0.0, points[i + 2].axial),
            ];
            field = field.min(sweep_outline(&connector, a, b, axis)?);
        }
        Ok(field)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub sections: Vec<Section>,
}

impl Tool {
    pub fn new(sections: Vec<Section>) -> Option<Self> {
        if sections.is_empty() {
            return None;
        }
        let bounds = sections
            .iter()
            .map(Section::bounds)
            .collect::<Option<Vec<_>>>()?;
        for (i, section) in sections.iter().enumerate() {
            for j in 0..i {
                if section.kind == sections[j].kind
                    && bounds[i].min_axial as f32 <= bounds[j].max_axial as f32
                    && bounds[j].min_axial as f32 <= bounds[i].max_axial as f32
                {
                    // Touching pieces belong in one profile with a shared
                    // endpoint or shoulder, not independently capped solids.
                    return None;
                }
            }
        }
        Some(Self { sections })
    }

    pub fn square(diameter: f64, length: f64) -> Option<Self> {
        if diameter <= 0.0 {
            return None;
        }
        Self::new(vec![Section::taper(
            Point::new(diameter / 2.0, 0.0),
            Point::new(diameter / 2.0, length),
            SectionKind::Cutting,
        )])
    }

    pub fn ball(diameter: f64, length: f64) -> Option<Self> {
        Self::bull(diameter, diameter / 2.0, length)
    }

    pub fn bull(diameter: f64, corner: f64, length: f64) -> Option<Self> {
        let r = diameter / 2.0;
        if !corner.is_finite() || corner <= 0.0 || corner > r || length <= corner {
            return None;
        }
        Self::new(vec![Section {
            kind: SectionKind::Cutting,
            start: Point::new(r - corner, 0.0),
            profile: vec![
                Segment::Arc {
                    end: Point::new(r, corner),
                    radius: corner,
                    bend: Bend::Convex,
                },
                Segment::Line(Point::new(r, length)),
            ],
        }])
    }

    pub fn sweep(
        &self,
        a: Point3,
        b: Point3,
        axis: Axis,
        tolerance: f64,
    ) -> Result<Option<Tree>, InvalidPath> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(InvalidPath::CoordinateRange);
        }
        let mut field: Option<Tree> = None;
        for section in self
            .sections
            .iter()
            .filter(|s| s.kind == SectionKind::Cutting)
        {
            let next = section.sweep(tolerance, a, b, axis)?;
            field = Some(match field {
                Some(f) => f.min(next),
                None => next,
            });
        }
        Ok(field)
    }
}

/// Sweep a continuous-radius polyline, capping only its two outer ends.
fn sweep_outline(points: &[Point], a: Point3, b: Point3, axis: Axis) -> Result<Tree, InvalidPath> {
    let mut field: Option<Tree> = None;
    for pair in points.windows(2) {
        let next = frustum(pair[0], pair[1], a, b, axis)?;
        field = Some(match field {
            Some(f) => f.min(next),
            None => next,
        });
    }
    let field = field.ok_or(InvalidPath::CoordinateRange)?;
    let a = coordinate(a)?;
    let b = coordinate(b)?;
    let z = axis.basis()[2];
    let along = (Tree::x() - a[0]) * z.x + (Tree::y() - a[1]) * z.y + (Tree::z() - a[2]) * z.z;
    let dz = z.dot(&(nalgebra::Vector3::from(b) - nalgebra::Vector3::from(a)));
    Ok(field
        .max(points[0].axial as f32 + dz.min(0.0) - along.clone())
        .max(along - (points.last().unwrap().axial as f32 + dz.max(0.0))))
}

/// Exact swept solid of one finite linear radius/axial band. At each query Z,
/// restrict time to where this band is present; then minimize its quadratic
/// radial inequality over that interval. Concave quadratics minimize at an end.
fn frustum(
    start: Point,
    end: Point,
    a: Point3,
    b: Point3,
    axis: Axis,
) -> Result<Tree, InvalidPath> {
    let a = coordinate(a)?;
    let b = coordinate(b)?;
    let basis = axis.basis();
    let direction = nalgebra::Vector3::from(b) - nalgebra::Vector3::from(a);
    let d = basis.map(|v| v.dot(&direction));
    let z0 = start.axial as f32;
    let z1 = end.axial as f32;
    let r0 = start.radius as f32;
    let k = (end.radius as f32 - r0) / (z1 - z0);
    let aa = d[0] * d[0] + d[1] * d[1] - (k * d[2]).powi(2);
    if z1 <= z0 || !d.into_iter().chain([k, aa, r0 * r0]).all(f32::is_finite) {
        return Err(InvalidPath::CoordinateRange);
    }
    let world = [Tree::x() - a[0], Tree::y() - a[1], Tree::z() - a[2]];
    let p = basis.map(|v| world[0].clone() * v.x + world[1].clone() * v.y + world[2].clone() * v.z);
    let radius = r0 + k * (p[2].clone() - z0);
    let (lo, hi) = if d[2] == 0.0 {
        (Tree::constant(0.0), Tree::constant(1.0))
    } else {
        let t0 = (p[2].clone() - z0) / d[2];
        let t1 = (p[2].clone() - z1) / d[2];
        (
            t0.clone().min(t1.clone()).max(0.0).min(1.0),
            t0.max(t1).max(0.0).min(1.0),
        )
    };
    let at = |t: Tree| {
        (p[0].clone() - t.clone() * d[0]).square() + (p[1].clone() - t.clone() * d[1]).square()
            - (radius.clone() - t * (k * d[2])).square()
    };
    let radial = if aa > 0.0 {
        let t = ((p[0].clone() * d[0] + p[1].clone() * d[1] - radius.clone() * (k * d[2])) / aa)
            .max(lo)
            .min(hi);
        at(t)
    } else {
        at(lo).min(at(hi))
    };
    let outside = (z0 + d[2].min(0.0) - p[2].clone()).max(p[2].clone() - (z1 + d[2].max(0.0)));
    // Axial band bounds select which radial inequality is applicable; they
    // aren't internal cap surfaces. Capping each band would leave zero sheets
    // inside the cutter where consecutive profile pieces meet. Only the whole
    // continuous outline receives end caps in sweep_outline.
    let absent = outside.compare(0.0).max(0.0);
    Ok(absent.and(outside).or(absent.not().and(radial)))
}
