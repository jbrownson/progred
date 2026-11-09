//! Volumetric stock subtraction through ordinary Fidget expressions.

use crate::{
    cutter::Tool,
    field::coordinate,
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
            // Fidget numbers a max's right operand first and orders
            // commutative operands by number. With the stock on the right,
            // each sweep is evaluated just before the max that removes it;
            // the other way round, every sweep is evaluated before the first
            // max, and all of them stay live at once.
            self.field = (-sweep).max(self.field.clone());
        }
        Ok(())
    }

    pub fn into_field(self) -> Tree {
        self.field
    }
}

#[cfg(test)]
#[path = "tests/stock.rs"]
mod tests;
