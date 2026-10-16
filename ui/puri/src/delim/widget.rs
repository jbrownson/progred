use super::{Delim, DelimStyle, close_with_width, open_with_width};
use crate::{Affine, Brush, Canvas, draw::CanvasSink};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Open,
    Close,
}

// Calibrated against the system font by delimiter_bench.
const GLYPH_ASC_EM: f64 = 0.704;
const GLYPH_DESC_EM: f64 = 0.171;
const TOP_TRIM_EM: f64 = 0.929 - GLYPH_ASC_EM;
const BOTTOM_TRIM_EM: f64 = 0.249 - GLYPH_DESC_EM;
const SIDE_BEARING_EM: f64 = 0.05;

pub fn advance(delim: Delim, text_size: f64) -> f64 {
    DelimStyle::for_text_size(text_size).bow(delim) + 2.0 * SIDE_BEARING_EM * text_size
}

pub fn minimum_span(text_size: f64) -> (f64, f64) {
    (GLYPH_ASC_EM * text_size, GLYPH_DESC_EM * text_size)
}

/// Paint a delimiter spanning the caller's ascent/descent. The transform's
/// origin is the top-left of its box, including the side bearings.
pub fn draw_stretched(
    delim: Delim,
    side: Side,
    text_size: f64,
    ascent: f64,
    descent: f64,
    brush: Brush,
    canvas: &mut (impl CanvasSink + ?Sized),
    transform: Affine,
) {
    let style = DelimStyle::for_text_size(text_size);
    let ascent = ascent.max(GLYPH_ASC_EM * text_size);
    let descent = descent.max(GLYPH_DESC_EM * text_size);
    let (top, bottom) = match delim {
        Delim::Bracket => (-ascent, descent),
        Delim::Paren | Delim::Brace => (
            -(ascent - TOP_TRIM_EM * text_size).max(GLYPH_ASC_EM * text_size),
            (descent - BOTTOM_TRIM_EM * text_size).max(GLYPH_DESC_EM * text_size),
        ),
    };
    let bearing = SIDE_BEARING_EM * text_size;
    let bow = style.bow(delim);
    let path = match side {
        Side::Open => open_with_width(delim, &style, top, bottom, bow),
        Side::Close => close_with_width(delim, &style, top, bottom, bow),
    };
    canvas.fill(
        path,
        brush,
        transform * Affine::translate((bearing, ascent)),
    );
}

#[cfg(test)]
mod tests;
