use super::{
    fidget::coordinate,
    paths::*,
    stock::{BallEnd, Stock},
    vocabulary::*,
};
use crate::libraries::{f64, fidget};
use gid::Value;

pub(super) trait Draw: Sink<Error = InvalidPath> {
    fn style(&mut self, radius: f64, color: [u8; 3]) -> Result<(), InvalidPath>;
    fn ball_end(&mut self, center: Point3, length: f64) -> Result<(), InvalidPath>;
}

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
    fn playback_accepts_an_eighth_inch_diameter_and_converts_it_to_radius() {
        let settings = |diameter| {
            Settings::read(&Value::record([
                (PROGRESS, f64::value(0.35)),
                (TOOL_DIAMETER, f64::value(diameter)),
                (TOOL_LENGTH, f64::value(0.22)),
                (STOCK_MIN, super::super::point_value([-0.5; 3])),
                (STOCK_MAX, super::super::point_value([0.5; 3])),
            ]))
        };
        assert_eq!(settings(0.125).unwrap().radius, 0.0625);
        for diameter in [0.0, -0.125, 0.25, f64::INFINITY, f64::NAN] {
            assert!(settings(diameter).is_none());
        }
    }

    #[derive(Default)]
    struct Drawing {
        path: Recording,
        tool: Option<Point3>,
    }

    impl Sink for Drawing {
        type Error = InvalidPath;
        fn start_at(&mut self, point: Point3) -> Result<(), InvalidPath> {
            self.path.start_at(point)
        }
        fn line_to(&mut self, point: Point3) -> Result<(), InvalidPath> {
            self.path.line_to(point)
        }
    }

    impl Draw for Drawing {
        fn style(&mut self, _: f64, _: [u8; 3]) -> Result<(), InvalidPath> {
            Ok(())
        }
        fn ball_end(&mut self, center: Point3, _: f64) -> Result<(), InvalidPath> {
            self.tool = Some(center);
            Ok(())
        }
    }

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
                stock_color: Some([180; 3]),
            };
            let mut drawing = Drawing::default();
            settings.draw(&path, &mut drawing, 0.02, color).unwrap();
            let mut expected = Recording::default();
            if progress < 1.0 {
                expected.start_at([2.0 * progress, 0.0, 0.0]).unwrap();
                expected.line_to([2.0, 0.0, 0.0]).unwrap();
            }
            assert!(drawing.path == expected);
            assert_eq!(drawing.tool, Some([2.0 * progress, 0.0, 0.0]));
        }
    }
}

impl Settings {
    pub(super) fn read(value: &Value) -> Option<Self> {
        let r = value.as_record()?;
        let progress = f64::read(r.get(&PROGRESS)?)?;
        let radius = f64::read(r.get(&TOOL_DIAMETER)?)? / 2.0;
        let length = f64::read(r.get(&TOOL_LENGTH)?)?;
        let stock_min = super::read_point(r.get(&STOCK_MIN)?)?;
        let stock_max = super::read_point(r.get(&STOCK_MAX)?)?;
        let stock_color = match r.get(&STOCK) {
            Some(value) => {
                let fields = value.as_record()?;
                Some(super::fidget::read_color(
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
        tubes: &mut impl Draw,
        line_radius: f64,
        path_color: [u8; 3],
    ) -> Result<(), InvalidPath> {
        let mut end = None;
        let center = path.playback(
            self.progress,
            |a, b, completed| -> Result<(), InvalidPath> {
                if !completed {
                    tubes.style(line_radius, path_color)?;
                    if end != Some(a) {
                        tubes.start_at(a)?;
                    }
                    tubes.line_to(b)?;
                    end = Some(b);
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
