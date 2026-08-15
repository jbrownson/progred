//! The editor's assembled libraries, theme, and value projections.

use crate::conventions;
use crate::display::LineEdit;
use crate::projection;
use crate::styles::Styles;
use parley::style::GenericFamily;
use progred_graph::{Cells, Value};
use puri::edit::EditStyle;
use puri::text::TextStyle;
use vello::peniko::{Brush, Color};

pub fn library() -> Cells {
    conventions::library().merged(grap_geometry::library())
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    grap::ForeignFunctions::merge_all([
        conventions::foreign_functions(),
        grap_geometry::functions(),
    ])
}

pub fn values(value: &Value) -> Option<LineEdit> {
    projection::try_partials([conventions::text, conventions::f64], value)
}

pub fn styles(scale: f64) -> Styles {
    let style = |size: f32, color: [f32; 4], weight: Option<f32>| TextStyle {
        size,
        brush: Brush::from(Color::new(color)),
        weight,
        family: GenericFamily::SystemUi,
    };
    Styles {
        label: style(14.0, [0.46, 0.49, 0.55, 1.0], None),
        name: style(14.0, [0.13, 0.14, 0.16, 1.0], None),
        string: style(14.0, [0.55, 0.33, 0.28, 1.0], None),
        dim: style(13.0, [0.55, 0.58, 0.64, 1.0], None),
        id: TextStyle {
            family: GenericFamily::Monospace,
            ..style(13.0, [0.55, 0.58, 0.64, 1.0], None)
        },
        edit: EditStyle {
            selection: Brush::from(Color::new([0.0, 0.48, 1.0, 0.30])),
            cursor: Brush::from(Color::new([0.13, 0.14, 0.16, 1.0])),
        },
        scale,
    }
}
