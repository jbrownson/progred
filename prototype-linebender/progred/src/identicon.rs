//! Identicons are the visual form of node ids — bijective, with all
//! 128 bits recoverable from a rendering at the small standard size,
//! and salience decaying exponentially from the low bits to the high
//! (the git-short-hash principle).
//!
//! The icon is a badge grammar. Low bits choose a silhouette family,
//! its variant, base hue, chroma, and a layout whose vocabulary is
//! family-specific: squares divide as grids, bands, or nested rings;
//! radial shapes as wedges, rings, or ring-crosses, in
//! boundary-normalized polar coordinates so regions follow the outline
//! into its corners. Four region records then carry the content — a
//! hue transform (analogous, one complement accent, a neutral), a
//! panel lightness, and an inner figure (disc, ring, diamond, square,
//! bars, plus, saltire) with a tone and four sizes — shapes inside
//! shapes, centered and sized by each region's inscribed circle so
//! they never cross their container. No vocabulary has a "none" and
//! no field renders below the standard size's raster: a field too
//! small to read breaks the bijection the same as one hidden. The
//! high half is a beaded rim: 34 beads at four lightness steps on a
//! dark band that follows the silhouette, ornament rather than noise.
//! Grout, band, and base coat show the base hue, anchoring the
//! decode; colors are OKLCH so the lightness channel reads uniformly
//! across hues.
//!
//! Bits, low to high: 0..2 family, 2..3 variant, 3..6 base hue, 6..8
//! chroma, 8..10 layout, 10..54 regions (4 x: transform 3, lightness
//! 2, figure 3, tone 1, size 2), 54..56 grout, 56..58 rim band,
//! 58..60 bead size, 60..128 beads (34 x 2). Identicons are not
//! spoof-resistant.

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
/// identity data, so a separate label treatment would hide it);
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

/// Category centers in OKLCH degrees — red, orange, yellow, green,
/// teal, blue, purple, pink. Glance identity is verbal ("the red
/// one"), so anchors sit where color names are unambiguous rather
/// than at uniform steps.
const BASE_HUES: [f32; 8] = [29.0, 65.0, 105.0, 142.0, 192.0, 255.0, 300.0, 340.0];
const CHROMAS: [f32; 4] = [0.07, 0.10, 0.13, 0.16];
const NEUTRAL_CHROMA: f32 = 0.015;
/// Analogous: rotations within ±45° so the base hue stays the icon's
/// at-a-glance cast, one complement accent, and a neutral. All
/// distinct mod 360.
const HUE_TRANSFORMS: [f32; 7] = [0.0, 15.0, -15.0, 30.0, -30.0, 45.0, 180.0];
const PANEL_L: [f32; 4] = [0.85, 0.74, 0.63, 0.52];
const FIGURE_TONE: f32 = 0.16;
/// Fractions of the region's inscribed radius; the largest stays
/// inside it with margin.
const FIGURE_SIZE: [f64; 4] = [0.36, 0.50, 0.63, 0.76];
const BEAD_L: [f32; 4] = [0.46, 0.60, 0.74, 0.88];
const GROUT: [f64; 4] = [0.012, 0.018, 0.026, 0.036];
const BAND: [f64; 4] = [0.10, 0.13, 0.17, 0.22];
const BEAD_FRAC: [f64; 4] = [0.38, 0.48, 0.58, 0.70];
const BEADS: usize = 34;
const BASE_L: f32 = 0.30;
const BASE_CHROMA: f32 = 0.09;

struct RegionSpec {
    transform: u8,
    lightness: u8,
    figure: u8,
    tone: u8,
    size: u8,
}

struct Fields {
    family: usize,
    variant: usize,
    base_hue: f32,
    chroma: f32,
    layout: usize,
    regions: [RegionSpec; 4],
    grout: f64,
    band: f64,
    bead_frac: f64,
    beads: [u8; BEADS],
}

fn fields(id: NodeId) -> Fields {
    let bits = id.as_u128();
    let get = |low: usize, width: usize| ((bits >> low) & ((1 << width) - 1)) as u64;
    Fields {
        family: get(0, 2) as usize,
        variant: get(2, 1) as usize,
        base_hue: BASE_HUES[get(3, 3) as usize],
        chroma: CHROMAS[get(6, 2) as usize],
        layout: get(8, 2) as usize,
        regions: std::array::from_fn(|i| {
            let b = 10 + i * 11;
            RegionSpec {
                transform: get(b, 3) as u8,
                lightness: get(b + 3, 2) as u8,
                figure: get(b + 5, 3) as u8,
                tone: get(b + 8, 1) as u8,
                size: get(b + 9, 2) as u8,
            }
        }),
        grout: GROUT[get(54, 2) as usize],
        band: BAND[get(56, 2) as usize],
        bead_frac: BEAD_FRAC[get(58, 2) as usize],
        beads: std::array::from_fn(|i| get(60 + i * 2, 2) as u8),
    }
}

/// Boundary-normalized radial shape ids for `rho_unit`.
const RHO_CIRCLE: u8 = 0;
const RHO_SQUARE: u8 = 1;
const RHO_DIAMOND: u8 = 2;
const RHO_SHIELD: u8 = 3;
const RHO_HEX_FLAT: u8 = 4;
const RHO_HEX_POINT: u8 = 5;

/// Polygons in half-width units, stretched to fill square bounds.
/// Must trace the same silhouettes as `outline_shape`, so the two are
/// edited together.
const DIAMOND: [(f64, f64); 4] = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
const SHIELD: [(f64, f64); 5] = [
    (-1.0, -1.0),
    (1.0, -1.0),
    (1.0, 1.0 / 3.0),
    (0.0, 1.0),
    (-1.0, 1.0 / 3.0),
];
const HEX_FLAT: [(f64, f64); 6] = [
    (-0.5, -1.0),
    (0.5, -1.0),
    (1.0, 0.0),
    (0.5, 1.0),
    (-0.5, 1.0),
    (-1.0, 0.0),
];
const HEX_POINT: [(f64, f64); 6] = [
    (0.0, -1.0),
    (1.0, -0.5),
    (1.0, 0.5),
    (0.0, 1.0),
    (-1.0, 0.5),
    (-1.0, -0.5),
];

/// Distance from the icon center to the silhouette along `angle`, in
/// half-width units. Boundary-normalized polar content multiplies its
/// normalized radius by this, filling each shape's corners instead of
/// stopping at an inscribed circle.
fn rho_unit(shape: u8, angle: f64) -> f64 {
    let dir = (angle.cos(), angle.sin());
    match shape {
        RHO_CIRCLE => 1.0,
        RHO_SQUARE => 1.0 / dir.0.abs().max(dir.1.abs()),
        RHO_DIAMOND => polygon_radius(&DIAMOND, dir),
        RHO_SHIELD => polygon_radius(&SHIELD, dir),
        RHO_HEX_FLAT => polygon_radius(&HEX_FLAT, dir),
        _ => polygon_radius(&HEX_POINT, dir),
    }
}

/// Ray from the origin along unit `dir` to a convex polygon that
/// contains it; returns the hit distance.
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

/// (rho shape, annular hole) for a family + variant.
fn family_shape(family: usize, variant: usize) -> (u8, f64) {
    match (family, variant) {
        (0, _) => (RHO_SQUARE, 0.0),
        (1, 0) => (RHO_CIRCLE, 0.0),
        (1, _) => (RHO_CIRCLE, 0.24),
        (2, 0) => (RHO_DIAMOND, 0.0),
        (2, _) => (RHO_SHIELD, 0.0),
        (3, 0) => (RHO_HEX_FLAT, 0.0),
        _ => (RHO_HEX_POINT, 0.0),
    }
}

#[derive(Clone, Copy)]
enum Region {
    Rect(Rect),
    Sector { shape: u8, scale: f64, t0: f64, t1: f64, a0: f64, a1: f64 },
}

/// Four regions for a family-specific layout, in content space
/// t in [hole, cap]. Region order is fixed per layout so bit
/// positions stay decodable.
fn layout_regions(shape: u8, scale: f64, layout: usize, hole: f64, cap: f64) -> [Region; 4] {
    let sector =
        |t0: f64, t1: f64, a0: f64, a1: f64| Region::Sector { shape, scale, t0, t1, a0, a1 };
    // Ring boundaries are equal-area so inner regions stay readable.
    let ring_t =
        |i: usize| (hole * hole + (cap * cap - hole * hole) * i as f64 / 4.0).sqrt();
    let wedges = |phase: f64| {
        std::array::from_fn(|i| {
            let step = PI / 2.0;
            let a = -PI / 2.0 + phase + i as f64 * step;
            sector(hole, cap, a, a + step)
        })
    };
    if shape == RHO_SQUARE {
        // Grid vocabulary: quadrants, golden quadrants, bands, rings.
        let half = cap * scale;
        let r = Rect::new(-half, -half, half, half);
        let quad = |fx: f64| {
            let (x, y) = (r.x0 + r.width() * fx, r.y0 + r.height() * fx);
            [
                Region::Rect(Rect::new(r.x0, r.y0, x, y)),
                Region::Rect(Rect::new(x, r.y0, r.x1, y)),
                Region::Rect(Rect::new(r.x0, y, x, r.y1)),
                Region::Rect(Rect::new(x, y, r.x1, r.y1)),
            ]
        };
        match layout {
            0 => quad(0.5),
            1 => quad(0.4),
            2 => std::array::from_fn(|i| {
                let w = r.width() / 4.0;
                Region::Rect(Rect::new(
                    r.x0 + i as f64 * w,
                    r.y0,
                    r.x0 + (i + 1) as f64 * w,
                    r.y1,
                ))
            }),
            _ => std::array::from_fn(|i| sector(ring_t(i), ring_t(i + 1), -PI / 2.0, 1.5 * PI)),
        }
    } else {
        // Polar vocabulary: wedges, rings, ring x wedge, offset wedges.
        match layout {
            0 => wedges(0.0),
            1 => std::array::from_fn(|i| sector(ring_t(i), ring_t(i + 1), -PI / 2.0, 1.5 * PI)),
            2 => {
                let tm = ring_t(2);
                [
                    sector(hole, tm, -PI / 2.0, PI / 2.0),
                    sector(hole, tm, PI / 2.0, 1.5 * PI),
                    sector(tm, cap, -PI / 2.0, PI / 2.0),
                    sector(tm, cap, PI / 2.0, 1.5 * PI),
                ]
            }
            _ => wedges(PI / 4.0),
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
            let full_turn = (a1 - a0) >= 2.0 * PI - 1e-9;
            let da = if full_turn {
                0.0
            } else {
                (gap / mid_r.max(gap)).min((a1 - a0) * 0.3)
            };
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
            let steps = (((a1 - a0).abs() / (PI / 16.0)).ceil() as usize).max(2);
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

/// Lowest boundary distance over an angular window, for conservative
/// containment in non-circular shapes.
fn rho_floor(shape: u8, a0: f64, a1: f64) -> f64 {
    (0..=16)
        .map(|k| rho_unit(shape, a0 + (a1 - a0) * k as f64 / 16.0))
        .fold(f64::MAX, f64::min)
}

/// Center and inscribed radius of a region: the figure anchor and the
/// largest circle that stays inside it, so figures never cross their
/// container. A full-turn ring anchors at twelve o'clock on the ring
/// itself, so concentric regions never stack their figures at the
/// center; a tapering wedge anchors at its equal-area radius, where
/// there is room.
fn inscribed(region: &Region) -> (Point, f64) {
    match region {
        Region::Rect(r) => (r.center(), r.width().min(r.height()) / 2.0),
        Region::Sector { shape, scale, t0, t1, a0, a1 } => {
            let full_turn = (a1 - a0) >= 2.0 * PI - 1e-9;
            if *t0 == 0.0 && full_turn {
                (
                    Point::ORIGIN,
                    t1 * scale * rho_floor(*shape, 0.0, 2.0 * PI) * 0.92,
                )
            } else if full_turn {
                let t = (t0 + t1) / 2.0;
                let up = scale * rho_unit(*shape, -PI / 2.0);
                let local = scale * rho_floor(*shape, -PI / 2.0 - 0.5, -PI / 2.0 + 0.5);
                (Point::new(0.0, -t * up), (t1 - t0) / 2.0 * local * 0.85)
            } else {
                let t = ((t0 * t0 + t1 * t1) / 2.0).sqrt();
                let a = (a0 + a1) / 2.0;
                let mid = scale * rho_unit(*shape, a);
                let floor = scale * rho_floor(*shape, *a0, *a1);
                let radial = (t - t0).min(t1 - t) * floor;
                let lateral = t * floor * ((a1 - a0) / 2.0).min(PI / 2.0).sin();
                (
                    Point::new(t * mid * a.cos(), t * mid * a.sin()),
                    radial.min(lateral) * 0.9,
                )
            }
        }
    }
}

/// Hue class for a region: its transform of the base hue, or neutral.
fn quadrant_color(f: &Fields, spec: &RegionSpec) -> (f32, f32) {
    if spec.transform == 7 {
        (f.base_hue, NEUTRAL_CHROMA)
    } else {
        (f.base_hue + HUE_TRANSFORMS[spec.transform as usize], f.chroma)
    }
}

fn panel_color(f: &Fields, spec: &RegionSpec) -> Color {
    let (hue, chroma) = quadrant_color(f, spec);
    oklch(PANEL_L[spec.lightness as usize], chroma, hue)
}

fn figure_color(f: &Fields, spec: &RegionSpec) -> Color {
    let (hue, chroma) = quadrant_color(f, spec);
    let tone = if spec.tone == 1 { FIGURE_TONE } else { -FIGURE_TONE };
    oklch(PANEL_L[spec.lightness as usize] + tone, chroma, hue)
}

/// The region's inner figure: a small contrasting shape, centered on
/// the region's inscribed circle. The vocabulary has no "none", so
/// every spec field stays visible under every branch.
fn draw_figure<P: Canvas>(
    p: &mut P,
    spec: &RegionSpec,
    center: Point,
    fit: f64,
    color: Color,
    transform: Affine,
) {
    let s = fit * FIGURE_SIZE[spec.size as usize];
    let (cx, cy) = (center.x, center.y);
    let poly = |p: &mut P, points: &[(f64, f64)]| {
        let mut path = BezPath::new();
        path.move_to((cx + points[0].0, cy + points[0].1));
        for point in &points[1..] {
            path.line_to((cx + point.0, cy + point.1));
        }
        path.close_path();
        p.fill(path, color, transform);
    };
    let bar = |p: &mut P, hw: f64, hh: f64| {
        p.fill(Rect::new(cx - hw, cy - hh, cx + hw, cy + hh), color, transform);
    };
    match spec.figure {
        0 => p.fill(Circle::new(center, s), color, transform),
        1 => p.stroke(
            Circle::new(center, s * 0.76),
            Stroke::new((s * 0.46).max(0.7)),
            color,
            transform,
        ),
        2 => poly(p, &[(s, 0.0), (0.0, s), (-s, 0.0), (0.0, -s)]),
        3 => bar(p, s * 0.78, s * 0.78),
        4 => bar(p, s * 0.36, s),
        5 => bar(p, s, s * 0.36),
        6 => {
            bar(p, s * 0.30, s);
            bar(p, s, s * 0.30);
        }
        _ => {
            // Saltire: the plus rotated 45°.
            let arm = |p: &mut P, dir: f64| {
                let (ux, uy) = ((PI / 4.0 + dir).cos(), (PI / 4.0 + dir).sin());
                let (vx, vy) = (-uy * 0.28, ux * 0.28);
                poly(p, &[
                    ((ux + vx) * s, (uy + vy) * s),
                    ((ux - vx) * s, (uy - vy) * s),
                    ((-ux - vx) * s, (-uy - vy) * s),
                    ((-ux + vx) * s, (-uy + vy) * s),
                ]);
            };
            arm(p, 0.0);
            arm(p, PI / 2.0);
        }
    }
}

enum Outline {
    Circle(Circle),
    Rounded(RoundedRect),
    Path(BezPath),
}

fn outline_shape(rect: Rect, family: usize, variant: usize) -> Outline {
    let c = rect.center();
    let w = rect.width();
    let poly_outline = |poly: &[(f64, f64)]| {
        let mut path = BezPath::new();
        path.move_to((c.x + poly[0].0 * w / 2.0, c.y + poly[0].1 * w / 2.0));
        for point in &poly[1..] {
            path.line_to((c.x + point.0 * w / 2.0, c.y + point.1 * w / 2.0));
        }
        path.close_path();
        Outline::Path(path)
    };
    match (family, variant) {
        (0, 0) => Outline::Rounded(RoundedRect::from_rect(rect, w / 28.0)),
        (0, _) => Outline::Rounded(RoundedRect::from_rect(rect, w / 7.0)),
        (1, _) => Outline::Circle(Circle::new(c, w / 2.0)),
        (2, 0) => poly_outline(&DIAMOND),
        (2, _) => poly_outline(&SHIELD),
        (3, 0) => poly_outline(&HEX_FLAT),
        _ => poly_outline(&HEX_POINT),
    }
}

fn draw<P: Canvas>(p: &mut P, rect: Rect, id: NodeId) {
    let f = fields(id);
    let (shape, hole) = family_shape(f.family, f.variant);
    let base = oklch(BASE_L, BASE_CHROMA, f.base_hue);
    let center = rect.center();
    let w = rect.width();
    let scale = w / 2.0 * 0.96;
    let cap = 1.0 - f.band - 0.045;
    let transform = Affine::translate(center.to_vec2());

    let body = |p: &mut P| {
        p.fill(rect, base, Affine::IDENTITY);
        let regions = layout_regions(shape, scale, f.layout, hole, cap);
        for (region, spec) in regions.into_iter().zip(&f.regions) {
            let region = shrink(region, f.grout * w);
            fill_region(p, region, panel_color(&f, spec), transform);
            let (anchor, fit) = inscribed(&region);
            draw_figure(p, spec, anchor, fit, figure_color(&f, spec), transform);
        }
        // The beaded rim: the band shows the base coat; beads carry
        // the high bits at four lightness steps.
        let t_bead = 1.0 - f.band / 2.0;
        for (k, bead) in f.beads.iter().enumerate() {
            let a = -PI / 2.0 + k as f64 / BEADS as f64 * 2.0 * PI;
            let rho = scale * rho_unit(shape, a);
            p.fill(
                Circle::new(
                    Point::new(t_bead * rho * a.cos(), t_bead * rho * a.sin()),
                    f.band * rho * f.bead_frac * 0.5,
                ),
                oklch(BEAD_L[*bead as usize], NEUTRAL_CHROMA * 2.0, f.base_hue),
                transform,
            );
        }
    };

    let edge = oklch(0.24, 0.04, f.base_hue);
    let edge_stroke = Stroke::new(0.04 * w);
    match outline_shape(rect, f.family, f.variant) {
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

/// OKLCH to sRGB (Ottosson's OKLab). Out-of-gamut chroma is walked in
/// rather than RGB-clamped: lightness carries most of the bits and
/// must survive exactly; chroma is read where the gamut is wide.
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
        mark(0, 2); // family
        mark(2, 1); // variant
        mark(3, 3); // base hue
        mark(6, 2); // chroma
        mark(8, 2); // layout
        for region in 0..4 {
            let b = 10 + region * 11;
            mark(b, 3); // hue transform
            mark(b + 3, 2); // panel lightness
            mark(b + 5, 3); // figure
            mark(b + 8, 1); // tone
            mark(b + 9, 2); // size
        }
        mark(54, 2); // grout
        mark(56, 2); // rim band
        mark(58, 2); // bead size
        for bead in 0..BEADS {
            mark(60 + bead * 2, 2);
        }
        assert!(covered.iter().all(|claimed| *claimed));
    }

    #[test]
    fn color_lattices_stay_readable() {
        // Panel levels distinct, clear of the base coat.
        for pair in PANEL_L.windows(2) {
            assert!((pair[0] - pair[1]).abs() > 0.05);
        }
        for level in PANEL_L {
            assert!((level - BASE_L).abs() > 0.1);
        }
        // Figure tone survives the lightness clamp in both directions.
        for level in PANEL_L {
            assert!((level + FIGURE_TONE).min(0.97) - level > 0.05);
            assert!(level - (level - FIGURE_TONE).max(0.05) > 0.05);
        }
        // Bead levels distinct, clear of the band's base coat.
        for pair in BEAD_L.windows(2) {
            assert!((pair[0] - pair[1]).abs() > 0.05);
        }
        for level in BEAD_L {
            assert!((level - BASE_L).abs() > 0.1);
        }
    }

    #[test]
    fn hue_transforms_are_distinct_and_gaps_are_never_zero() {
        for (i, a) in HUE_TRANSFORMS.iter().enumerate() {
            for b in &HUE_TRANSFORMS[i + 1..] {
                let d = (a - b).rem_euclid(360.0);
                assert!(d.min(360.0 - d) > 1.0);
            }
        }
        assert!(GROUT.iter().all(|gap| *gap > 0.0));
        assert!(BAND.iter().all(|band| *band > 0.0));
        assert!(BEAD_FRAC.iter().all(|frac| *frac > 0.0));
    }

    #[test]
    fn radial_shapes_reach_their_corners() {
        let sample = |shape: u8| {
            (0..360)
                .map(|deg| rho_unit(shape, (deg as f64).to_radians()))
                .fold(f64::MIN, f64::max)
        };
        // Boundary-normalized content must extend past the inscribed
        // circle toward each shape's corners.
        assert!(sample(RHO_SQUARE) > 1.35);
        assert!(sample(RHO_SHIELD) > 1.3);
        assert!(sample(RHO_HEX_FLAT) > 1.05);
        assert!(sample(RHO_HEX_POINT) > 1.05);
        for deg in 0..360 {
            assert!((rho_unit(RHO_CIRCLE, (deg as f64).to_radians()) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn every_layout_yields_four_positive_regions() {
        for family in 0..4 {
            for variant in 0..2 {
                let (shape, hole) = family_shape(family, variant);
                for layout in 0..4 {
                    for region in layout_regions(shape, 100.0, layout, hole, 0.8) {
                        match region {
                            Region::Rect(r) => {
                                assert!(r.width() > 0.0 && r.height() > 0.0);
                            }
                            Region::Sector { t0, t1, a0, a1, .. } => {
                                assert!(t1 > t0 && t0 >= 0.0 && a1 > a0);
                            }
                        }
                    }
                }
            }
        }
    }

    fn contains(region: &Region, p: Point) -> bool {
        match region {
            Region::Rect(r) => r.inflate(0.5, 0.5).contains(p),
            Region::Sector { shape, scale, t0, t1, a0, a1 } => {
                let r = p.to_vec2().length();
                if r < 1e-9 {
                    return *t0 < 1e-9;
                }
                let angle = p.y.atan2(p.x);
                let t = r / (scale * rho_unit(*shape, angle));
                let angular = (a1 - a0) >= 2.0 * PI - 1e-9 || {
                    let rel = (angle - a0).rem_euclid(2.0 * PI);
                    rel <= (a1 - a0) + 0.01 || rel >= 2.0 * PI - 0.01
                };
                t >= t0 - 0.005 && t <= t1 + 0.005 && angular
            }
        }
    }

    /// The largest figure, drawn on the inscribed circle, must stay
    /// inside its region for every family, variant, and layout — the
    /// figures-crossing-their-containers class of bug.
    #[test]
    fn figures_stay_inside_their_regions() {
        for family in 0..4 {
            for variant in 0..2 {
                let (shape, hole) = family_shape(family, variant);
                for layout in 0..4 {
                    for region in layout_regions(shape, 100.0, layout, hole, 0.8) {
                        let region = shrink(region, 2.0);
                        let (anchor, fit) = inscribed(&region);
                        assert!(fit > 0.0);
                        let reach = fit * FIGURE_SIZE[3];
                        for k in 0..32 {
                            let a = k as f64 / 32.0 * 2.0 * PI;
                            let probe =
                                anchor + vello::kurbo::Vec2::new(a.cos(), a.sin()) * reach;
                            assert!(
                                contains(&region, probe),
                                "family {family} variant {variant} layout {layout}"
                            );
                        }
                    }
                }
            }
        }
    }
}
