use super::{cutter::Tool, fidget::coordinate, paths::*, stock::Stock, vocabulary::*};
use crate::libraries::{f64, fidget};
use gid::Value;

pub(super) trait Draw: Sink<Error = InvalidPath> {
    fn style(&mut self, radius: f64, color: [u8; 3]) -> Result<(), InvalidPath>;
    fn tool(
        &mut self,
        tool: &Tool,
        pose: Pose,
        color: [u8; 3],
        tolerance: f64,
    ) -> Result<(), InvalidPath>;
}

#[derive(Clone, PartialEq)]
pub(super) struct Settings {
    progress: f64,
    tool: Tool,
    profile_tolerance: f64,
    stock_min: Point3,
    stock_max: Point3,
    stock_color: Option<[u8; 3]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_reads_the_tool_profile_not_independent_dimensions() {
        let tool = Tool::ball(0.125, 0.22).unwrap();
        let settings = |tool: Value| {
            Settings::read(&Value::record([
                (PROGRESS, f64::value(0.35)),
                (PROFILE_TOLERANCE, f64::value(0.001)),
                (super::super::cutter::vocabulary::TOOL, tool),
                (STOCK_MIN, super::super::point_value([-0.5; 3])),
                (STOCK_MAX, super::super::point_value([0.5; 3])),
            ]))
        };
        assert_eq!(settings(tool.value()).unwrap().tool, tool);
        assert!(settings(Value::record([])).is_none());
    }

    #[test]
    fn accuracy_belongs_to_playback_and_is_required_and_validated() {
        let tool = Tool::bull(0.125, 0.02, 0.22).unwrap();
        let mut fields = Value::record([
            (PROGRESS, f64::value(0.35)),
            (super::super::cutter::vocabulary::TOOL, tool.value()),
            (STOCK_MIN, super::super::point_value([-0.5; 3])),
            (STOCK_MAX, super::super::point_value([0.5; 3])),
        ])
        .as_record()
        .unwrap()
        .clone();
        assert!(Settings::read(&Value::Record(fields.clone())).is_none());
        for tolerance in [0.0, -0.1, f64::NAN, f64::INFINITY] {
            fields.insert(PROFILE_TOLERANCE, f64::value(tolerance));
            assert!(Settings::read(&Value::Record(fields.clone())).is_none());
        }
        fields.insert(PROFILE_TOLERANCE, f64::value(0.001));
        let fine = Settings::read(&Value::Record(fields.clone())).unwrap();
        fields.insert(PROFILE_TOLERANCE, f64::value(0.01));
        let coarse = Settings::read(&Value::Record(fields)).unwrap();
        assert_eq!(fine.tool, tool);
        assert_eq!(fine.tool, coarse.tool);
        assert!(
            fine != coarse,
            "changing accuracy must invalidate computation inputs"
        );
    }

    #[derive(Default)]
    struct Drawing {
        path: Recording,
        tool: Option<Point3>,
    }

    impl Sink for Drawing {
        type Error = InvalidPath;
        fn start_at(&mut self, point: Point3, axis: Axis) -> Result<(), InvalidPath> {
            self.path.start_at(point, axis)
        }
        fn line_to(&mut self, point: Point3) -> Result<(), InvalidPath> {
            self.path.line_to(point)
        }
    }

    impl Draw for Drawing {
        fn style(&mut self, _: f64, _: [u8; 3]) -> Result<(), InvalidPath> {
            Ok(())
        }
        fn tool(&mut self, _: &Tool, pose: Pose, _: [u8; 3], _: f64) -> Result<(), InvalidPath> {
            self.tool = Some(pose.tip);
            Ok(())
        }
    }

    #[test]
    fn completed_lines_disappear_including_the_completed_part_of_a_segment() {
        let color = [20, 150, 230];
        let mut path = Recording::default();
        path.start_at([0.0; 3], Axis::Z).unwrap();
        path.line_to([2.0, 0.0, 0.0]).unwrap();
        for progress in [0.0, 0.25, 1.0] {
            let settings = Settings {
                progress,
                tool: Tool::ball(0.2, 0.5).unwrap(),
                profile_tolerance: 0.001,
                stock_min: [-3.0; 3],
                stock_max: [3.0; 3],
                stock_color: Some([180; 3]),
            };
            let mut drawing = Drawing::default();
            settings.draw(&path, &mut drawing, 0.02, color).unwrap();
            let mut expected = Recording::default();
            if progress < 1.0 {
                expected
                    .start_at([2.0 * progress, 0.0, 0.0], Axis::Z)
                    .unwrap();
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
        let tool = Tool::read(r.get(&super::cutter::vocabulary::TOOL)?)?;
        let profile_tolerance = f64::read(r.get(&PROFILE_TOLERANCE)?)?;
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
        if !progress.is_finite()
            || !profile_tolerance.is_finite()
            || profile_tolerance <= 0.0
            || !(0.0..=1.0).contains(&progress)
            || !(0..3).all(|i| stock_min[i] < stock_max[i])
        {
            return None;
        }
        coordinate(stock_min).ok()?;
        coordinate(stock_max).ok()?;
        Some(Self {
            progress,
            tool,
            profile_tolerance,
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
        let mut stock =
            Stock::block(self.stock_min, self.stock_max).ok_or(InvalidPath::CoordinateRange)?;
        path.playback(self.progress, |a, b, axis, completed| {
            if completed {
                stock.cut(&self.tool, a, b, axis, self.profile_tolerance)?;
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
        let cursor = path.playback(
            self.progress,
            |a, b, axis, completed| -> Result<(), InvalidPath> {
                if !completed {
                    tubes.style(line_radius, path_color)?;
                    if end != Some(a) {
                        tubes.start_at(a, axis)?;
                    }
                    tubes.line_to(b)?;
                    end = Some(b);
                }
                Ok(())
            },
        )?;
        if let Some(pose) = cursor {
            tubes.tool(&self.tool, pose, [225, 94, 58], self.profile_tolerance)?;
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
                    tubes.start_at(a, Axis::Z)?;
                    tubes.line_to(b)?;
                }
            }
        }
        Ok(())
    }
}
