//! Stretched delimiters as filled outlines, the way math fonts draw
//! them: curvature follows the FULL span — the paren one elliptical
//! bow, the brace two mirrored S-waves meeting at its mid point, the
//! bracket square — and the stroke is MODULATED, thick at the bellies
//! and thin at terminals and the brace's point, with blunt-cut ends.
//! A tall delimiter grows wider and gains contrast (TeX's \big
//! through \Bigg grow in both dimensions); the one-line form is the
//! same shape at glyph height, near-monoline at the base weight. Ink
//! spans exactly x in [0, bow-for-height] and y in [top, bottom].

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

/// Proportions taken from the system font's own glyphs (SF Pro:
/// paren/bracket ink 0.21 em wide, brace 0.30 em, both spanning
/// -0.704..+0.171 em around the baseline with 0.05 em side
/// bearings); `line` is the one-line reference height the growth and
/// contrast ramps measure against.
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
            bow: size * 0.21,
            brace_bow: size * 0.30,
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
    let bottom = bottom.max(top + style.stem);
    let (belly, tip) = style.weights(bottom - top);
    let bow = style.bow_for(delim, bottom - top);
    match delim {
        Delim::Paren => paren(bow, top, bottom, belly, tip),
        Delim::Bracket => bracket(bow, top, bottom, belly, tip),
        Delim::Brace => brace(bow, top, bottom, belly, tip),
    }
}

pub fn close(delim: Delim, style: &DelimStyle, top: f64, bottom: f64) -> BezPath {
    let mut path = open(delim, style, top, bottom);
    path.apply_affine(Affine::new([
        -1.0,
        0.0,
        0.0,
        1.0,
        style.bow_for(delim, bottom.max(top + style.stem) - top),
        0.0,
    ]));
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

/// Thick upright, thin arms — the bracket's contrast lives in the
/// stem against its serif-like ticks.
fn bracket(bow: f64, top: f64, bottom: f64, belly: f64, tip: f64) -> BezPath {
    let mut path = Rect::new(0.0, top, belly.min(bow), bottom).to_path(0.05);
    path.extend(Rect::new(0.0, top, bow, top + tip).to_path(0.05));
    path.extend(Rect::new(0.0, bottom - tip, bow, bottom).to_path(0.05));
    path
}

/// Two mirrored S-wave bands meeting at a blunt point face: each half
/// runs terminal cap to point face between an outer and an inner
/// composite curve — quarter-ellipses joined at the vertical-tangent
/// waists, where the band is `belly` thick; ends and the point stay
/// `tip` thin. The halves overlap at the face and fill as one.
fn brace(bow: f64, top: f64, bottom: f64, belly: f64, tip: f64) -> BezPath {
    let mid = (top + bottom) / 2.0;
    let half = (bottom - top) / 2.0;
    let hook = half * 2.0 / 3.0;
    let point = half / 3.0;
    let xw = bow / 2.0;
    let spread = (belly / 2.0).min(xw);
    let mut path = BezPath::new();
    for (cap, sign) in [(top, 1.0), (bottom, -1.0)] {
        let flip = |y: f64| cap + sign * (y - top);
        let hook_center = flip(top + hook);
        let point_center = flip(mid - point);
        path.move_to((bow, cap));
        arc_dir(
            &mut path,
            (bow, hook_center),
            Vec2::new(bow - xw + spread, hook),
            -FRAC_PI_2,
            -FRAC_PI_2,
            sign,
        );
        arc_dir(
            &mut path,
            (0.0, point_center),
            Vec2::new(xw - spread, (point - tip / 2.0).max(0.0)),
            0.0,
            FRAC_PI_2,
            sign,
        );
        path.line_to((0.0, flip(mid + tip / 2.0)));
        arc_dir(
            &mut path,
            (0.0, point_center),
            Vec2::new(xw + spread, (point + tip / 2.0).max(0.0)),
            FRAC_PI_2,
            -FRAC_PI_2,
            sign,
        );
        arc_dir(
            &mut path,
            (bow, hook_center),
            Vec2::new((bow - xw - spread).max(0.0), (hook - tip).max(0.0)),
            PI,
            FRAC_PI_2,
            sign,
        );
        path.close_path();
    }
    path
}

fn arc(path: &mut BezPath, center: (f64, f64), radii: Vec2, start: f64, sweep: f64) {
    path.extend(Arc::new(Point::new(center.0, center.1), radii, start, sweep, 0.0).append_iter(0.05));
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
