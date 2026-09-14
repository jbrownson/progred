use super::*;

#[derive(Clone)]
pub(super) struct Settings {
    progress: f64,
    radius: f64,
    length: f64,
    stock_min: Point3,
    stock_max: Point3,
}

impl Settings {
    pub(super) fn read(value: &Value) -> Option<Self> {
        let r = value.as_record()?;
        let progress = f64::read(r.get(&PROGRESS)?)?;
        let radius = f64::read(r.get(&TOOL_RADIUS)?)?;
        let length = f64::read(r.get(&TOOL_LENGTH)?)?;
        let stock_min = super::super::read_point(r.get(&STOCK_MIN)?)?;
        let stock_max = super::super::read_point(r.get(&STOCK_MAX)?)?;
        super::super::fidget::read_radius(radius)?;
        if !progress.is_finite()
            || !(0.0..=1.0).contains(&progress)
            || !length.is_finite()
            || length < 2.0 * radius
            || !(0..3).all(|i| stock_min[i] < stock_max[i])
        {
            return None;
        }
        coordinate(stock_min).ok()?;
        coordinate(stock_max).ok()?;
        Some(Self {
            progress,
            radius,
            length,
            stock_min,
            stock_max,
        })
    }

    pub(super) fn draw(
        &self,
        path: &Recording,
        tubes: &mut tubes::Tubes,
        line_radius: f64,
        completed_color: [u8; 3],
    ) -> Result<(), InvalidPath> {
        let center = path.playback(self.progress, |a, b, completed| {
            tubes.style(
                line_radius,
                if completed {
                    completed_color
                } else {
                    [145, 163, 178]
                },
            )?;
            tubes.start_at(a)?;
            tubes.line_to(b)
        })?;
        if let Some(center) = center {
            tubes.style(self.radius, [225, 94, 58])?;
            tubes.start_at(center)?;
            tubes.line_to([
                center[0],
                center[1],
                center[2] + self.length - 2.0 * self.radius,
            ])?;
        }
        tubes.style(line_radius * 0.6, [137, 150, 163])?;
        for corner in 0..8 {
            let a = std::array::from_fn(|i| {
                if corner & (1 << i) == 0 {
                    self.stock_min[i]
                } else {
                    self.stock_max[i]
                }
            });
            for axis in 0..3 {
                if corner & (1 << axis) == 0 {
                    let mut b = a;
                    b[axis] = self.stock_max[axis];
                    tubes.start_at(a)?;
                    tubes.line_to(b)?;
                }
            }
        }
        Ok(())
    }
}
