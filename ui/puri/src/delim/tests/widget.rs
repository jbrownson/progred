use super::*;
use crate::draw::{DrawCmd, DrawList, Shape};
use kurbo::Rect;
use kurbo::Shape as _;
use peniko::Color;

#[test]
fn stretched_delimiters_fill_their_advance_and_contain_their_ink() {
    for scale in [1.0, 2.0] {
        for (ascent, descent) in [(0.0, 0.0), (20.0, 10.0), (80.0, 80.0)] {
            for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
                for side in [Side::Open, Side::Close] {
                    let width = advance(delim, 14.0 * scale);
                    let (min_ascent, min_descent) = minimum_span(14.0 * scale);
                    let rect = Rect::new(
                        20.0,
                        100.0,
                        20.0 + width,
                        100.0
                            + (ascent * scale).max(min_ascent)
                            + (descent * scale).max(min_descent),
                    );
                    let mut recording = DrawList::new();
                    draw_stretched(
                        delim,
                        side,
                        14.0 * scale,
                        ascent * scale,
                        descent * scale,
                        Color::BLACK.into(),
                        &mut recording,
                        Affine::translate((20.0, 100.0)),
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
