//! Opt-in experiment, not a production sweep kernel. See docs/tool-sweep-experiment.md.
use super::*;
use std::time::Instant;

fn choose(condition: Tree, yes: Tree, no: Tree) -> Tree {
    condition.clone().and(yes).or(condition.not().and(no))
}

fn quadratic_min(p: &[Tree; 3], d: [f32; 3], lo: Tree, hi: Tree, axes: usize) -> Tree {
    let dd = d[..axes].iter().map(|x| x * x).sum::<f32>();
    if dd == 0.0 {
        return lo;
    }
    let dot = (0..axes)
        .map(|i| p[i].clone() * d[i])
        .reduce(|a, b| a + b)
        .unwrap();
    (dot / dd).max(lo).min(hi)
}

/// Minimum of a quartic on an interval: endpoints plus the real roots of its
/// cubic derivative. Cardano's real/trigonometric forms are emitted as ordinary
/// Fidget operations, so this tests the actual f32 VM, not just scalar f64 math.
fn quartic_min(c: [Tree; 3], lo: Tree, hi: Tree) -> Tree {
    let at = |t: Tree| ((t.square() + c[2].clone()) * t.clone() + c[1].clone()) * t + c[0].clone();
    let p = c[2].clone() / 2.0;
    let q = c[1].clone() / 4.0;
    let disc = q.square() / 4.0 + p.pow(3) / 27.0;
    let cbrt = |v: Tree| v.compare(0.0) * (v.abs().max(1e-30).ln() / 3.0).exp();
    let root = cbrt(-q.clone() / 2.0 + disc.clone().max(0.0).sqrt())
        + cbrt(-q.clone() / 2.0 - disc.clone().max(0.0).sqrt());
    let one = at(root.max(lo.clone()).min(hi.clone()));
    let magnitude = (-p.clone() / 3.0).max(1e-30).sqrt();
    let cosine = -q / (2.0 * magnitude.pow(3));
    // Clamping the input then calling acos produces 0 * infinity in automatic
    // differentiation at +/-1. Select the constant limiting angle instead.
    let theta = choose(
        cosine.clone().compare(-1.0).max(0.0).not(),
        Tree::constant(std::f32::consts::PI),
        choose(
            cosine.clone().compare(1.0).min(0.0).not(),
            Tree::constant(0.0),
            cosine.acos(),
        ),
    ) / 3.0;
    let three = (0..3)
        .map(|i| {
            let root = 2.0
                * magnitude.clone()
                * (theta.clone() + std::f32::consts::TAU * i as f32 / 3.0).cos();
            at(root.max(lo.clone()).min(hi.clone()))
        })
        .reduce(|a, b| a.min(b))
        .unwrap();
    at(lo)
        .min(at(hi))
        .min(choose(disc.compare(0.0).max(0.0), one, three))
}

fn band_interval(z: Tree, start: f32, end: f32, dz: f32) -> (Tree, Tree, Tree) {
    let (lo, hi) = if dz == 0.0 {
        (Tree::constant(0.0), Tree::constant(1.0))
    } else {
        let t0 = (z.clone() - start) / dz;
        let t1 = (z.clone() - end) / dz;
        (
            t0.clone().min(t1.clone()).max(0.0).min(1.0),
            t0.max(t1).max(0.0).min(1.0),
        )
    };
    let outside = (start + dz.min(0.0) - z.clone()).max(z - (end + dz.max(0.0)));
    (lo, hi, outside)
}

/// Bull mill about Z, tip at `a`, with constant orientation and straight motion.
/// The rounded band is a filled torus section. If A = rho² + h² + R² - r²,
/// its outer disk is inside when rho <= R, A <= 0, or A² - 4R²rho² <= 0.
fn analytic_bull(radius: f32, corner: f32, length: f32, a: [f32; 3], b: [f32; 3]) -> Tree {
    let d: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let p = [
        Tree::x() - a[0],
        Tree::y() - a[1],
        Tree::z() - a[2] - corner,
    ];
    let major = radius - corner;
    let dd = d.iter().map(|v| v * v).sum::<f32>();
    let z = p[2].clone() + corner;
    let (lo, hi, outside) = band_interval(z.clone(), 0.0, corner, d[2]);
    let field = if dd == 0.0 {
        ((p[0].square() + p[1].square()).sqrt() - major)
            .max(0.0)
            .square()
            + p[2].square()
            - corner * corner
    } else {
        // Center distance along the normalized ray at its closest point to the
        // torus center. This removes the cubic coefficient and avoids dividing
        // by |move|^4, particularly ill-conditioned for short moves.
        let distance = dd.sqrt();
        let v = d.map(|v| v / distance);
        let shift = p[0].clone() * v[0] + p[1].clone() * v[1] + p[2].clone() * v[2];
        let q: [Tree; 3] = std::array::from_fn(|i| p[i].clone() - shift.clone() * v[i]);
        let aa = q[0].square() + q[1].square() + q[2].square() + major * major - corner * corner;
        let torus = quartic_min(
            [
                aa.square() - 4.0 * major * major * (q[0].square() + q[1].square()),
                8.0 * major * major * (q[0].clone() * v[0] + q[1].clone() * v[1]),
                2.0 * aa - 4.0 * major * major * (v[0] * v[0] + v[1] * v[1]),
            ],
            lo.clone() * distance - shift.clone(),
            hi.clone() * distance - shift,
        );
        let sphere_t = quadratic_min(&p, d, lo.clone(), hi.clone(), 3);
        let sphere = (0..3)
            .map(|i| (p[i].clone() - sphere_t.clone() * d[i]).square())
            .reduce(|a, b| a + b)
            .unwrap()
            + major * major
            - corner * corner;
        let core_t = quadratic_min(&p, d, lo, hi, 2);
        let core = (p[0].clone() - core_t.clone() * d[0]).square()
            + (p[1].clone() - core_t * d[1]).square()
            - major * major;
        torus.min(sphere).min(core)
    };
    let band = choose(outside.compare(0.0).max(0.0), outside, field);
    let (lo, hi, outside) = band_interval(z.clone(), corner, length, d[2]);
    let t = quadratic_min(&p, d, lo, hi, 2);
    let cylinder = (p[0].clone() - t.clone() * d[0]).square() + (p[1].clone() - t * d[1]).square()
        - radius * radius;
    // Only the whole section gets capped: no zero-valued internal join sheet.
    let cylinder = choose(outside.compare(0.0).max(0.0), outside, cylinder);
    band.min(cylinder)
        .max(d[2].min(0.0) - z.clone())
        .max(z - (length + d[2].max(0.0)))
}

/// Independent numerical oracle: this convex field's minimum along a line.
/// No profile chords or polynomial roots. This is not a production algorithm.
fn numerical_bull(
    p: [f32; 3],
    radius: f64,
    corner: f64,
    length: f64,
    a: [f32; 3],
    b: [f32; 3],
) -> f64 {
    let at = |t: f64| {
        let p: [f64; 3] = std::array::from_fn(|i| {
            f64::from(p[i]) - f64::from(a[i]) - t * (f64::from(b[i]) - f64::from(a[i]))
        });
        ((p[0].hypot(p[1]) - (radius - corner))
            .max(0.0)
            .hypot((p[2] - corner).min(0.0))
            - corner)
            .max(-p[2])
            .max(p[2] - length)
    };
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..80 {
        let left = lo + (hi - lo) / 3.0;
        let right = hi - (hi - lo) / 3.0;
        if at(left) < at(right) {
            hi = right;
        } else {
            lo = left;
        }
    }
    at(0.0).min(at(1.0)).min(at((lo + hi) / 2.0))
}

/// Probe either side of the swept boundary, found independently by ray searches
/// from an interior point. This is a convex bull mill, not a general profile.
fn boundary_points(corner: f32, b: [f32; 3]) -> Vec<[f32; 3]> {
    let center: [f32; 3] = std::array::from_fn(|i| b[i] / 2.0 + if i == 2 { 0.6 } else { 0.0 });
    let mut points = Vec::new();
    for i in 0..64 {
        let z = 2.0 * (i as f32 + 0.5) / 64.0 - 1.0;
        let angle = i as f32 * 2.3999631;
        let v = [
            (1.0 - z * z).sqrt() * angle.cos(),
            (1.0 - z * z).sqrt() * angle.sin(),
            z,
        ];
        let at = |t: f32| std::array::from_fn(|i| center[i] + t * v[i]);
        let (mut lo, mut hi) = (0.0, 10.0);
        assert!(numerical_bull(at(hi), 0.5, corner.into(), 1.2, [0.0; 3], b) > 0.0);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if numerical_bull(at(mid), 0.5, corner.into(), 1.2, [0.0; 3], b) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        for delta in [-0.003, -0.0001, 0.0001, 0.003] {
            points.push(at((lo + hi) / 2.0 + delta));
        }
    }
    points
}

#[test]
#[ignore = "compares a closed-form toroidal sweep experiment with profile chords and a numerical oracle"]
fn bull_sweep_experiment() {
    let grid: Vec<_> = (-12..=12)
        .flat_map(|x| {
            (-9..=9).flat_map(move |y| {
                (-5..=15).map(move |z| {
                    [
                        x as f32 * 0.1 + 0.003,
                        y as f32 * 0.1 + 0.007,
                        z as f32 * 0.1 + 0.011,
                    ]
                })
            })
        })
        .collect();
    let mut analytic_failures = 0;
    for corner in [0.001_f32, 0.1, 0.3, 0.499] {
        for b in [
            [0.0; 3],
            [0.0, 0.0, 0.8],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.5],
            [0.01, 0.0, 0.005],
            [0.0001, 0.0, 0.00005],
            [-1.0, 0.3, -0.5],
        ] {
            let a = [0.0; 3];
            let mut points = grid.clone();
            points.extend(boundary_points(corner, b));
            // The join between the rounded band and cylinder is interior.
            points.push([0.1, 0.0, corner]);
            let tool = Tool::bull(1.0, corner.into(), 1.2).unwrap();
            for (name, tree) in [
                ("analytic", analytic_bull(0.5, corner, 1.2, a, b)),
                (
                    "chords",
                    tool.sweep(a.map(f64::from), b.map(f64::from), Axis::Z, 0.001)
                        .unwrap()
                        .unwrap(),
                ),
            ] {
                let start = Instant::now();
                let shape = VmShape::from(tree);
                let tape = shape.ez_float_slice_tape();
                let compile = start.elapsed();
                let xyz: [Vec<f32>; 3] =
                    std::array::from_fn(|i| points.iter().map(|p| p[i]).collect());
                let mut eval = VmShape::new_float_slice_eval();
                let start = Instant::now();
                let actual = eval
                    .eval(&tape, &xyz[0], &xyz[1], &xyz[2])
                    .unwrap()
                    .to_vec();
                let elapsed = start.elapsed();
                let mut wrong = 0;
                let mut nonfinite = 0;
                let mut worst = 0.0_f64;
                for (&point, &value) in points.iter().zip(&actual) {
                    nonfinite += usize::from(!value.is_finite());
                    let expected = numerical_bull(point, 0.5, corner.into(), 1.2, a, b);
                    let margin = if name == "analytic" { 0.00001 } else { 0.002 };
                    if (value < 0.0) != (expected < 0.0) && expected.abs() > margin {
                        wrong += 1;
                        worst = worst.max(expected.abs());
                    }
                }
                eprintln!(
                    "{name} corner={corner} move={b:?}: compile {compile:?}, {} samples {elapsed:?}, wrong {wrong}, nonfinite {nonfinite}, worst field margin {worst}",
                    points.len()
                );
                if name == "chords" {
                    assert_eq!((wrong, nonfinite), (0, 0));
                } else {
                    analytic_failures += wrong + nonfinite;
                }
            }
        }
    }
    assert_eq!(
        analytic_failures, 0,
        "candidate is not production-ready if this fails"
    );
}

#[test]
#[ignore = "measures real interval/gradient meshing, not just bulk point evaluation"]
fn bull_sweep_mesh_experiment() {
    use fidget_engine::mesh::{Octree, Settings};
    use nalgebra::{Scale3, Translation3};
    let a = [0.0; 3];
    let b = [1.0, 0.0, 0.5];
    let tool = Tool::bull(1.0, 0.1, 1.2).unwrap();
    let mut analytic_nonfinite = 0;
    for (name, tree) in [
        ("analytic", analytic_bull(0.5, 0.1, 1.2, a, b)),
        (
            "chords",
            tool.sweep(a.map(f64::from), b.map(f64::from), Axis::Z, 0.001)
                .unwrap()
                .unwrap(),
        ),
    ] {
        let shape = VmShape::from(tree).try_into().unwrap();
        for depth in [5, 6] {
            let settings = Settings {
                depth,
                world_to_model: Translation3::new(0.5, 0.0, 0.85).to_homogeneous()
                    * Scale3::new(1.1, 0.6, 0.95).to_homogeneous(),
                ..Default::default()
            };
            let start = Instant::now();
            let mesh = Octree::build(&shape, &settings).unwrap().walk_dual();
            eprintln!(
                "{name} mesh depth {depth}: {:?}, {} vertices, {} triangles",
                start.elapsed(),
                mesh.vertices.len(),
                mesh.triangles.len()
            );
            assert!(!mesh.vertices.is_empty());
            let nonfinite = mesh
                .vertices
                .iter()
                .filter(|v| !v.iter().all(|x| x.is_finite()))
                .count();
            eprintln!("{name} mesh depth {depth}: {nonfinite} non-finite vertices");
            if name == "analytic" {
                analytic_nonfinite += nonfinite;
            } else {
                assert_eq!(nonfinite, 0);
            }
        }
    }
    // Deliberately a promotion gate: point classification alone is insufficient.
    assert_eq!(
        analytic_nonfinite, 0,
        "do not promote this candidate with invalid mesh vertices"
    );
}
