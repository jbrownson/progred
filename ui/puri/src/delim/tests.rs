use super::*;

const STYLE: DelimStyle = DelimStyle {
    stem: 1.0,
    bow: 4.0,
    brace_bow: 4.0,
    line: 40.0,
};

#[test]
fn a_braces_halves_fill_as_one_where_they_meet() {
    for height in [14.0, 40.0, 120.0] {
        let (_, tip) = STYLE.weights(height);
        let width = STYLE.bow_for(Delim::Brace, height);
        // Just inside each point, where both halves' bands overlap.
        for (path, x) in [
            (open(Delim::Brace, &STYLE, 0.0, height), tip / 4.0),
            (close(Delim::Brace, &STYLE, 0.0, height), width - tip / 4.0),
        ] {
            let point = Point::new(x, height / 2.0);
            assert_ne!(
                path.winding(point),
                0,
                "a brace {height} tall has a gap at its point"
            );
        }
    }
}

#[test]
fn curved_strokes_thin_to_their_tips() {
    let height = 40.0;
    let (belly, tip) = STYLE.weights(height);
    assert!(tip < belly / 2.0);
    let inside = |path: &BezPath, x: f64, y: f64| path.winding(Point::new(x, y)) != 0;
    // A paren is `belly` wide across its waist and `tip` wide where its
    // ends are cut.
    let paren = open(Delim::Paren, &STYLE, 0.0, height);
    assert!(inside(&paren, belly * 0.9, height / 2.0));
    assert!(!inside(&paren, belly * 1.1, height / 2.0));
    let bow = STYLE.bow_for(Delim::Paren, height);
    assert!(inside(&paren, bow - tip * 0.5, 0.001));
    assert!(!inside(&paren, bow - tip * 1.5, 0.001));
    // A brace's hooks thin to `tip` where they end.
    let brace = open(Delim::Brace, &STYLE, 0.0, height);
    let end = STYLE.bow_for(Delim::Brace, height) - tip / 2.0;
    assert!(inside(&brace, end, tip * 0.9));
    assert!(!inside(&brace, end, tip * 1.1));
}

#[test]
fn hobby_curves_turn_a_quarter_circle_like_metafont() {
    // The best cubic quarter circle: control points 4/3 tan(π/8) out.
    let (leave, arrive) = hobby(
        Point::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Point::new(0.0, 1.0),
        Vec2::new(-1.0, 0.0),
    );
    let out = 4.0 / 3.0 * (PI / 8.0).tan();
    assert!((leave - Point::new(1.0, out)).hypot() < 1e-9);
    assert!((arrive - Point::new(out, 1.0)).hypot() < 1e-9);
}

fn ink(path: &BezPath) -> Rect {
    path.bounding_box()
}

fn assert_close(a: Rect, b: Rect, context: &str) {
    let close = (a.x0 - b.x0).abs() < 1e-6
        && (a.y0 - b.y0).abs() < 1e-6
        && (a.x1 - b.x1).abs() < 1e-6
        && (a.y1 - b.y1).abs() < 1e-6;
    assert!(close, "{context}: {a} vs {b}");
}

#[test]
fn ink_fills_the_requested_box() {
    for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
        assert_close(
            ink(&open(delim, &STYLE, 0.0, 40.0)),
            Rect::new(0.0, 0.0, 4.0, 40.0),
            &format!("{delim:?}"),
        );
    }
}

#[test]
fn close_mirrors_open() {
    for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
        assert_close(
            ink(&close(delim, &STYLE, 0.0, 40.0)),
            ink(&open(delim, &STYLE, 0.0, 40.0)),
            &format!("{delim:?}"),
        );
    }
}

#[test]
fn tall_delimiters_grow_wider_up_to_the_cap() {
    for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
        let at = |height: f64| ink(&open(delim, &STYLE, 0.0, height)).width();
        assert_eq!(at(40.0), 4.0, "{delim:?} at one line");
        assert_eq!(at(160.0), 8.0, "{delim:?} at the cap");
        assert_eq!(at(4000.0), 8.0, "{delim:?} beyond the cap");
        assert_close(
            ink(&close(delim, &STYLE, 0.0, 160.0)),
            ink(&open(delim, &STYLE, 0.0, 160.0)),
            &format!("{delim:?} grown mirror"),
        );
    }
}

#[test]
fn stock_braces_are_wider_than_parens_and_brackets() {
    let style = DelimStyle::for_text_size(14.0);
    for height in [style.line, style.line * 2.0, style.line * 4.0] {
        let [paren, bracket, brace] =
            [Delim::Paren, Delim::Bracket, Delim::Brace].map(|delim| style.bow_for(delim, height));
        assert_eq!(paren, bracket);
        assert!(
            brace > paren,
            "a brace {height} tall is no wider than a paren"
        );
    }
}

#[test]
fn fitted_delimiters_keep_their_base_width_at_any_height() {
    for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
        for height in [40.0, 160.0, 4_000.0] {
            assert_eq!(
                ink(&open_fitted(delim, &STYLE, 0.0, height)).width(),
                STYLE.bow(delim),
            );
            assert_eq!(
                ink(&close_fitted(delim, &STYLE, 0.0, height)).width(),
                STYLE.bow(delim),
            );
        }
    }
}

#[test]
fn outlines_are_closed() {
    for delim in [Delim::Paren, Delim::Bracket, Delim::Brace] {
        let closed = open(delim, &STYLE, 0.0, 40.0)
            .elements()
            .iter()
            .filter(|el| matches!(el, kurbo::PathEl::ClosePath))
            .count();
        assert!(closed >= 1, "{delim:?} has no closed subpath");
    }
}
