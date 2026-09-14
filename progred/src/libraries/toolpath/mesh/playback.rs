use super::super::stock::{BallEnd, Stock};
use super::*;

#[derive(Clone, PartialEq)]
pub(super) struct Settings {
    progress: f64,
    radius: f64,
    length: f64,
    stock_min: Point3,
    stock_max: Point3,
    stock_color: Option<[u8; 3]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_lines_disappear_including_the_completed_part_of_a_segment() {
        let color = [20, 150, 230];
        let mut path = Recording::default();
        path.start_at([0.0; 3]).unwrap();
        path.line_to([2.0, 0.0, 0.0]).unwrap();
        for progress in [0.0, 0.25, 1.0] {
            let settings = Settings {
                progress,
                radius: 0.1,
                length: 0.5,
                stock_min: [-3.0; 3],
                stock_max: [3.0; 3],
                stock_color: None,
            };
            let mut tubes = tubes::Tubes::new(0.02, color).unwrap();
            settings.draw(&path, &mut tubes, 0.02, color).unwrap();
            let path_vertices: Vec<_> = tubes
                .geometry
                .vertices
                .iter()
                .filter(|v| v.color == color.map(|n| n as f32 / 255.0))
                .collect();
            if progress == 1.0 {
                assert!(path_vertices.is_empty());
            } else {
                assert!(!path_vertices.is_empty());
                let start = path_vertices
                    .iter()
                    .map(|v| v.position.x)
                    .fold(f32::INFINITY, f32::min);
                assert!((start - (2.0 * progress as f32 - 0.02)).abs() < 1e-6);
            }
        }
    }
}

impl Settings {
    pub(super) fn read(value: &Value) -> Option<Self> {
        let r = value.as_record()?;
        let progress = f64::read(r.get(&PROGRESS)?)?;
        let radius = f64::read(r.get(&TOOL_RADIUS)?)?;
        let length = f64::read(r.get(&TOOL_LENGTH)?)?;
        let stock_min = super::super::read_point(r.get(&STOCK_MIN)?)?;
        let stock_max = super::super::read_point(r.get(&STOCK_MAX)?)?;
        let stock_color = match r.get(&STOCK) {
            Some(value) => {
                let fields = value.as_record()?;
                Some(super::super::fidget::read_color(
                    fields.get(&fidget::vocabulary::COLOR)?,
                )?)
            }
            None => None,
        };
        BallEnd::new(radius, length)?;
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
            stock_color,
        })
    }

    pub(super) fn remaining_stock(
        &self,
        path: &Recording,
    ) -> Result<Option<fidget::SceneObject>, InvalidPath> {
        let Some(color) = self.stock_color else {
            return Ok(None);
        };
        let tool = BallEnd::new(self.radius, self.length).ok_or(InvalidPath::CoordinateRange)?;
        let mut stock =
            Stock::block(self.stock_min, self.stock_max).ok_or(InvalidPath::CoordinateRange)?;
        path.playback(self.progress, |a, b, completed| {
            if completed {
                stock.cut(&tool, a, b)?;
            }
            Ok(())
        })?;
        Ok(Some(fidget::SceneObject {
            tree: stock.into_field(),
            color,
        }))
    }

    pub(super) fn draw(
        &self,
        path: &Recording,
        tubes: &mut tubes::Tubes,
        line_radius: f64,
        path_color: [u8; 3],
    ) -> Result<(), InvalidPath> {
        let center = path.playback(
            self.progress,
            |a, b, completed| -> Result<(), InvalidPath> {
                if !completed {
                    tubes.style(line_radius, path_color)?;
                    tubes.start_at(a)?;
                    tubes.line_to(b)?;
                }
                Ok(())
            },
        )?;
        if let Some(center) = center {
            tubes.style(self.radius, [225, 94, 58])?;
            tubes.ball_end(center, self.length)?;
        }
        if self.stock_color.is_some() {
            return Ok(());
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
