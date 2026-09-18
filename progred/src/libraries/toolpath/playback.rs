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
    cursor: Cursor,
    focus: Option<std::ops::Range<usize>>,
    profile_tolerance: f64,
    stock_min: Point3,
    stock_max: Point3,
    stock_color: Option<[u8; 3]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accuracy_belongs_to_playback_and_is_required_and_validated() {
        let mut fields = Value::record([
            (PROGRESS, f64::value(0.35)),
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
        assert!(
            fine != coarse,
            "changing accuracy must invalidate computation inputs"
        );
    }

    #[derive(Default)]
    struct Drawing {
        path: Recording,
        tool: Option<(Tool, Pose)>,
    }

    impl Sink for Drawing {
        type Error = InvalidPath;
        fn end_path(&mut self) {
            self.path.end_path();
        }
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
        fn tool(&mut self, tool: &Tool, pose: Pose, _: [u8; 3], _: f64) -> Result<(), InvalidPath> {
            self.tool = Some((tool.clone(), pose));
            Ok(())
        }
    }

    #[test]
    fn completed_lines_disappear_including_the_completed_part_of_a_segment() {
        let color = [20, 150, 230];
        let mut path = Recording::default();
        path.enter_tool(&Tool::ball(0.2, 0.5).unwrap());
        path.start_at([0.0; 3], Axis::Z).unwrap();
        path.line_to([2.0, 0.0, 0.0]).unwrap();
        path.leave_tool();
        for progress in [0.0, 0.25, 1.0] {
            let settings = Settings {
                cursor: Cursor::Progress(progress),
                focus: None,
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
            assert_eq!(drawing.tool.unwrap().1.tip, [2.0 * progress, 0.0, 0.0]);
        }
    }

    fn settings(progress: f64) -> Settings {
        Settings {
            cursor: Cursor::Progress(progress),
            focus: None,
            profile_tolerance: 0.001,
            stock_min: [-1.0, -1.0, -0.5],
            stock_max: [1.0, 1.0, 0.5],
            stock_color: Some([180; 3]),
        }
    }

    #[test]
    fn playback_switches_tools_and_subtracts_both_from_one_stock() {
        use fidget_engine::{shape::EzShape, vm::VmShape};
        let small = Tool::square(0.2, 0.4).unwrap();
        let large = Tool::square(0.4, 0.4).unwrap();
        let mut path = Recording::default();
        for (x, tool) in [(-0.5, &small), (0.5, &large)] {
            with_tool(&mut path, tool, |path| {
                path.start_at([x, -0.3, 0.0], Axis::Z).unwrap();
                path.line_to([x, 0.3, 0.0]).unwrap();
            });
        }
        let samples = |progress| {
            let object = settings(progress).remaining_stock(&path).unwrap().unwrap();
            let shape = VmShape::from(object.tree);
            let mut evaluator = VmShape::new_float_slice_eval();
            evaluator
                .eval(
                    &shape.ez_float_slice_tape(),
                    &[-0.5, -0.36, 0.5, 0.64],
                    &[0.0; 4],
                    &[0.2; 4],
                )
                .unwrap()
                .to_vec()
        };
        assert!(samples(0.0).iter().all(|v| *v < 0.0));
        let halfway = samples(0.5);
        assert_eq!(
            halfway.iter().map(|v| *v > 0.0).collect::<Vec<_>>(),
            [true, false, false, false]
        );
        assert_eq!(
            samples(1.0).iter().map(|v| *v > 0.0).collect::<Vec<_>>(),
            [true, false, true, true]
        );
        assert_eq!(samples(0.5), halfway);
        for (progress, tool) in [(0.25, &small), (0.5, &small), (0.75, &large)] {
            let mut drawing = Drawing::default();
            settings(progress)
                .draw(&path, &mut drawing, 0.01, [20, 150, 230])
                .unwrap();
            assert_eq!(&drawing.tool.unwrap().0, tool);
        }
    }

    #[test]
    fn untooled_paths_can_be_drawn_but_cannot_remove_stock() {
        let mut path = Recording::default();
        path.start_at([0.0; 3], Axis::Z).unwrap();
        path.line_to([0.5, 0.0, 0.0]).unwrap();
        let mut drawing = Drawing::default();
        settings(0.5)
            .draw(&path, &mut drawing, 0.01, [20, 150, 230])
            .unwrap();
        assert!(drawing.tool.is_none());
        assert_eq!(drawing.path.segments().count(), 1);
        for progress in [0.0, 0.5, 1.0] {
            assert!(matches!(
                settings(progress).remaining_stock(&path),
                Err(InvalidPath::MissingTool)
            ));
        }
    }

    #[test]
    fn focused_stock_contains_earlier_cuts_but_not_later_ones() {
        use fidget_engine::{shape::EzShape, vm::VmShape};
        let mut program = Recording::default();
        for x in [-0.6, 0.0, 0.6] {
            let mut part = Recording::default();
            with_tool(&mut part, &Tool::square(0.2, 0.4).unwrap(), |part| {
                part.start_at([x, -0.3, 0.0], Axis::Z).unwrap();
                part.line_to([x, 0.3, 0.0]).unwrap();
            });
            program.append_part(part);
        }
        for (progress, expected) in [(0.0, [true, false, false]), (1.0, [true, true, false])] {
            let mut playback = settings(progress);
            playback.focus = Some(1..2);
            let object = playback.remaining_stock(&program).unwrap().unwrap();
            let shape = VmShape::from(object.tree);
            let mut evaluator = VmShape::new_float_slice_eval();
            let samples = evaluator
                .eval(
                    &shape.ez_float_slice_tape(),
                    &[-0.6, 0.0, 0.6],
                    &[0.0; 3],
                    &[0.2; 3],
                )
                .unwrap();
            assert_eq!(
                samples.iter().map(|v| *v > 0.0).collect::<Vec<_>>(),
                expected
            );
            let mut drawing = Drawing::default();
            playback
                .draw(&program, &mut drawing, 0.01, [200; 3])
                .unwrap();
            assert_eq!(drawing.tool.unwrap().1.tip[0], 0.0);
            assert!(
                drawing
                    .path
                    .segments()
                    .all(|(a, b, _)| a[0] == 0.0 && b[0] == 0.0)
            );
        }
    }
}

impl Settings {
    pub(super) fn read(value: &Value) -> Option<Self> {
        let r = value.as_record()?;
        let cursor = match r.get(&crate::libraries::controls::vocabulary::POSITION) {
            Some(value) => {
                Cursor::Position(f64::read(value).filter(|p| p.is_finite() && *p >= 0.0)?)
            }
            None => {
                Cursor::Progress(f64::read(r.get(&PROGRESS)?).filter(|p| (0.0..=1.0).contains(p))?)
            }
        };
        let focus = match r.get(&FOCUS) {
            Some(value) => Some(crate::libraries::list::index_range(value)?),
            None => None,
        };
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
        if !profile_tolerance.is_finite()
            || profile_tolerance <= 0.0
            || !(0..3).all(|i| stock_min[i] < stock_max[i])
        {
            return None;
        }
        coordinate(stock_min).ok()?;
        coordinate(stock_max).ok()?;
        Some(Self {
            cursor,
            focus,
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
        path.playback_cursor(
            self.cursor,
            self.focus.clone(),
            |a, b, axis, tool, completed| {
                let tool = tool.ok_or(InvalidPath::MissingTool)?;
                if completed {
                    stock.cut(tool, a, b, axis, self.profile_tolerance)?;
                }
                Ok(())
            },
        )?;
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
        let cursor = path.playback_cursor(
            self.cursor,
            self.focus.clone(),
            |a, b, axis, _, completed| -> Result<(), InvalidPath> {
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
        if let Some((pose, Some(tool))) = cursor {
            tubes.tool(tool, pose, [225, 94, 58], self.profile_tolerance)?;
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
