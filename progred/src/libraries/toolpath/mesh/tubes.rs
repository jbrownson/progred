use super::*;

const SIDES: usize = 12;
const RINGS: usize = 6;
const VERTICES: usize = SIDES * RINGS + 2;

pub(super) struct Tubes {
    pub(super) geometry: Geometry,
    radius: f32,
    color: [f32; 3],
    previous: Option<Vector3<f32>>,
    circle: [(f32, f32); SIDES],
}

impl Tubes {
    pub(super) fn style(&mut self, radius: f64, color: [u8; 3]) -> Result<(), InvalidPath> {
        self.radius =
            super::super::fidget::read_radius(radius).ok_or(InvalidPath::CoordinateRange)?;
        self.color = color.map(|v| f32::from(v) / 255.0);
        Ok(())
    }

    pub(super) fn new(radius: f64, color: [u8; 3]) -> Option<Self> {
        Some(Self {
            geometry: Geometry::default(),
            radius: super::super::fidget::read_radius(radius)?,
            color: color.map(|v| f32::from(v) / 255.0),
            previous: None,
            circle: std::array::from_fn(|i| {
                (std::f32::consts::TAU * i as f32 / SIDES as f32).sin_cos()
            }),
        })
    }

    pub(super) fn ball_end(&mut self, center: Point3, length: f64) -> Result<(), InvalidPath> {
        let center = Vector3::from(coordinate(center)?);
        let height = length as f32 - self.radius;
        if !height.is_finite() || height < self.radius {
            return Err(InvalidPath::CoordinateRange);
        }
        let vertices: [Vector3<f32>; SIDES * 4 + 2] = std::array::from_fn(|i| match i {
            0 => center - Vector3::z() * self.radius,
            i if i == SIDES * 4 + 1 => center + Vector3::z() * height,
            i => {
                let ring = (i - 1) / SIDES;
                let (z, radius) = if ring == 3 {
                    (height, self.radius)
                } else {
                    let (sin, cos) = ((ring as f32 - 2.0) * std::f32::consts::FRAC_PI_6).sin_cos();
                    (self.radius * sin, self.radius * cos)
                };
                let (sin, cos) = self.circle[(i - 1) % SIDES];
                center + Vector3::new(radius * cos, radius * sin, z)
            }
        });
        self.append(vertices)
    }

    fn segment(&mut self, a: Vector3<f32>, b: Vector3<f32>) -> Result<(), InvalidPath> {
        let direction = b.cast::<f64>() - a.cast::<f64>();
        let axis = direction
            .try_normalize(0.0)
            .map(|v| v.cast::<f32>())
            .unwrap_or(Vector3::z());
        let helper = if axis.x.abs() < 0.9 {
            Vector3::x()
        } else {
            Vector3::y()
        };
        let u = axis.cross(&helper).normalize();
        let v = axis.cross(&u);
        let vertices: [Vector3<f32>; VERTICES] = std::array::from_fn(|i| match i {
            0 => a - axis * self.radius,
            i if i == VERTICES - 1 => b + axis * self.radius,
            i => {
                let ring = (i - 1) / SIDES;
                let angle =
                    (ring as f32 - if ring < 3 { 2.0 } else { 3.0 }) * std::f32::consts::FRAC_PI_6;
                let (along, across) = angle.sin_cos();
                let (sin, cos) = self.circle[(i - 1) % SIDES];
                let center = if ring < 3 { a } else { b };
                center + self.radius * (axis * along + across * (u * cos + v * sin))
            }
        });
        self.append(vertices)
    }

    fn append<const N: usize>(&mut self, vertices: [Vector3<f32>; N]) -> Result<(), InvalidPath> {
        let rings = (N - 2) / SIDES;
        if !vertices.iter().all(|p| p.iter().all(|v| v.is_finite())) {
            return Err(InvalidPath::CoordinateRange);
        }
        let offset = u32::try_from(self.geometry.vertices.len())
            .ok()
            .filter(|n| n.checked_add(N as u32).is_some())
            .ok_or(InvalidPath::CoordinateRange)?;
        self.geometry
            .vertices
            .extend(vertices.map(|position| Vertex {
                position,
                color: self.color,
            }));
        let mut triangle = |a, b, c| {
            self.geometry
                .indices
                .extend([a, b, c].map(|i| offset + i as u32))
        };
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            triangle(0, 1 + next, 1 + side);
            for ring in 0..rings - 1 {
                let a = 1 + ring * SIDES + side;
                let b = 1 + ring * SIDES + next;
                let c = a + SIDES;
                let d = b + SIDES;
                triangle(a, b, c);
                triangle(b, d, c);
            }
            triangle(
                1 + (rings - 1) * SIDES + side,
                1 + (rings - 1) * SIDES + next,
                N - 1,
            );
        }
        Ok(())
    }
}

impl Sink for Tubes {
    type Error = InvalidPath;

    fn start_at(&mut self, point: Point3) -> Result<(), Self::Error> {
        self.previous = Some(Vector3::from(coordinate(point)?));
        Ok(())
    }

    fn line_to(&mut self, point: Point3) -> Result<(), Self::Error> {
        let previous = self.previous.ok_or(InvalidPath::MissingStart)?;
        let point = Vector3::from(coordinate(point)?);
        self.segment(previous, point)?;
        self.previous = Some(point);
        Ok(())
    }
}

impl playback::Draw for Tubes {
    fn style(&mut self, radius: f64, color: [u8; 3]) -> Result<(), InvalidPath> {
        self.style(radius, color)
    }

    fn ball_end(&mut self, center: Point3, length: f64) -> Result<(), InvalidPath> {
        self.ball_end(center, length)
    }
}
