use super::*;
use fidget_engine::{shape::EzShape, vm::VmShape};

fn samples(tree: Tree, points: &[[f32; 3]]) -> Vec<f32> {
    let shape = VmShape::from(tree);
    let mut evaluator = VmShape::new_float_slice_eval();
    let tape = shape.ez_float_slice_tape();
    let coordinates: [Vec<_>; 3] = std::array::from_fn(|i| points.iter().map(|p| p[i]).collect());
    evaluator
        .eval(&tape, &coordinates[0], &coordinates[1], &coordinates[2])
        .unwrap()
        .to_vec()
}

fn grid() -> Vec<[f32; 3]> {
    (-5..=5)
        .flat_map(|x| {
            (-5..=5).flat_map(move |y| {
                (-3..=8).map(move |z| {
                    [
                        x as f32 * 0.17 + 0.007,
                        y as f32 * 0.17 - 0.003,
                        z as f32 * 0.17 + 0.011,
                    ]
                })
            })
        })
        .collect()
}

#[test]
fn stationary_ball_end_has_a_hemisphere_flute_and_flat_top() {
    let tool = BallEnd::new(0.25, 1.0).unwrap();
    let values = samples(
        tool.sweep([0.0; 3], [0.0; 3]).unwrap(),
        &[
            [0.0, 0.0, -0.2],
            [0.2, 0.0, -0.2],
            [0.2, 0.0, 0.7],
            [0.0, 0.0, 0.8],
            [0.0, 0.0, -0.25],
            [0.25, 0.0, 0.5],
            [0.2, 0.0, 0.75],
        ],
    );
    assert!(values[0] < 0.0 && values[1] > 0.0);
    assert!(values[2] < 0.0 && values[3] > 0.0);
    assert_eq!(&values[4..], &[0.0; 3]);
}

#[test]
fn continuous_sweeps_match_dense_full_tool_placements() {
    let tool = BallEnd::new(0.25, 1.0).unwrap();
    let points = grid();
    for (a, b) in [
        ([-0.7, 0.0, 0.0], [0.7, 0.0, 0.0]),
        ([0.0, 0.0, -0.2], [0.0, 0.0, 0.8]),
        ([-0.7, -0.4, 0.0], [0.7, 0.4, 0.5]),
        ([0.3, -0.4, 0.5], [-0.3, 0.4, -0.2]),
    ] {
        let actual = samples(tool.sweep(a, b).unwrap(), &points);
        for (point, actual) in points.iter().zip(actual) {
            assert!(actual.is_finite());
            let expected = (0..=2000)
                .map(|step| {
                    let t = step as f64 / 2000.0;
                    let p: Point3 =
                        std::array::from_fn(|i| f64::from(point[i]) - (a[i] + t * (b[i] - a[i])));
                    let radial = p[0] * p[0] + p[1] * p[1] - 0.25 * 0.25;
                    let ball = radial + p[2] * p[2];
                    ball.min(radial.max(-p[2]).max(p[2] - 0.75))
                })
                .fold(f64::INFINITY, f64::min);
            if expected.abs() > 1e-4 {
                assert_eq!(
                    actual < 0.0,
                    expected < 0.0,
                    "{a:?} → {b:?} at {point:?}: {actual} vs {expected}"
                );
            }
        }
    }
}

#[test]
fn subdivision_and_reversal_preserve_the_swept_solid() {
    let tool = BallEnd::new(0.25, 1.0).unwrap();
    let a = [-0.6, -0.2, 0.1];
    let b = [0.4, 0.3, 0.7];
    let mid = std::array::from_fn(|i| a[i] + 0.37 * (b[i] - a[i]));
    let points = grid();
    let whole = samples(tool.sweep(a, b).unwrap(), &points);
    for tree in [
        tool.sweep(b, a).unwrap(),
        tool.sweep(a, mid).unwrap().min(tool.sweep(mid, b).unwrap()),
    ] {
        for (a, b) in whole.iter().zip(samples(tree, &points)) {
            if a.abs() > 1e-5 && b.abs() > 1e-5 {
                assert_eq!(*a < 0.0, b < 0.0);
            }
        }
    }
}

#[test]
fn subtraction_preserves_roofs_and_supports_through_cuts() {
    let tool = BallEnd::new(0.2, 0.6).unwrap();
    let mut stock = Stock::block([-1.0; 3], [1.0; 3]).unwrap();
    stock.cut(&tool, [-2.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap();
    let values = samples(
        stock.into_field(),
        &[[0.0, 0.0, 0.0], [0.0, 0.0, 0.7], [0.0, 0.0, -0.7]],
    );
    assert!(values[0] > 0.0 && values[1] < 0.0 && values[2] < 0.0);
    let mut stock = Stock::block([-1.0; 3], [1.0; 3]).unwrap();
    stock.cut(&tool, [0.0, 0.0, -2.0], [0.0, 0.0, 2.0]).unwrap();
    assert!(
        samples(stock.into_field(), &[[0.0, 0.0, -0.9], [0.0, 0.0, 0.9]])
            .iter()
            .all(|v| *v > 0.0)
    );
}

#[test]
fn invalid_geometry_does_not_change_stock() {
    assert!(Stock::block([0.0; 3], [0.0; 3]).is_none());
    assert!(Stock::block([f64::NAN; 3], [1.0; 3]).is_none());
    for (radius, length) in [(0.0, 1.0), (0.25, 0.2), (0.25, f64::INFINITY)] {
        assert!(BallEnd::new(radius, length).is_none());
    }
    let tool = BallEnd::new(0.25, 1.0).unwrap();
    let mut stock = Stock::block([-1.0; 3], [1.0; 3]).unwrap();
    assert_eq!(
        stock.cut(&tool, [0.0; 3], [f64::MAX; 3]),
        Err(InvalidPath::CoordinateRange)
    );
    assert_eq!(samples(stock.into_field(), &[[0.0; 3]]), vec![-1.0]);
}
