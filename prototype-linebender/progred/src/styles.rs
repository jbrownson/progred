//! Named faces the walk picks from. The editor fills them in.

use parley::style::GenericFamily;
use puri::edit::{EditStyle, LineEditPresentation};
use puri::text::TextStyle;
use peniko::{Brush, Color};

pub struct Styles {
    pub label: TextStyle,
    pub name: TextStyle,
    pub string: TextStyle,
    pub dim: TextStyle,
    pub id: TextStyle,
    pub accent_wash: TextStyle,
    pub ink: TextStyle,
    pub edit: EditStyle,
    pub scale: f64,
}

pub fn editor(scale: f64) -> Styles {
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
        accent_wash: style(14.0, [0.0, 0.48, 1.0, 0.30], None),
        ink: style(14.0, [0.13, 0.14, 0.16, 1.0], None),
        edit: EditStyle {
            selection: Brush::from(Color::new([0.0, 0.48, 1.0, 0.30])),
            cursor: Brush::from(Color::new([0.13, 0.14, 0.16, 1.0])),
        },
        scale,
    }
}

impl Styles {
    pub fn line_presentation(&self, prefix: &str, suffix: &str) -> LineEditPresentation {
        LineEditPresentation::new(self.string.size, self.string.brush.clone())
            .with_affixes(prefix, suffix)
    }
}
