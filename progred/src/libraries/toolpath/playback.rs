use super::{cutter::Tool, paths::*, stock::Stock, vocabulary::*};
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

impl Settings {
    pub(super) fn stock_color(&self) -> Option<[u8; 3]> {
        self.stock_color
    }

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
                Some(crate::libraries::color::read_opaque(
                    fields.get(&fidget::vocabulary::COLOR)?,
                )?)
            }
            None => None,
        };
        // The bounds are valid when they make the block that's cut.
        (profile_tolerance.is_finite()
            && profile_tolerance > 0.0
            && Stock::block(stock_min, stock_max).is_some())
        .then_some(Self {
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

#[cfg(test)]
mod tests;
