//! Stretched delimiters as filled outlines, the way math fonts draw
//! them, after Knuth's Computer Modern: the paren one arc over the FULL
//! span, its ends leaving at a slant and cut flat; the brace hooked at
//! its tips and pointed at its middle with straight arms between; the
//! bracket square. The curved strokes are MODULATED as Latin Modern's
//! are, thick at the paren's belly and the brace's arms and thinning to
//! `TIP` of that at the paren's ends, the brace's hook tips and its
//! point. A tall delimiter grows wider and gains weight (TeX's \big
//! through \Bigg grow in both dimensions); the one-line form is drawn
//! the same way at glyph height and the base weight. Ink spans exactly
//! x in [0, bow-for-height] and y in [top, bottom].

mod widget;
pub use widget::{Side, advance, draw_stretched, minimum_span};

use kurbo::{Affine, Arc, BezPath, Point, Rect, Shape, Vec2};
use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Paren,
    Bracket,
    Brace,
}

/// Width ramps with the square root of the height in lines, capped
/// here; contrast (belly weight over the base stem) ramps the same
/// way to its own cap. The contrast cap is deliberately modest: the
/// surrounding text is a near-monoline UI face, and the mathy
/// quality comes from the modulation being PRESENT, not from
/// absolute weight — a display-grade 2.5x read heavy against it.
pub const MAX_GROWTH: f64 = 2.0;
pub const MAX_CONTRAST: f64 = 1.6;

/// How thin a curved stroke's ends are, as a fraction of its belly:
/// Latin Modern's parens and brace hooks keep about this contrast at
/// every size.
const TIP: f64 = 0.4;

/// The steepest a paren's outer edge leaves its ends, below the
/// horizontal. Latin Modern's leave at 38° to 49° at every size, because
/// its tall parens widen; ours keep their width, so Knuth's rule alone
/// would leave a tall paren's ends nearly vertical and the paren nearly
/// straight.
const END: f64 = 45.0 * PI / 180.0;

/// Parens and brackets share a 0.30 em base width, wide enough to keep
/// their curvature and arms legible and deliberately more regular
/// than the system font's differently proportioned glyphs. Braces take
/// half as much again, LaTeX's proportion to its parens, so their hooks
/// read at text size. All span -0.704..+0.171 em around the baseline
/// with 0.05 em side bearings; `line` is the one-line reference height
/// the growth and contrast ramps measure against.
#[derive(Debug, Clone, Copy)]
pub struct DelimStyle {
    pub stem: f64,
    pub bow: f64,
    pub brace_bow: f64,
    pub line: f64,
}

impl DelimStyle {
    pub fn for_text_size(size: f64) -> Self {
        Self {
            stem: size * 0.075,
            bow: size * 0.30,
            brace_bow: size * 0.45,
            line: size * 1.18,
        }
    }

    pub fn bow(&self, delim: Delim) -> f64 {
        match delim {
            Delim::Paren | Delim::Bracket => self.bow,
            Delim::Brace => self.brace_bow,
        }
    }

    /// The ink width at `height`: the base bow grown by the height
    /// ramp.
    pub fn bow_for(&self, delim: Delim, height: f64) -> f64 {
        self.bow(delim) * (height / self.line).sqrt().clamp(1.0, MAX_GROWTH)
    }

    /// The modulated weights at `height`: the belly thickens with the
    /// contrast ramp, and the ends thin to [`TIP`] of it.
    fn weights(&self, height: f64) -> (f64, f64) {
        let belly = self.stem * (height / self.line).sqrt().clamp(1.0, MAX_CONTRAST);
        (belly, belly * TIP)
    }
}

pub fn open(delim: Delim, style: &DelimStyle, top: f64, bottom: f64) -> BezPath {
    open_with_width(
        delim,
        style,
        top,
        bottom,
        style.bow_for(delim, bottom - top),
    )
}

/// The same height-sensitive stroke as [`open`], constrained to the
/// delimiter's base advance. Useful when a layout leaf promises that
/// its rectangle contains all of its ink.
pub fn open_fitted(delim: Delim, style: &DelimStyle, top: f64, bottom: f64) -> BezPath {
    open_with_width(delim, style, top, bottom, style.bow(delim))
}

/// Draw a delimiter at an explicitly allocated width. This separates
/// vertical trimming from width growth when a layout box, rather than
/// the ink span within it, determines the width.
pub fn open_with_width(
    delim: Delim,
    style: &DelimStyle,
    top: f64,
    bottom: f64,
    bow: f64,
) -> BezPath {
    let bottom = bottom.max(top + style.stem);
    let (belly, tip) = style.weights(bottom - top);
    match delim {
        Delim::Paren => paren(bow, top, bottom, belly, tip),
        Delim::Bracket => bracket(bow, top, bottom, belly, tip),
        Delim::Brace => brace(bow, top, bottom, belly, tip),
    }
}

pub fn close(delim: Delim, style: &DelimStyle, top: f64, bottom: f64) -> BezPath {
    let width = style.bow_for(delim, bottom.max(top + style.stem) - top);
    close_with_width(delim, style, top, bottom, width)
}

/// Mirrored [`open_fitted`].
pub fn close_fitted(delim: Delim, style: &DelimStyle, top: f64, bottom: f64) -> BezPath {
    close_with_width(delim, style, top, bottom, style.bow(delim))
}

/// Mirrored [`open_with_width`].
pub fn close_with_width(
    delim: Delim,
    style: &DelimStyle,
    top: f64,
    bottom: f64,
    width: f64,
) -> BezPath {
    let mut path = open_with_width(delim, style, top, bottom, width);
    path.apply_affine(Affine::new([-1.0, 0.0, 0.0, 1.0, width, 0.0]));
    path
}

/// One crescent as Knuth draws it: each edge a Metafont curve from one
/// end through the waist to the other, and the ends cut flat. Knuth
/// leaves each end along its edge's chord to the waist with the run
/// tripled; a tall paren stretches the run further, so that its outer
/// edge leaves no steeper than [`END`]. The inner edge, with its shorter
/// run, leaves steeper, so the band is `belly` wide at the waist and
/// thins to `tip` wide at the ends.
fn paren(bow: f64, top: f64, bottom: f64, belly: f64, tip: f64) -> BezPath {
    let mid = (top + bottom) / 2.0;
    let half = (bottom - top) / 2.0;
    let stretch = (half / ((bow - tip).max(f64::EPSILON) * END.tan())).max(3.0);
    let flip = |point: Point| Point::new(point.x, 2.0 * mid - point.y);
    let down = Vec2::new(0.0, 1.0);
    let (outer, outer_waist) = (Point::new(bow - tip, top), Point::new(0.0, mid));
    let (inner, inner_waist) = (Point::new(bow, top), Point::new(belly, mid));
    let (o1, o2) = hobby(
        outer,
        Vec2::new(-stretch * (bow - tip), half),
        outer_waist,
        down,
    );
    let (i1, i2) = hobby(
        inner,
        Vec2::new(stretch * (belly - bow), half),
        inner_waist,
        down,
    );
    let mut path = BezPath::new();
    path.move_to(outer);
    path.curve_to(o1, o2, outer_waist);
    path.curve_to(flip(o2), flip(o1), flip(outer));
    path.line_to(flip(inner));
    path.curve_to(flip(i1), flip(i2), inner_waist);
    path.curve_to(i2, i1, inner);
    path.close_path();
    path
}

/// The control points Metafont gives `from{leave}...{arrive}to`: John
/// Hobby's curve at tension one, kept within the triangle its tangents
/// make so that it never inflects.
fn hobby(from: Point, leave: Vec2, to: Point, arrive: Vec2) -> (Point, Point) {
    let chord = to - from;
    let turn = |angle: f64| (angle + PI).rem_euclid(2.0 * PI) - PI;
    let theta = turn(leave.atan2() - chord.atan2());
    let phi = turn(chord.atan2() - arrive.atan2());
    let (st, ct, sf, cf) = (theta.sin(), theta.cos(), phi.sin(), phi.cos());
    let root5 = 5f64.sqrt();
    let velocity = |st: f64, ct: f64, sf: f64, cf: f64| {
        let num = 2.0 + SQRT_2 * (st - sf / 16.0) * (sf - st / 16.0) * (ct - cf);
        let den = 3.0 * (1.0 + 0.5 * (root5 - 1.0) * ct + 0.5 * (3.0 - root5) * cf);
        (num / den).min(4.0)
    };
    let mut leaving = velocity(st, ct, sf, cf);
    let mut arriving = velocity(sf, cf, st, ct);
    let sine = st.abs() * cf + sf.abs() * ct;
    if st * sf >= 0.0 && sine > 0.0 {
        leaving = leaving.min(sf.abs() / sine);
        arriving = arriving.min(st.abs() / sine);
    }
    let rotate = |v: Vec2, angle: f64| {
        let (sin, cos) = angle.sin_cos();
        Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    };
    (
        from + rotate(chord, theta) * leaving,
        to - rotate(chord, -phi) * arriving,
    )
}

/// A square bracket is monoline: its horizontal arms track the
/// height-sensitive weight of its vertical stem. Its square corners
/// read heavier than curves at equal weight, so lighten both together.
fn bracket(bow: f64, top: f64, bottom: f64, belly: f64, _tip: f64) -> BezPath {
    let weight = belly * 0.8;
    let mut path = Rect::new(0.0, top, weight.min(bow), bottom).to_path(0.05);
    path.extend(Rect::new(0.0, top, bow, top + weight).path_elements(0.05));
    path.extend(Rect::new(0.0, bottom - weight, bow, bottom).path_elements(0.05));
    path
}

/// The brace as math fonts draw it: each half a band that runs straight
/// down its arm, curls in a tight hook at its tip, thinning as it goes, and
/// ends there in a small round cap; toward the middle it thins again as it
/// curves into the point, where the halves meet at a short face. The band
/// is `belly` thick along the arm and `tip` thick at the hook's end and the
/// point. The hooks and the point keep to the brace's width, so they grow
/// with it. The lower half is the upper one mirrored and reversed, so both
/// wind the same way and fill as one where they overlap.
fn brace(bow: f64, top: f64, bottom: f64, belly: f64, tip: f64) -> BezPath {
    let weight = belly.min(bow / 2.0);
    let half_weight = weight / 2.0;
    let mid = (top + bottom) / 2.0;
    // Where the hook ends, inset so its round cap stays in the box.
    let end = bow - tip / 2.0;
    let arm = bow / 2.0;
    let reach = (mid - top - half_weight).max(0.0);
    // A quarter of the way from tip to point each, as in Knuth's big
    // braces, up to the brace's width.
    let hook = (reach / 4.0)
        .clamp(HOOK * bow, bow)
        .min(reach * 0.45)
        .max(half_weight);
    let point = (reach / 4.0)
        .clamp(POINT * bow, bow)
        .min(reach * 0.55)
        .max(tip);
    let hook_y = top + half_weight + hook;
    let hook_radii = Vec2::new(end - arm + half_weight, hook + half_weight);
    let mut path = BezPath::new();
    for sign in [1.0, -1.0] {
        let y = |y: f64| mid + sign * (y - mid);
        let hook_center = (end, y(hook_y));
        let point_center = (0.0, y(mid - point));
        let mut half = BezPath::new();
        half.move_to((end, y(top)));
        arc_dir(
            &mut half,
            hook_center,
            hook_radii,
            -FRAC_PI_2,
            -FRAC_PI_2,
            sign,
        );
        half.line_to((arm - half_weight, y(mid - point)));
        arc_dir(
            &mut half,
            point_center,
            Vec2::new(arm - half_weight, point - tip / 2.0),
            0.0,
            FRAC_PI_2,
            sign,
        );
        half.line_to((0.0, y(mid + tip / 2.0)));
        arc_dir(
            &mut half,
            point_center,
            Vec2::new(arm + half_weight, point + tip / 2.0),
            FRAC_PI_2,
            -FRAC_PI_2,
            sign,
        );
        half.line_to((arm + half_weight, y(hook_y)));
        tapered(
            &mut half,
            hook_center,
            hook_radii,
            (weight, tip),
            sign * PI,
            sign * FRAC_PI_2,
        );
        arc_dir(
            &mut half,
            (end, y(top + tip / 2.0)),
            Vec2::new(tip / 2.0, tip / 2.0),
            FRAC_PI_2,
            -PI,
            sign,
        );
        half.close_path();
        path.extend(if sign < 0.0 {
            half.reverse_subpaths()
        } else {
            half
        });
    }
    path
}

/// The least height of the brace's hook and point curves, as fractions
/// of its width; a short brace keeps these.
const HOOK: f64 = 0.32;
const POINT: f64 = 0.38;

fn arc(path: &mut BezPath, center: (f64, f64), radii: Vec2, start: f64, sweep: f64) {
    path.extend(
        Arc::new(Point::new(center.0, center.1), radii, start, sweep, 0.0).append_iter(0.05),
    );
}

/// The inner edge of a band whose outer edge is the ellipse at `center`
/// with `radii`, from angle `start` through `sweep`. It is drawn in by
/// `thick` where the ellipse runs vertically and by `thin` where it runs
/// horizontally, the horizontal inset falling off with the square of the
/// angle's cosine, so the band thins toward its ends the way a math font's
/// strokes do. Cubic Hermite segments, four to a quarter turn, follow it to
/// well within a thousandth of an em.
fn tapered(
    path: &mut BezPath,
    center: (f64, f64),
    radii: Vec2,
    (thick, thin): (f64, f64),
    start: f64,
    sweep: f64,
) {
    let reach = Vec2::new((radii.x - thin).max(0.0), (radii.y - thin).max(0.0));
    let taper = thick - thin;
    let at = |angle: f64| {
        let (sin, cos) = angle.sin_cos();
        (
            Point::new(
                center.0 + reach.x * cos - taper * cos.powi(3),
                center.1 + reach.y * sin,
            ),
            Vec2::new(-sin * (reach.x - 3.0 * taper * cos * cos), reach.y * cos),
        )
    };
    let segments = (sweep.abs() / FRAC_PI_2 * 4.0).ceil().max(1.0);
    let step = sweep / segments;
    let (mut from, mut from_slope) = at(start);
    for index in 1..=segments as usize {
        let (to, to_slope) = at(start + step * index as f64);
        path.curve_to(
            from + from_slope * (step / 3.0),
            to - to_slope * (step / 3.0),
            to,
        );
        (from, from_slope) = (to, to_slope);
    }
}

/// An arc mirrored vertically about its center when `sign` is
/// negative — the lower brace half reuses the upper half's angles.
fn arc_dir(path: &mut BezPath, center: (f64, f64), radii: Vec2, start: f64, sweep: f64, sign: f64) {
    if sign >= 0.0 {
        arc(path, center, radii, start, sweep);
    } else {
        arc(path, center, radii, -start, -sweep);
    }
}

#[cfg(test)]
mod tests {
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
            let [paren, bracket, brace] = [Delim::Paren, Delim::Bracket, Delim::Brace]
                .map(|delim| style.bow_for(delim, height));
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
}
