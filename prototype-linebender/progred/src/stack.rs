//! The editor's assembled libraries and value projections.

use crate::conventions;
use crate::display::Language;
use crate::projection::{self, Projected};
use progred_graph::{Cells, Value};

pub fn library() -> Cells {
    conventions::library().merged(grap_geometry::library())
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    grap::ForeignFunctions::merge_all([
        conventions::foreign_functions(),
        grap_geometry::functions(),
    ])
}

pub fn values<D: Language>(
    display: &mut D,
    value: &Value,
) -> Option<Projected<D::View>> {
    projection::try_partials(
        [conventions::text, conventions::f64],
        display,
        value,
    )
}
