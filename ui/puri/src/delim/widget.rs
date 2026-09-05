use super::{Delim, DelimStyle, MAX_GROWTH, close_with_width, open_with_width};
use crate::{Affine, Command, Drawing, Shape};

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

/// Reserve this width when the final vertical span is not yet known.
pub fn maximum_advance(delim: Delim, text_size: f64) -> f64 {
    DelimStyle::for_text_size(text_size).bow(delim) * MAX_GROWTH + 2.0 * SIDE_BEARING_EM * text_size
}

/// A delimiter spanning the caller's ascent/descent, with its bearings
/// included in the returned metrics. All inputs use the same units.
pub fn stretched<Paint>(
    delim: Delim,
    side: Side,
    text_size: f64,
    ascent: f64,
    descent: f64,
    paint: Paint,
) -> Drawing<Paint> {
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
    let bow = style.bow_for(delim, ascent + descent);
    let path = match side {
        Side::Open => open_with_width(delim, &style, top, bottom, bow),
        Side::Close => close_with_width(delim, &style, top, bottom, bow),
    };
    Drawing {
        width: bow + 2.0 * bearing,
        ascent,
        descent,
        commands: vec![Command::Fill {
            shape: Shape::Path(path),
            paint,
            transform: Affine::translate((bearing, ascent)),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, DrawCmd, DrawList, Rect};
    use kurbo::Shape as _;

    #[test]
    fn stretched_families_have_equal_bounded_widths_and_contain_their_ink() {
        for scale in [1.0, 2.0] {
            for (ascent, descent) in [(0.0, 0.0), (20.0, 10.0), (80.0, 80.0)] {
                let mut expected_width = None;
                for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
                    for side in [Side::Open, Side::Close] {
                        let drawing = stretched(
                            delim,
                            side,
                            14.0 * scale,
                            ascent * scale,
                            descent * scale,
                            Color::BLACK,
                        );
                        let rect = Rect::new(
                            20.0,
                            100.0,
                            20.0 + drawing.width,
                            100.0 + drawing.ascent + drawing.descent,
                        );
                        let width = *expected_width.get_or_insert(drawing.width);
                        assert!((drawing.width - width).abs() < 1e-6);
                        assert!(drawing.width <= maximum_advance(delim, 14.0 * scale));
                        let mut recording = DrawList::new();
                        crate::draw::draw(
                            drawing,
                            &mut recording,
                            Affine::translate((20.0, 100.0)),
                            |color| (*color).into(),
                        );
                        let [
                            DrawCmd::Fill {
                                shape: Shape::Path(path),
                                transform,
                                ..
                            },
                        ] = recording.0.as_slice()
                        else {
                            panic!("one filled delimiter outline");
                        };
                        let mut path = path.clone();
                        path.apply_affine(*transform);
                        let ink = path.bounding_box();
                        let bearing = SIDE_BEARING_EM * 14.0 * scale;
                        assert!((ink.x0 - (rect.x0 + bearing)).abs() < 1e-6);
                        assert!((ink.x1 - (rect.x1 - bearing)).abs() < 1e-6);
                        assert!(ink.y0 >= rect.y0 - 1e-6 && ink.y1 <= rect.y1 + 1e-6);
                        if matches!(delim, Delim::Bracket) {
                            assert!((ink.y0 - rect.y0).abs() < 1e-6);
                            assert!((ink.y1 - rect.y1).abs() < 1e-6);
                        }
                    }
                }
            }
        }
    }
}
