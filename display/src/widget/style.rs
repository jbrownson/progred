//! Named faces the walk picks from. The editor fills them in.

use peniko::{Brush, Color};
use puri::edit::{EditStyle, LineEditPresentation};
use puri::text::GenericFamily;
use puri::text::TextStyle;
use puri::{Rect, RoundedRect};

pub fn highlight_outline(scale: f64, rect: Rect) -> RoundedRect {
    RoundedRect::from_rect(rect.inflate(2.0 * scale, 2.0 * scale), 4.0 * scale)
}

pub fn hover_wash() -> Color {
    Color::new([0.0, 0.48, 1.0, 0.08])
}

pub struct Styles {
    pub label: TextStyle,
    pub name: TextStyle,
    pub string: TextStyle,
    pub dim: TextStyle,
    pub detail: TextStyle,
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
        detail: style(11.0, [0.55, 0.58, 0.64, 1.0], None),
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
    pub fn line_presentation(&self, line: &crate::LineEdit) -> LineEditPresentation {
        LineEditPresentation::new(self.string.size, self.string.brush.clone())
            .with_family(line_family(line.family))
            .with_affixes(&line.prefix, &line.suffix)
    }

    pub fn line_style(&self, line: &crate::LineEdit) -> TextStyle {
        TextStyle {
            family: line_family(line.family),
            ..self.string.clone()
        }
    }
}

fn line_family(family: crate::TextFamily) -> GenericFamily {
    match family {
        crate::TextFamily::SystemUi => GenericFamily::SystemUi,
        crate::TextFamily::Monospace => GenericFamily::Monospace,
    }
}

pub fn face_style(styles: &Styles, face: crate::Face) -> &TextStyle {
    match face {
        crate::Face::Name => &styles.name,
        crate::Face::String => &styles.string,
        crate::Face::Dim => &styles.dim,
        crate::Face::Label => &styles.label,
        crate::Face::Id => &styles.id,
        crate::Face::AccentWash => &styles.accent_wash,
        crate::Face::Ink => &styles.ink,
    }
}
