//! Identicons are the visual form of node ids — bijective, with all
//! 128 bits recoverable from a rendering at the small standard size,
//! and salience decaying exponentially from the low bits to the high
//! (the git-short-hash principle).
//!
//! The icon is a nested mosaic. Low bits pick a family — an outline
//! paired with a subdivision vocabulary that suits it: grids live only
//! inside grid-friendly outlines (sharp square, rounded square, and
//! the diamond, whose grid rotates to fit exactly), while round and
//! pointy outlines subdivide in boundary-normalized polar coordinates
//! (radius is a fraction of the distance to the outline in each
//! direction, so rings nest the shape and wedges reach into its
//! corners — the shield's shoulders and point carry mosaic instead of
//! blank margin). Then base hue, chroma, and the
//! level-1 division; four level-1 regions carry hue transforms
//! (analogous-leaning, with one complement accent and a neutral);
//! sixteen level-2 regions carry lightness; sixty-four leaf tiles
//! carry one-bit offsets, tiling the icon with no blank space. Grout
//! and frame show the raw base color, anchoring the decode. Colors are
//! OKLCH, so the lightness channel reads uniformly across hues. Every
//! bit lands in a region of cell scale or larger.
//!
//! Bits, low to high: 0..3 family, 3..6 base hue, 6..8 chroma, 8..10
//! level-1 pattern, 10..12 grout, 12..14 frame, 14..30 level 1 (4 x
//! hue transform + split variant), 30..62 level 2 (16 x lightness),
//! 62..126 level 3 (64 x offset sign), 126..128 offset magnitude.
//! Identicons are not spoof-resistant.

use progred_graph::NodeId;
use puri::draw::Canvas;
use puri::layout::{Extent, Node, leaf};
use std::f64::consts::PI;
use vello::kurbo::{Affine, BezPath, Circle, Point, Rect, RoundedRect, Stroke};
use vello::peniko::Color;

pub fn node_identicon<P: Canvas>(id: NodeId, size: f64) -> Node<P> {
    leaf(icon_extent(size), move |p: &mut P, at| {
        draw(p, icon_rect(at, size), id);
    })
}

/// Edge labels currently render the same as nodes (the outline is
/// identity data, so the old circular-label treatment yielded);
/// context and size distinguish them until a better treatment lands.
pub fn label_identicon<P: Canvas>(id: NodeId, size: f64) -> Node<P> {
    node_identicon(id, size)
}

fn icon_extent(size: f64) -> Extent {
    Extent {
        width: size,
        ascent: size * 0.8,
        descent: size * 0.2,
    }
}

fn icon_rect(at: Point, size: f64) -> Rect {
    let top = at.y - size * 0.8;
    Rect::new(at.x, top, at.x + size, top + size)
}

const HUE_STEP: f32 = 45.0;
const CHROMAS: [f32; 4] = [0.07, 0.10, 0.13, 0.16];
const NEUTRAL_CHROMA: f32 = 0.015;
/// Analogous-leaning: small rotations, one complement accent, a
/// near-complement, and a neutral. All distinct mod 360.
const HUE_TRANSFORMS: [f32; 7] = [0.0, 30.0, -30.0, 60.0, -60.0, 150.0, 180.0];
const LEVEL2_L: [f32; 4] = [0.85, 0.74, 0.63, 0.52];
// None may equal half the distance between two level-2 values, or
// leaf lightnesses would collide; the tests enforce it.
const LEVEL3_DELTA: [f32; 4] = [0.018, 0.030, 0.044, 0.062];
const GROUT: [f64; 4] = [0.010, 0.018, 0.030, 0.045];
const BASE_L: f32 = 0.30;
const BASE_CHROMA: f32 = 0.06;

struct Fields {
    family: usize,
    base_hue: f32,
    chroma: f32,
    l1_pattern: u8,
    grout: f64,
    frame: u8,
    l1_transform: [u8; 4],
    l1_split: [u8; 4],
    l2_lightness: [u8; 16],
    l3_sign: u64,
    delta: f32,
}

fn fields(id: NodeId) -> Fields {
    let bits = id.as_u128();
    let get = |low: usize, width: usize| ((bits >> low) & ((1 << width) - 1)) as u64;
    Fields {
        family: get(0, 3) as usize,
        base_hue: get(3, 3) as f32 * HUE_STEP,
        chroma: CHROMAS[get(6, 2) as usize],
        l1_pattern: get(8, 2) as u8,
        grout: GROUT[get(10, 2) as usize],
        frame: get(12, 2) as u8,
        l1_transform: std::array::from_fn(|i| get(14 + i * 4, 3) as u8),
        l1_split: std::array::from_fn(|i| get(17 + i * 4, 1) as u8),
        l2_lightness: std::array::from_fn(|i| get(30 + i * 2, 2) as u8),
        l3_sign: get(62, 64),
        delta: LEVEL3_DELTA[get(126, 2) as usize],
    }
}

/// A region is a rect (grid families) or a boundary-normalized polar
/// cell (radial families): the physical radius along direction `a` is
/// `t * scale * rho_unit(shape, a)`, so `t` runs 0..1 from center to
/// outline whatever the outline's shape. Splits always yield four
/// children in a fixed order so bit positions stay decodable.
#[derive(Clone, Copy)]
enum Region {
    Rect(Rect),
    Sector { shape: u8, scale: f64, t0: f64, t1: f64, a0: f64, a1: f64 },
}

/// (outline kind, radial engine, content scale, annular).
///
/// Families that share an outline or a subdivision vocabulary carry a
/// constant structural signature so no two families can render alike:
/// the matted rounded square insets its tiles inside a visible mat
/// (0.82 vs the sharp square's flush 1.0), and the second circle
/// family is an annulus with a center hole.
const FAMILIES: [(u8, bool, f64, bool); 8] = [
    (7, false, 1.0, false),   // sharp square, flush grid
    (1, false, 0.82, false),  // rounded square, matted grid
    (3, false, 0.707, false), // diamond, grid rotated 45° (exact fit)
    (4, true, 0.90, false),   // hexagon, radial
    (0, true, 0.92, false),   // circle, radial
    (0, true, 0.92, true),    // annulus, radial around a center hole
    (2, true, 0.90, false),   // squircle, radial
    (5, true, 0.88, false),   // shield, radial
];

fn draw<P: Canvas>(p: &mut P, rect: Rect, id: NodeId) {
    let f = fields(id);
    let (outline, radial, content_factor, annular) = FAMILIES[f.family];
    let base = oklch(BASE_L, BASE_CHROMA, f.base_hue);
    let center = rect.center();
    let w = rect.width();

    let root = if radial {
        Region::Sector {
            shape: outline,
            scale: w / 2.0 * content_factor,
            t0: if annular { 0.24 } else { 0.0 },
            t1: 1.0,
            a0: -PI / 2.0,
            a1: 1.5 * PI,
        }
    } else {
        let half = w / 2.0 * content_factor;
        Region::Rect(Rect::new(-half, -half, half, half))
    };
    // Grid content lives in a local frame so the diamond family can
    // rotate its grid to match its outline.
    let transform = Affine::translate(center.to_vec2())
        * if f.family == 2 {
            Affine::rotate(PI / 4.0)
        } else {
            Affine::IDENTITY
        };

    let body = |p: &mut P| {
        p.fill(rect, base, Affine::IDENTITY);
        let l1_regions = split(root, l1_mode(radial, annular, f.l1_pattern));
        for (i, l1) in l1_regions.into_iter().enumerate() {
            let l1 = shrink(l1, f.grout * w);
            let split2 = subdivision(radial, &l1, f.l1_split[i] == 1);
            for (j, l2) in split(l1, split2).into_iter().enumerate() {
                for (k, l3) in split(l2, subdivision(radial, &l2, false)).into_iter().enumerate()
                {
                    let leaf_index = i * 16 + j * 4 + k;
                    let sign = (f.l3_sign >> leaf_index) & 1 == 1;
                    let color = leaf_color(&f, i, i * 4 + j, sign);
                    fill_region(p, l3, color, transform);
                }
            }
        }
        frame(p, rect, outline, f.frame, oklch(0.86, f.chroma * 0.5, f.base_hue));
    };

    // Clip the mosaic to the silhouette, then stroke that silhouette in
    // a dark hairline so the icon reads as a defined shape on any
    // background; the frame bits stay a secondary inner decoration.
    let edge = oklch(0.24, 0.015, f.base_hue);
    let edge_stroke = Stroke::new(0.045 * w);
    match outline_shape(rect, outline) {
        Outline::Circle(c) => {
            p.clip(c, Affine::IDENTITY, body);
            p.stroke(c, edge_stroke, edge, Affine::IDENTITY);
        }
        Outline::Rounded(r) => {
            p.clip(r, Affine::IDENTITY, body);
            p.stroke(r, edge_stroke, edge, Affine::IDENTITY);
        }
        Outline::Path(path) => {
            p.clip(path.clone(), Affine::IDENTITY, body);
            p.stroke(path, edge_stroke, edge, Affine::IDENTITY);
        }
    }
}

/// No level-1 mode is rings-only: the grout inset would erase a thin
/// ring outright, so every polar mode keeps angular boundaries.
#[derive(Clone, Copy)]
enum Split {
    Quad,
    Rows,
    Cols,
    GoldenQuad,
    PolarQuad,
    PolarQuadGolden,
    Angular,
    AngularOffset,
}

fn l1_mode(radial: bool, annular: bool, pattern: u8) -> Split {
    if !radial {
        [Split::Quad, Split::Rows, Split::Cols, Split::GoldenQuad][pattern as usize]
    } else if annular {
        [Split::PolarQuadGolden, Split::PolarQuad, Split::Angular, Split::AngularOffset]
            [pattern as usize]
    } else {
        [Split::Angular, Split::PolarQuad, Split::PolarQuadGolden, Split::AngularOffset]
            [pattern as usize]
    }
}

fn subdivision(radial: bool, region: &Region, alternate: bool) -> Split {
    match (radial, alternate) {
        (false, false) => Split::Quad,
        (false, true) => match region {
            Region::Rect(r) if r.width() >= r.height() => Split::Cols,
            _ => Split::Rows,
        },
        (true, false) => Split::PolarQuad,
        (true, true) => Split::Angular,
    }
}

fn split(region: Region, mode: Split) -> [Region; 4] {
    match region {
        Region::Rect(r) => {
            let quad = |fx: f64| {
                let (x, y) = (r.x0 + r.width() * fx, r.y0 + r.height() * fx);
                [
                    Region::Rect(Rect::new(r.x0, r.y0, x, y)),
                    Region::Rect(Rect::new(x, r.y0, r.x1, y)),
                    Region::Rect(Rect::new(r.x0, y, x, r.y1)),
                    Region::Rect(Rect::new(x, y, r.x1, r.y1)),
                ]
            };
            match mode {
                Split::Rows => std::array::from_fn(|i| {
                    let h = r.height() / 4.0;
                    Region::Rect(Rect::new(
                        r.x0,
                        r.y0 + i as f64 * h,
                        r.x1,
                        r.y0 + (i + 1) as f64 * h,
                    ))
                }),
                Split::Cols => std::array::from_fn(|i| {
                    let w = r.width() / 4.0;
                    Region::Rect(Rect::new(
                        r.x0 + i as f64 * w,
                        r.y0,
                        r.x0 + (i + 1) as f64 * w,
                        r.y1,
                    ))
                }),
                Split::GoldenQuad => quad(0.38),
                _ => quad(0.5),
            }
        }
        Region::Sector { shape, scale, t0, t1, a0, a1 } => {
            let sector = |t0: f64, t1: f64, a0: f64, a1: f64| Region::Sector {
                shape,
                scale,
                t0,
                t1,
                a0,
                a1,
            };
            let angular = |phase: f64| {
                std::array::from_fn(|i| {
                    let step = (a1 - a0) / 4.0;
                    sector(
                        t0,
                        t1,
                        a0 + phase + i as f64 * step,
                        a0 + phase + (i + 1) as f64 * step,
                    )
                })
            };
            // Radius splits are equal-area, not equal-step, so inner
            // leaves stay readable at the small standard size.
            let polar_quad = |frac: f64, phase: f64| {
                let tm = (t0 * t0 + (t1 * t1 - t0 * t0) * frac).sqrt();
                let am = a0 + phase + (a1 - a0) / 2.0;
                let (b0, b1) = (a0 + phase, a1 + phase);
                [
                    sector(t0, tm, b0, am),
                    sector(t0, tm, am, b1),
                    sector(tm, t1, b0, am),
                    sector(tm, t1, am, b1),
                ]
            };
            match mode {
                Split::Angular => angular(0.0),
                Split::AngularOffset => angular((a1 - a0) / 8.0),
                Split::PolarQuadGolden => polar_quad(0.38, (a1 - a0) / 8.0),
                _ => polar_quad(0.5, 0.0),
            }
        }
    }
}

fn shrink(region: Region, gap: f64) -> Region {
    match region {
        Region::Rect(r) => Region::Rect(r.inset(-gap)),
        Region::Sector { shape, scale, t0, t1, a0, a1 } => {
            let rho = scale * rho_unit(shape, (a0 + a1) / 2.0);
            let dt = (gap / rho).min((t1 - t0) * 0.3);
            let mid_r = (t0 + t1) / 2.0 * rho;
            let da = (gap / mid_r.max(gap)).min((a1 - a0) * 0.3);
            Region::Sector {
                shape,
                scale,
                t0: if t0 > 0.0 { t0 + dt } else { 0.0 },
                t1: t1 - dt,
                a0: a0 + da,
                a1: a1 - da,
            }
        }
    }
}

fn fill_region<P: Canvas>(p: &mut P, region: Region, color: Color, transform: Affine) {
    match region {
        Region::Rect(r) => p.fill(r, color, transform),
        Region::Sector { shape, scale, t0, t1, a0, a1 } => {
            let steps = (((a1 - a0).abs() / (PI / 12.0)).ceil() as usize).max(2);
            let at = |t: f64, s: usize| {
                let angle = a0 + (a1 - a0) * s as f64 / steps as f64;
                let r = t * scale * rho_unit(shape, angle);
                (r * angle.cos(), r * angle.sin())
            };
            let mut path = BezPath::new();
            path.move_to(at(t1, 0));
            for s in 1..=steps {
                path.line_to(at(t1, s));
            }
            if t0 > 0.0 {
                for s in (0..=steps).rev() {
                    path.line_to(at(t0, s));
                }
            } else {
                path.line_to((0.0, 0.0));
            }
            path.close_path();
            p.fill(path, color, transform);
        }
    }
}

/// Frames stroke the family's actual outline so the border always
/// matches the silhouette.
fn frame<P: Canvas>(p: &mut P, rect: Rect, outline: u8, style: u8, color: Color) {
    if style == 0 {
        return;
    }
    let w = rect.width();
    let width = [0.0, 0.03, 0.06, 0.10][style as usize] * w;
    let stroke_outline = |p: &mut P, r: Rect, width: f64, color: Color| {
        let stroke = Stroke::new(width);
        match outline_shape(r, outline) {
            Outline::Circle(c) => p.stroke(c, stroke, color, Affine::IDENTITY),
            Outline::Rounded(rr) => p.stroke(rr, stroke, color, Affine::IDENTITY),
            Outline::Path(path) => p.stroke(path, stroke, color, Affine::IDENTITY),
        }
    };
    stroke_outline(p, rect, width, color);
    if style == 3 {
        stroke_outline(p, rect.inset(-0.05 * w), 0.015 * w, oklch(0.20, 0.02, 0.0));
    }
}

/// Leaf tiles carry the whole color story: hue class from level 1,
/// lightness level from level 2, offset from level 3. Grout shows the
/// raw base, which anchors the decode.
fn leaf_color(f: &Fields, l1: usize, l2: usize, positive: bool) -> Color {
    let transform = f.l1_transform[l1];
    let (hue, chroma) = if transform == 7 {
        (f.base_hue, NEUTRAL_CHROMA)
    } else {
        (f.base_hue + HUE_TRANSFORMS[transform as usize], f.chroma)
    };
    let delta = if positive { f.delta } else { -f.delta };
    oklch(
        LEVEL2_L[f.l2_lightness[l2] as usize] + delta,
        chroma,
        hue,
    )
}

/// Radial families measured in half-width units, centered like the
/// content (rect center → origin). Must trace the same silhouettes as
/// `outline_shape`, so the two are edited together.
const HEXAGON: [(f64, f64); 6] = [
    (-0.5, -1.0),
    (0.5, -1.0),
    (1.0, 0.0),
    (0.5, 1.0),
    (-0.5, 1.0),
    (-1.0, 0.0),
];
const SHIELD: [(f64, f64); 5] = [
    (-1.0, -1.0),
    (1.0, -1.0),
    (1.0, 1.0 / 3.0),
    (0.0, 1.0),
    (-1.0, 1.0 / 3.0),
];

/// Distance from the icon center to the family's outline along
/// `angle`, in half-width units — so a circle is 1 everywhere and a
/// shield reaches ~√2 toward its shoulders. Boundary-normalized polar
/// content multiplies its normalized radius by this, filling each
/// shape's corners instead of stopping at an inscribed circle.
fn rho_unit(shape: u8, angle: f64) -> f64 {
    let dir = (angle.cos(), angle.sin());
    match shape {
        0 => 1.0,
        // Squircle as a superellipse (n = 4): between circle and
        // square, reaching 2^¼ ≈ 1.19 toward the corners.
        2 => 1.0 / (dir.0.abs().powi(4) + dir.1.abs().powi(4)).powf(0.25),
        4 => polygon_radius(&HEXAGON, dir),
        _ => polygon_radius(&SHIELD, dir),
    }
}

/// Ray from the origin along unit `dir` to a convex polygon that
/// contains it; returns the hit distance (the origin-centered polygon
/// radius in that direction).
fn polygon_radius(poly: &[(f64, f64)], dir: (f64, f64)) -> f64 {
    let (dx, dy) = dir;
    for i in 0..poly.len() {
        let (ax, ay) = poly[i];
        let (bx, by) = poly[(i + 1) % poly.len()];
        let (ex, ey) = (bx - ax, by - ay);
        let denom = dx * ey - dy * ex;
        if denom.abs() < 1e-9 {
            continue;
        }
        let t = (ax * ey - ay * ex) / denom;
        let s = (ax * dy - ay * dx) / denom;
        if t > 0.0 && (0.0..=1.0).contains(&s) {
            return t;
        }
    }
    1.0
}

enum Outline {
    Circle(Circle),
    Rounded(RoundedRect),
    Path(BezPath),
}

fn outline_shape(rect: Rect, kind: u8) -> Outline {
    let c = rect.center();
    let w = rect.width();
    let poly = |points: &[(f64, f64)]| {
        let mut path = BezPath::new();
        path.move_to(points[0]);
        for point in &points[1..] {
            path.line_to(*point);
        }
        path.close_path();
        Outline::Path(path)
    };
    match kind {
        0 => Outline::Circle(Circle::new(c, w / 2.0)),
        1 => Outline::Rounded(RoundedRect::from_rect(rect, w / 10.0)),
        2 => Outline::Rounded(RoundedRect::from_rect(rect, w / 3.2)),
        3 => poly(&[(c.x, rect.y0), (rect.x1, c.y), (c.x, rect.y1), (rect.x0, c.y)]),
        4 => poly(&[
            (rect.x0 + w / 4.0, rect.y0),
            (rect.x1 - w / 4.0, rect.y0),
            (rect.x1, c.y),
            (rect.x1 - w / 4.0, rect.y1),
            (rect.x0 + w / 4.0, rect.y1),
            (rect.x0, c.y),
        ]),
        5 => {
            let mut path = BezPath::new();
            path.move_to((rect.x0, rect.y0));
            path.line_to((rect.x1, rect.y0));
            path.line_to((rect.x1, c.y + w / 6.0));
            path.line_to((c.x, rect.y1));
            path.line_to((rect.x0, c.y + w / 6.0));
            path.close_path();
            Outline::Path(path)
        }
        _ => Outline::Rounded(RoundedRect::from_rect(rect, w / 24.0)),
    }
}

/// OKLCH to sRGB (Ottosson's OKLab). Out-of-gamut chroma is walked in
/// rather than RGB-clamped: lightness carries 96 of the bits and must
/// survive exactly; chroma carries two, read where the gamut is wide.
fn oklch(lightness: f32, chroma: f32, hue_degrees: f32) -> Color {
    let h = hue_degrees.to_radians();
    let l = lightness.clamp(0.05, 0.97);
    let rgb = |c: f32| linear_rgb(l, c * h.cos(), c * h.sin());
    let in_gamut = |c: f32| rgb(c).iter().all(|x| (0.0..=1.0).contains(x));

    let mut c = chroma;
    if !in_gamut(c) {
        let (mut lo, mut hi) = (0.0, c);
        for _ in 0..20 {
            c = (lo + hi) / 2.0;
            if in_gamut(c) { lo = c } else { hi = c }
        }
        c = lo;
    }

    let gamma = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    let [r, g, b] = rgb(c);
    Color::new([gamma(r), gamma(g), gamma(b), 1.0])
}

fn linear_rgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_93 * s3,
        -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_fields_partition_all_128_bits() {
        let mut covered = [false; 128];
        let mut mark = |low: usize, width: usize| {
            for bit in low..low + width {
                assert!(!covered[bit], "bit {bit} claimed twice");
                covered[bit] = true;
            }
        };
        mark(0, 3); // family
        mark(3, 3); // base hue
        mark(6, 2); // chroma
        mark(8, 2); // level-1 pattern
        mark(10, 2); // grout
        mark(12, 2); // frame
        for region in 0..4 {
            mark(14 + region * 4, 3); // hue transform
            mark(17 + region * 4, 1); // split variant
        }
        for region in 0..16 {
            mark(30 + region * 2, 2); // lightness level
        }
        mark(62, 64); // leaf offsets
        mark(126, 2); // offset magnitude
        assert!(covered.iter().all(|claimed| *claimed));
    }

    #[test]
    fn leaf_lightness_values_never_collide() {
        for delta in LEVEL3_DELTA {
            let mut values: Vec<f32> = LEVEL2_L
                .iter()
                .flat_map(|level| [level - delta, level + delta])
                .collect();
            values.sort_by(f32::total_cmp);
            assert!(
                values.windows(2).all(|pair| pair[1] - pair[0] > 0.005),
                "collision at delta {delta}"
            );
        }
        // The base coat stays outside the leaf range so grout decodes.
        for delta in LEVEL3_DELTA {
            for level in LEVEL2_L {
                assert!((level - delta - BASE_L).abs() > 0.02);
            }
        }
    }

    #[test]
    fn hue_transforms_are_distinct_and_grout_is_never_zero() {
        for (i, a) in HUE_TRANSFORMS.iter().enumerate() {
            for b in &HUE_TRANSFORMS[i + 1..] {
                assert!((a - b).rem_euclid(360.0) > 1.0);
            }
        }
        assert!(GROUT.iter().all(|gap| *gap > 0.0));
    }

    #[test]
    fn radial_families_reach_their_corners() {
        // Boundary-normalized content must extend past the inscribed
        // circle toward each shape's corners, or the shoulders go
        // blank — the whole point of the polar rework.
        let sample = |shape: u8| {
            (0..360)
                .map(|deg| rho_unit(shape, (deg as f64).to_radians()))
                .fold(f64::MIN, f64::max)
        };
        for shape in [2, 4, 5] {
            assert!(sample(shape) > 1.1, "shape {shape} never leaves the disc");
        }
        // The circle is the unit everywhere by definition.
        for deg in 0..360 {
            assert!((rho_unit(0, (deg as f64).to_radians()) - 1.0).abs() < 1e-9);
        }
        // Shield shoulders (the reported blank spot) carry content.
        let shoulder = rho_unit(5, (-45.0_f64).to_radians());
        assert!(shoulder > 1.3, "shield shoulder radius {shoulder}");
    }
}
