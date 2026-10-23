use super::*;
use fidget_engine::{shape::EzShape, vm::VmShape};

mod analytic;
mod shoulders;

fn samples(tree: Tree, points: &[[f32; 3]]) -> Vec<f32> {
    let shape = VmShape::from(tree);
    let tape = shape.ez_float_slice_tape();
    let mut eval = VmShape::new_float_slice_eval();
    let xyz: [Vec<f32>; 3] = std::array::from_fn(|i| points.iter().map(|p| p[i]).collect());
    eval.eval(&tape, &xyz[0], &xyz[1], &xyz[2])
        .unwrap()
        .to_vec()
}

#[test]
fn square_and_taper_sweeps_match_dense_placements_including_axial_motion() {
    let points: Vec<_> = (-5..=5)
        .flat_map(|x| {
            (-4..=4).flat_map(move |y| {
                (-3..=9).map(move |z| {
                    [
                        x as f32 * 0.17 + 0.007,
                        y as f32 * 0.19 + 0.003,
                        z as f32 * 0.13 + 0.011,
                    ]
                })
            })
        })
        .collect();
    for (r0, r1) in [(0.25, 0.25), (0.1, 0.4), (0.4, 0.1)] {
        let tool = Tool::new(vec![Section::taper(
            Point::new(r0, 0.0),
            Point::new(r1, 0.5),
            SectionKind::Cutting,
        )])
        .unwrap();
        for (a, b) in [
            ([0.0; 3], [0.0; 3]),
            ([-0.4, -0.3, 0.0], [0.5, 0.4, 0.2]),
            ([0.0; 3], [0.0, 0.0, 0.8]),
            ([-0.3, 0.0, 0.6], [0.4, 0.0, -0.2]),
        ] {
            let actual = samples(tool.sweep(a, b, Axis::Z, 0.001).unwrap().unwrap(), &points);
            for (p, v) in points.iter().zip(actual) {
                let mut expected = f64::INFINITY;
                for i in 0..=4000 {
                    let q: Point3 = std::array::from_fn(|k| {
                        f64::from(p[k]) - a[k] - (b[k] - a[k]) * i as f64 / 4000.0
                    });
                    let radius = r0 + (r1 - r0) * q[2] / 0.5;
                    expected = expected.min(
                        (q[0] * q[0] + q[1] * q[1] - radius * radius)
                            .max(-q[2])
                            .max(q[2] - 0.5),
                    );
                }
                if expected.abs() > 1e-4 {
                    assert_eq!(
                        v < 0.0,
                        expected < 0.0,
                        "{r0}/{r1} {a:?} {b:?} {p:?}: {v} / {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn profile_joints_are_not_internal_cap_surfaces() {
    let tool = Tool::bull(0.25, 0.05, 0.4).unwrap();
    let outline = tool.sections()[0].outline(0.001).unwrap();
    let points: Vec<_> = outline
        .iter()
        .skip(1)
        .take(outline.len() - 2)
        .map(|p| [0.0, 0.0, p.axial as f32])
        .collect();
    assert!(
        samples(
            tool.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001)
                .unwrap()
                .unwrap(),
            &points
        )
        .iter()
        .all(|v| *v < 0.0)
    );
}

#[test]
fn non_cutting_shanks_and_gaps_are_not_swept_into_stock() {
    let tool = Tool::new(vec![
        Section::taper(
            Point::new(0.1, 0.0),
            Point::new(0.1, 0.2),
            SectionKind::Cutting,
        ),
        Section::taper(
            Point::new(0.3, 0.2),
            Point::new(0.4, 0.5),
            SectionKind::NonCutting,
        ),
        Section::taper(
            Point::new(0.1, 0.6),
            Point::new(0.1, 0.7),
            SectionKind::Cutting,
        ),
    ])
    .unwrap();
    let values = samples(
        tool.sweep([-0.4, 0.0, 0.0], [0.4, 0.0, 0.0], Axis::Z, 0.001)
            .unwrap()
            .unwrap(),
        &[
            [0.0, 0.0, 0.1],
            [0.0, 0.0, 0.4],
            [0.0, 0.0, 0.65],
            [0.0, 0.2, 0.1],
        ],
    );
    assert!(values[0] < 0.0 && values[1] > 0.0 && values[2] < 0.0 && values[3] > 0.0);
    let shank = tool.sections()[1]
        .sweep(0.001, [0.0; 3], [0.0; 3], Axis::Z)
        .unwrap();
    assert!(samples(shank, &[[0.2, 0.0, 0.3]])[0] < 0.0);
}
