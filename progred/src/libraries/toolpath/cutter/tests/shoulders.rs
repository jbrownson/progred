use super::*;

fn stepped(r0: f64, r1: f64) -> Tool {
    Tool::new(vec![Section {
        kind: SectionKind::Cutting,
        start: Point::new(r0, 0.0),
        profile: vec![
            Segment::Line(Point::new(r0, 0.5)),
            Segment::Shoulder(r1),
            Segment::Line(Point::new(r1, 1.0)),
        ],
    }])
    .unwrap()
}

#[test]
fn shoulder_inherits_height_and_survives_value_roundtrip() {
    use vocabulary::*;
    let tool = stepped(0.125, 0.25);
    let outline = tool.sections[0].outline(0.001).unwrap();
    assert_eq!(
        outline,
        [
            Point::new(0.125, 0.0),
            Point::new(0.125, 0.5),
            Point::new(0.25, 0.5),
            Point::new(0.25, 1.0),
        ]
    );
    let value = tool.value();
    let section = value
        .as_record()
        .unwrap()
        .get(&TOOL)
        .unwrap()
        .as_list()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&CUTTING)
        .unwrap()
        .as_list()
        .unwrap();
    assert_eq!(
        section.values().nth(2).unwrap(),
        &Value::record([(RADIUS, f64::value(0.25))])
    );
    assert_eq!(Tool::read(&value), Some(tool.clone()));
}

#[test]
fn radius_only_and_explicit_same_height_moves_lower_to_shoulders() {
    use vocabulary::*;
    let tool = stepped(0.125, 0.25);
    let read = |segment: Value| {
        let value = tool.value();
        let mut profile = value
            .as_record()
            .unwrap()
            .get(&TOOL)
            .unwrap()
            .as_list()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .as_record()
            .unwrap()
            .get(&CUTTING)
            .unwrap()
            .as_list()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        profile[2] = segment;
        Tool::read(&Value::record([(
            TOOL,
            Value::list([Value::record([(CUTTING, Value::list(profile))])]),
        )]))
    };
    for bad in [
        Value::record([(RADIUS, Value::record([]))]),
        Value::record([(RADIUS, f64::value(-0.1))]),
        Value::record([(RADIUS, f64::value(f64::NAN))]),
    ] {
        assert!(read(bad).is_none());
    }
    assert!(
        read(Value::record([
            (RADIUS, f64::value(0.25)),
            (AXIAL, f64::value(0.5)),
            (
                crate::libraries::name::vocabulary::NAME,
                crate::libraries::text::value("step")
            ),
        ]))
        .is_some()
    );

    // These are lowered geometry invariants. The value reader handles leading
    // caps and coalesces radial moves before constructing the native bands.
    for profile in [
        vec![
            Segment::Shoulder(0.25),
            Segment::Line(Point::new(0.25, 1.0)),
        ],
        vec![
            Segment::Line(Point::new(0.125, 0.5)),
            Segment::Shoulder(0.25),
        ],
        vec![
            Segment::Line(Point::new(0.125, 0.5)),
            Segment::Shoulder(0.25),
            Segment::Shoulder(0.3),
            Segment::Line(Point::new(0.3, 1.0)),
        ],
    ] {
        assert!(
            Tool::new(vec![Section {
                profile,
                ..tool.sections[0].clone()
            }])
            .is_none()
        );
    }
}

#[test]
fn separate_same_kind_sections_require_a_gap_not_independent_touching_caps() {
    let section =
        |low, high, kind| Section::taper(Point::new(0.25, low), Point::new(0.25, high), kind);
    for kind in [SectionKind::Cutting, SectionKind::NonCutting] {
        for start in [0.25, 0.5, 0.5 + 1e-10] {
            assert!(Tool::new(vec![section(0.0, 0.5, kind), section(start, 1.0, kind)]).is_none());
        }
        assert!(Tool::new(vec![section(0.6, 1.0, kind), section(0.0, 0.5, kind)]).is_some());
    }
    assert!(
        Tool::new(vec![
            section(0.0, 0.5, SectionKind::Cutting),
            section(0.5, 1.0, SectionKind::NonCutting)
        ])
        .is_some()
    );
}

#[test]
fn shoulder_has_an_exposed_annulus_but_no_internal_disk() {
    for (r0, r1) in [(0.25, 0.5), (0.5, 0.25)] {
        let tool = stepped(r0, r1);
        let tree = tool
            .sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001)
            .unwrap()
            .unwrap();
        let values = samples(
            tree,
            &[
                [0.0, 0.0, 0.5],
                [0.2, 0.0, 0.5],   // shared disk is interior
                [0.375, 0.0, 0.5], // shoulder's actual face
                [0.375, 0.0, 0.49],
                [0.375, 0.0, 0.51],
                [0.6, 0.0, 0.5], // outside both sides
            ],
        );
        assert!(values.iter().all(|v| v.is_finite()));
        assert!(values[0] < 0.0 && values[1] < 0.0, "{values:?}");
        assert_eq!(values[2], 0.0);
        assert_eq!(values[3] < 0.0, r0 > r1);
        assert_eq!(values[4] < 0.0, r1 > r0);
        assert!(values[5] > 0.0);
    }
    // A zero-width shoulder is harmless, not an internal cap.
    let tool = stepped(0.25, 0.25);
    assert!(
        samples(
            tool.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001)
                .unwrap()
                .unwrap(),
            &[[0.0, 0.0, 0.5]]
        )[0] < 0.0
    );
}

#[test]
fn swept_shoulders_match_dense_placements_for_oblique_axial_and_tilted_moves() {
    let points: Vec<_> = (-3..=3)
        .flat_map(|x| {
            (-3..=3).flat_map(move |y| {
                (-2..=8).map(move |z| {
                    [
                        x as f32 * 0.23 + 0.009,
                        y as f32 * 0.21 + 0.005,
                        z as f32 * 0.19 + 0.013,
                    ]
                })
            })
        })
        .collect();
    for (r0, r1) in [(0.25, 0.5), (0.5, 0.25)] {
        // Sloping neighboring walls test that the interior connector never
        // protrudes from either side (it is not a constant-radius cylinder).
        let tool = Tool::new(vec![Section {
            kind: SectionKind::Cutting,
            start: Point::new(0.125, 0.0),
            profile: vec![
                Segment::Line(Point::new(r0, 0.5)),
                Segment::Shoulder(r1),
                Segment::Line(Point::new(0.125, 1.0)),
            ],
        }])
        .unwrap();
        for axis in [Axis::Z, Axis::new([1.0, -1.0, 2.0]).unwrap()] {
            for (a, b) in [
                ([0.0; 3], [0.0; 3]),
                ([-0.4, -0.3, 0.0], [0.5, 0.4, 0.2]),
                ([0.0; 3], [0.0, 0.0, 0.8]),
                ([-0.3, 0.0, 0.6], [0.4, 0.0, -0.2]),
            ] {
                let actual = samples(tool.sweep(a, b, axis, 0.001).unwrap().unwrap(), &points);
                for (p, v) in points.iter().zip(actual) {
                    assert!(v.is_finite());
                    let mut expected = f64::INFINITY;
                    for i in 0..=1000 {
                        let q: Point3 = std::array::from_fn(|k| {
                            f64::from(p[k]) - a[k] - (b[k] - a[k]) * i as f64 / 1000.0
                        });
                        let z = q
                            .into_iter()
                            .zip(axis.vector())
                            .map(|(a, b)| a * b)
                            .sum::<f64>();
                        let rho2 = q.into_iter().map(|x| x * x).sum::<f64>() - z * z;
                        let lower = 0.125 + (r0 - 0.125) * z / 0.5;
                        let upper = r1 + (0.125 - r1) * (z - 0.5) / 0.5;
                        let field = (rho2 - lower * lower)
                            .max(-z)
                            .max(z - 0.5)
                            .min((rho2 - upper * upper).max(0.5 - z).max(z - 1.0));
                        expected = expected.min(field);
                    }
                    // The reference samples time; disregard its near-boundary
                    // uncertainty, not errors in a substantial interior/exterior.
                    if expected.abs() > 0.001 {
                        assert_eq!(
                            v < 0.0,
                            expected < 0.0,
                            "{r0}/{r1} {axis:?} {a:?}->{b:?} {p:?}: {v} / {expected}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn shoulders_between_arcs_and_multiple_steps_keep_their_interiors() {
    let tool = Tool::new(vec![Section {
        kind: SectionKind::Cutting,
        start: Point::new(0.0, 0.0),
        profile: vec![
            Segment::Arc {
                end: Point::new(0.25, 0.25),
                radius: 0.25,
                bend: Bend::Convex,
            },
            Segment::Shoulder(0.5),
            Segment::Arc {
                end: Point::new(0.75, 0.5),
                radius: 0.25,
                bend: Bend::Convex,
            },
            Segment::Shoulder(0.25),
            Segment::Line(Point::new(0.25, 1.0)),
        ],
    }])
    .unwrap();
    for tolerance in [0.01, 0.001] {
        let v = samples(
            tool.sweep([0.0; 3], [0.0; 3], Axis::Z, tolerance)
                .unwrap()
                .unwrap(),
            &[
                [0.1, 0.0, 0.25],
                [0.1, 0.0, 0.5],
                [0.4, 0.0, 0.25],
                [0.6, 0.0, 0.5],
                [0.8, 0.0, 0.5],
                [0.3, 0.0, 0.51],
            ],
        );
        assert!(v[0] < 0.0 && v[1] < 0.0, "{v:?}");
        assert_eq!(&v[2..4], &[0.0, 0.0]);
        assert!(v[4] > 0.0 && v[5] > 0.0);
    }
}

#[test]
fn shoulder_sweeps_mesh_with_finite_vertices_and_no_internal_disk() {
    use fidget_engine::mesh::{Octree, Settings};
    use nalgebra::{Scale3, Translation3};
    for (r0, r1) in [(0.25, 0.5), (0.5, 0.25)] {
        for b in [[0.0; 3], [0.4, 0.0, 0.2]] {
            let shape = VmShape::from(
                stepped(r0, r1)
                    .sweep([0.0; 3], b, Axis::Z, 0.001)
                    .unwrap()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
            let settings = Settings {
                depth: 5,
                world_to_model: Translation3::new(0.2, 0.0, 0.6).to_homogeneous()
                    * Scale3::new(0.8, 0.6, 0.7).to_homogeneous(),
                ..Default::default()
            };
            let mesh = Octree::build(&shape, &settings).unwrap().walk_dual();
            assert!(!mesh.triangles.is_empty());
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.iter().all(|x| x.is_finite()))
            );
            if b == [0.0; 3] {
                // Fidget returns model coordinates: the meshing-volume
                // transform has already been applied to these vertices.
                for p in &mesh.vertices {
                    assert!(
                        !(p.x.hypot(p.y) < 0.15 && p.z > 0.25 && p.z < 0.75),
                        "spurious interior vertex: {p:?}"
                    );
                }
            }
        }
    }
}
