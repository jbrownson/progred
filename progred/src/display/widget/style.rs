//! Editor faces and highlight styling.

use kurbo::{Affine, Rect, RoundedRect, Stroke};
use parley::style::GenericFamily;
use peniko::{Brush, Color};
use puri::draw::Canvas;
use puri::edit::{EditStyle, LineEditPresentation};
use puri::text::TextStyle;

mod palette;
pub use palette::{Palette, Theme};

pub fn highlight_outline(scale: f64, rect: Rect) -> RoundedRect {
    RoundedRect::from_rect(rect.inflate(2.0 * scale, 2.0 * scale), 4.0 * scale)
}

/// The pane-local primary: a translucent accent wash ringed at full
/// strength, the strongest mark in the shared vocabulary.
pub fn primary_highlight<P: Canvas + ?Sized>(
    palette: Palette,
    scale: f64,
    canvas: &mut P,
    outline: RoundedRect,
) {
    canvas.fill(outline, palette.accent.with_alpha(0.22), Affine::IDENTITY);
    canvas.stroke(
        outline,
        primary_highlight_stroke(scale),
        palette.accent,
        Affine::IDENTITY,
    );
}

/// The primary highlight's ring, which what floats beside it clears.
pub fn primary_highlight_stroke(scale: f64) -> Stroke {
    Stroke::new(2.5 * scale)
}

/// A secondary copy of the selected or hovered value, such as another
/// reference to its cell; `strong` for the selection's copies.
pub fn secondary_highlight<P: Canvas + ?Sized>(
    palette: Palette,
    scale: f64,
    canvas: &mut P,
    outline: RoundedRect,
    strong: bool,
) {
    canvas.fill(
        outline,
        palette.accent.with_alpha(if strong { 0.10 } else { 0.05 }),
        Affine::IDENTITY,
    );
    if strong {
        canvas.stroke(
            outline,
            Stroke::new(1.5 * scale),
            palette.accent.with_alpha(0.55),
            Affine::IDENTITY,
        );
    }
}

/// What the pointer rests on: an accent [`outline_wash`].
pub fn hover_highlight<P: Canvas + ?Sized>(
    palette: Palette,
    scale: f64,
    canvas: &mut P,
    outline: RoundedRect,
) {
    outline_wash(palette.accent, scale, canvas, outline);
}

/// A faint wash of `color` inside a firmer ring of it.
pub fn outline_wash<P: Canvas + ?Sized>(
    color: Color,
    scale: f64,
    canvas: &mut P,
    outline: RoundedRect,
) {
    canvas.fill(outline, color.with_alpha(0.08), Affine::IDENTITY);
    canvas.stroke(
        outline,
        Stroke::new(1.5 * scale),
        color.with_alpha(0.55),
        Affine::IDENTITY,
    );
}

#[derive(Clone)]
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
        LineEditPresentation {
            family: line_family(line.family),
            prefix: line.prefix.to_string(),
            suffix: line.suffix.to_string(),
            ..LineEditPresentation::new(self.string.size, self.string.brush.clone())
        }
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

/// The brush `paint` stands for in `styles`.
pub fn brush(styles: &Styles, paint: crate::display::Paint) -> Brush {
    match paint {
        crate::display::Paint::Face(face) => face_style(styles, face).brush.clone(),
        crate::display::Paint::Brush(brush) => brush,
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
mod tests;
