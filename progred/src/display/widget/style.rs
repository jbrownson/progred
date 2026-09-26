//! Editor faces and highlight styling.

use peniko::{Brush, Color};
use puri::draw::Canvas;
use puri::edit::{EditStyle, LineEditPresentation};
use puri::text::GenericFamily;
use puri::text::TextStyle;
use puri::{Affine, Rect, RoundedRect, Stroke};

mod palette;
pub use palette::{Palette, Theme};

pub fn highlight_outline(scale: f64, rect: Rect) -> RoundedRect {
    RoundedRect::from_rect(rect.inflate(2.0 * scale, 2.0 * scale), 4.0 * scale)
}

pub fn hover_highlight<P: Canvas + ?Sized>(
    palette: Palette,
    scale: f64,
    canvas: &mut P,
    outline: RoundedRect,
) {
    canvas.fill(outline, palette.accent.with_alpha(0.08), Affine::IDENTITY);
    canvas.stroke(
        outline,
        Stroke::new(1.5 * scale),
        palette.accent.with_alpha(0.55),
        Affine::IDENTITY,
    );
}

pub struct Styles {
    pub palette: Palette,
    pub label: TextStyle,
    pub name: TextStyle,
    pub string: TextStyle,
    pub dim: TextStyle,
    pub detail: TextStyle,
    pub id: TextStyle,
    pub accent_wash: TextStyle,
    pub selection_wash: Brush,
    pub ink: TextStyle,
    pub delimiter: Brush,
    pub edit: EditStyle,
    pub scale: f64,
}

pub fn editor(palette: Palette, scale: f64) -> Styles {
    let style = |size: f32, color: Color, weight: Option<f32>| TextStyle {
        size,
        brush: Brush::from(color),
        weight,
        family: GenericFamily::SystemUi,
    };
    let ink = palette.ink;
    let muted = palette.muted;
    Styles {
        palette,
        label: style(14.0, palette.label, None),
        name: style(14.0, palette.name, None),
        string: style(14.0, palette.literal, None),
        dim: style(13.0, muted, None),
        detail: style(11.0, muted, None),
        id: TextStyle {
            family: GenericFamily::Monospace,
            ..style(13.0, muted, None)
        },
        accent_wash: style(14.0, palette.accent.with_alpha(0.30), None),
        selection_wash: Brush::from(palette.accent.with_alpha(0.50)),
        ink: style(14.0, ink, None),
        delimiter: Brush::from(palette.delimiter),
        edit: EditStyle {
            selection: Brush::from(palette.accent.with_alpha(0.30)),
            cursor: Brush::from(ink),
        },
        scale,
    }
}

impl Styles {
    pub fn line_presentation(&self, line: &crate::display::LineEdit) -> LineEditPresentation {
        LineEditPresentation::new(self.string.size, self.string.brush.clone())
            .with_family(line_family(line.family))
            .with_affixes(&line.prefix, &line.suffix)
    }

    pub fn line_style(&self, line: &crate::display::LineEdit) -> TextStyle {
        TextStyle {
            family: line_family(line.family),
            ..self.string.clone()
        }
    }
}

fn line_family(family: crate::display::TextFamily) -> GenericFamily {
    match family {
        crate::display::TextFamily::SystemUi => GenericFamily::SystemUi,
        crate::display::TextFamily::Monospace => GenericFamily::Monospace,
    }
}

pub fn face_style(styles: &Styles, face: crate::display::Face) -> &TextStyle {
    match face {
        crate::display::Face::Name => &styles.name,
        crate::display::Face::String => &styles.string,
        crate::display::Face::Dim => &styles.dim,
        crate::display::Face::Label => &styles.label,
        crate::display::Face::Id => &styles.id,
        crate::display::Face::AccentWash => &styles.accent_wash,
        crate::display::Face::Ink => &styles.ink,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_remains_legible_on_document_and_readonly_grounds() {
        let luminance = |color: Color| {
            color.components[..3]
                .iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(&channel, weight)| {
                    weight
                        * if channel <= 0.04045 {
                            channel / 12.92
                        } else {
                            ((channel + 0.055) / 1.055).powf(2.4)
                        }
                })
                .sum::<f32>()
        };
        for theme in [Theme::Light, Theme::Dark] {
            let palette = theme.palette();
            let paper = palette.paper;
            let wash = palette.readonly_ground;
            let library = Color::new(std::array::from_fn(|i| {
                if i == 3 {
                    1.0
                } else {
                    wash.components[i] * wash.components[3]
                        + paper.components[i] * (1.0 - wash.components[3])
                }
            }));
            let styles = editor(palette, 1.0);
            for face in [
                &styles.name,
                &styles.label,
                &styles.string,
                &styles.dim,
                &styles.detail,
                &styles.id,
                &styles.ink,
            ] {
                let Brush::Solid(ink) = face.brush else {
                    panic!("solid text color")
                };
                for ground in [paper, library, palette.panel] {
                    let a = luminance(ground);
                    let b = luminance(ink);
                    let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(contrast >= 4.5, "{ink:?} on {ground:?}: {contrast}");
                }
            }
        }
    }
}
