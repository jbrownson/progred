//! Stretched delimiters as filled outlines, the way math fonts draw
//! them: the paren one elliptical bow over the FULL span, the brace
//! hooked at its tips and pointed at its middle with straight arms
//! between, the bracket square — and the stroke is MODULATED, thick
//! at the bellies and arms, thin at the paren's blunt-cut terminals
//! and the brace's point.
//! A tall delimiter grows wider and gains contrast (TeX's \big
//! through \Bigg grow in both dimensions); the one-line form is the
//! same shape at glyph height, near-monoline at the base weight. Ink
//! spans exactly x in [0, bow-for-height] and y in [top, bottom].

mod widget;
pub use widget::{Side, advance, draw_stretched, minimum_span};

use kurbo::{Affine, Arc, BezPath, Point, Rect, Shape, Vec2};
use std::f64::consts::{FRAC_PI_2, PI};

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
    /// contrast ramp, the ends stay thin.
    fn weights(&self, height: f64) -> (f64, f64) {
        let belly = self.stem * (height / self.line).sqrt().clamp(1.0, MAX_CONTRAST);
        (belly, self.stem * 0.65)
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

/// One crescent: outer and inner half-ellipses sharing blunt vertical
/// caps at the tips, so the band is `belly` thick at the waist and
/// `tip` thick at the ends.
fn paren(bow: f64, top: f64, bottom: f64, belly: f64, tip: f64) -> BezPath {
    let mid = (top + bottom) / 2.0;
    let half = (bottom - top) / 2.0;
    let outer = Vec2::new(bow, half);
    let inner = Vec2::new((bow - belly).max(0.0), (half - tip).max(0.0));
    let mut path = BezPath::new();
    path.move_to((bow, top));
    arc(&mut path, (bow, mid), outer, -FRAC_PI_2, -FRAC_PI_2);
    arc(&mut path, (bow, mid), outer, PI, -FRAC_PI_2);
    path.line_to((bow, bottom - tip));
    arc(&mut path, (bow, mid), inner, FRAC_PI_2, FRAC_PI_2);
    arc(&mut path, (bow, mid), inner, PI, FRAC_PI_2);
    path.close_path();
    path
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

/// The brace as math fonts draw it: each half a band that curls in a tight
/// hook at its tip, ends there in a round cap, runs straight down its arm,
/// and thins as it curves into the point, where the halves meet at a short
/// face. The band is `belly` thick along the hook and arm and `tip` thick at
/// the point. The hooks and the point keep to the brace's width, so they
/// grow with it. The lower half is the upper one mirrored and reversed, so
/// both wind the same way and fill as one where they overlap.
fn brace(bow: f64, top: f64, bottom: f64, belly: f64, tip: f64) -> BezPath {
    let weight = belly.min(bow / 2.0);
    let half_weight = weight / 2.0;
    let mid = (top + bottom) / 2.0;
    // The tip's centerline, inset so its round cap stays in the box.
    let (end, cap) = (bow - half_weight, top + half_weight);
    let arm = bow / 2.0;
    let reach = (mid - cap).max(0.0);
    let hook = (HOOK * bow).min(reach * 0.45).max(half_weight);
    let point = (POINT * bow).min(reach * 0.55).max(tip);
    let mut path = BezPath::new();
    for sign in [1.0, -1.0] {
        let y = |y: f64| mid + sign * (y - mid);
        let hook_center = (end, y(cap + hook));
        let point_center = (0.0, y(mid - point));
        let mut half = BezPath::new();
        half.move_to((end, y(top)));
        arc_dir(
            &mut half,
            hook_center,
            Vec2::new(end - arm + half_weight, hook + half_weight),
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
        half.line_to((arm + half_weight, y(cap + hook)));
        arc_dir(
            &mut half,
            hook_center,
            Vec2::new(end - arm - half_weight, hook - half_weight),
            PI,
            FRAC_PI_2,
            sign,
        );
        arc_dir(
            &mut half,
            (end, y(cap)),
            Vec2::new(half_weight, half_weight),
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

/// How tall the brace's hook and point curves are, as fractions of its
/// width.
const HOOK: f64 = 0.32;
const POINT: f64 = 0.38;

fn arc(path: &mut BezPath, center: (f64, f64), radii: Vec2, start: f64, sweep: f64) {
    path.extend(
        Arc::new(Point::new(center.0, center.1), radii, start, sweep, 0.0).append_iter(0.05),
    );
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
