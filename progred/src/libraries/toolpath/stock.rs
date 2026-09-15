//! Volumetric stock subtraction through ordinary Fidget expressions.

use super::{
    cutter::Tool,
    fidget::coordinate,
    paths::{Axis, InvalidPath, Point3},
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

    pub fn cut(
        &mut self,
        tool: &Tool,
        a: Point3,
        b: Point3,
        axis: Axis,
        tolerance: f64,
    ) -> Result<(), InvalidPath> {
        if let Some(sweep) = tool.sweep(a, b, axis, tolerance)? {
            self.field = self.field.max(-sweep);
        }
        Ok(())
    }

    pub fn into_field(self) -> Tree {
        self.field
    }
}

#[cfg(test)]
mod tests;
