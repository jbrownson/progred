//! The editor's loaded libraries, theme, and assembled offerings.

use crate::display::LineEdit;
use crate::library::{self, Library};
use crate::projection;
use crate::styles::Styles;
use parley::style::GenericFamily;
use progred_display::{Env, Layout};
use progred_graph::{Cells, Value};
use puri::edit::EditStyle;
use puri::text::TextStyle;
use vello::peniko::{Brush, Color};

pub fn libraries() -> Vec<Library> {
    vec![
        library::conventions(),
        library::grap(),
        library::absent(),
        library::control(),
        library::f64(),
        library::geometry(),
    ]
}

fn loaded() -> Library {
    library::merge(libraries())
}

pub fn library() -> Cells {
    loaded().cells
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    loaded().functions
}

pub fn project(env: &dyn Env, value: &Value) -> Option<Layout> {
    projection::try_partials(&loaded().projections, env, value)
}

/// The line a projection would mount for `value`, if the whole
/// layout is an editable line. Keyboard landings use this; a click
/// already has the line on the event.
pub fn line(value: &Value) -> Option<LineEdit> {
    project(&NoEval, value).and_then(|layout| progred_display::line_edit_of(&layout).cloned())
}

struct NoEval;

impl Env for NoEval {
    fn evaluate(&self, _: &Value) -> (Value, usize) {
        (Value::record([]), 0)
    }
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
