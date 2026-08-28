//! Named faces the walk picks from. The editor fills them in.

use parley::style::GenericFamily;
use peniko::{Brush, Color};
use puri::edit::{EditStyle, LineEditPresentation};
use puri::text::TextStyle;

pub struct Styles {
    pub label: TextStyle,
    pub name: TextStyle,
    pub string: TextStyle,
    pub dim: TextStyle,
    pub id: TextStyle,
    pub accent_wash: TextStyle,
    pub selection_wash: Brush,
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
        selection_wash: Brush::from(Color::new([0.0, 0.38, 0.90, 0.50])),
        ink: style(14.0, [0.13, 0.14, 0.16, 1.0], None),
        edit: EditStyle {
            selection: Brush::from(Color::new([0.0, 0.48, 1.0, 0.30])),
            cursor: Brush::from(Color::new([0.13, 0.14, 0.16, 1.0])),
        },
        scale,
    }
}

impl Styles {
    pub fn line_presentation(&self, line: &progred_display::LineEdit) -> LineEditPresentation {
        LineEditPresentation::new(self.string.size, self.string.brush.clone())
            .with_family(line_family(line.family))
            .with_affixes(&line.prefix, &line.suffix)
    }

    pub fn line_style(&self, line: &progred_display::LineEdit) -> TextStyle {
        TextStyle {
            family: line_family(line.family),
            ..self.string.clone()
        }
    }
}

fn line_family(family: progred_display::TextFamily) -> GenericFamily {
    match family {
        progred_display::TextFamily::SystemUi => GenericFamily::SystemUi,
        progred_display::TextFamily::Monospace => GenericFamily::Monospace,
    }
}
