use super::{tubes::Tubes, *};

fn draw(sink: &mut impl Sink<Error = InvalidPath>) {
    sink.start_at([0.0; 3], super::super::paths::Axis::Z)
        .unwrap();
    sink.line_to([1.0, 0.5, 0.1]).unwrap();
    sink.start_at([-1.0, 2.0, 0.5], super::super::paths::Axis::Z)
        .unwrap();
    sink.line_to([0.0, 2.5, 0.2]).unwrap();
}

#[test]
fn direct_and_recorded_emissions_build_identical_meshes() {
    let mut direct = Tubes::new(0.1, [200, 120, 50]).unwrap();
    draw(&mut direct);
    let mut recording = Recording::default();
    draw(&mut recording);
    let mut replayed = Tubes::new(0.1, [200, 120, 50]).unwrap();
    recording.replay(&mut replayed).unwrap();
    assert_eq!(direct.geometry.indices, replayed.geometry.indices);
    assert_eq!(
        direct.geometry.vertices.len(),
        replayed.geometry.vertices.len()
    );
    for (a, b) in direct
        .geometry
        .vertices
        .iter()
        .zip(&replayed.geometry.vertices)
    {
        assert_eq!(a.position, b.position);
        assert_eq!(a.color, b.color);
    }
}

#[test]
fn capsules_have_the_requested_radius_for_arbitrary_axes() {
    for end in [
        [1.0, 0.0, 0.0],
        [0.0, -2.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 2.0, 3.0],
    ] {
        let mut tubes = Tubes::new(0.1, [255, 100, 20]).unwrap();
        tubes
            .start_at([0.0; 3], super::super::paths::Axis::Z)
            .unwrap();
        tubes.line_to(end).unwrap();
        let end = Vector3::from(end.map(|v| v as f32));
        for vertex in &tubes.geometry.vertices {
            let t = (vertex.position.dot(&end) / end.norm_squared()).clamp(0.0, 1.0);
            assert!(((vertex.position - t * end).norm() - 0.1).abs() < 1e-6);
            assert_eq!(vertex.color, [1.0, 100.0 / 255.0, 20.0 / 255.0]);
        }
        for t in tubes.geometry.indices.chunks_exact(3) {
            let [a, b, c] =
                [t[0], t[1], t[2]].map(|i| tubes.geometry.vertices[i as usize].position);
            let center = (a + b + c) / 3.0;
            let along = (center.dot(&end) / end.norm_squared()).clamp(0.0, 1.0);
            assert!(
                (b - a).cross(&(c - a)).dot(&(center - along * end)) > 0.0,
                "outward winding"
            );
        }
    }
}

#[test]
fn path_breaks_do_not_emit_links_and_duplicate_points_are_spheres() {
    let mut tubes = Tubes::new(0.1, [255; 3]).unwrap();
    tubes
        .start_at([100.0; 3], super::super::paths::Axis::Z)
        .unwrap();
    assert!(tubes.geometry.vertices.is_empty());
    tubes
        .start_at([0.0; 3], super::super::paths::Axis::Z)
        .unwrap();
    tubes.line_to([0.0; 3]).unwrap();
    let sphere_vertices = tubes.geometry.vertices.len();
    assert!(
        tubes
            .geometry
            .vertices
            .iter()
            .all(|v| (v.position.norm() - 0.1).abs() < 1e-6)
    );
    tubes
        .start_at([10.0; 3], super::super::paths::Axis::Z)
        .unwrap();
    assert_eq!(tubes.geometry.vertices.len(), sphere_vertices);
    tubes.line_to([10.0; 3]).unwrap();
    assert_eq!(tubes.geometry.vertices.len(), 2 * sphere_vertices);
    assert!(
        tubes.geometry.vertices[sphere_vertices..]
            .iter()
            .all(|v| (v.position - Vector3::repeat(10.0)).norm() < 0.101)
    );
}

#[test]
fn invalid_emissions_do_not_change_geometry_or_the_previous_point() {
    for radius in [
        0.0,
        -1.0,
        f64::INFINITY,
        f64::NAN,
        f64::MAX,
        f64::MIN_POSITIVE,
    ] {
        assert!(Tubes::new(radius, [255; 3]).is_none());
    }
    let mut tubes = Tubes::new(0.1, [255; 3]).unwrap();
    assert_eq!(tubes.line_to([0.0; 3]), Err(InvalidPath::MissingStart));
    tubes
        .start_at([0.0; 3], super::super::paths::Axis::Z)
        .unwrap();
    assert_eq!(
        tubes.start_at([f64::NAN; 3], super::super::paths::Axis::Z),
        Err(InvalidPath::NonFinitePoint)
    );
    assert_eq!(
        tubes.line_to([f64::MAX; 3]),
        Err(InvalidPath::CoordinateRange)
    );
    assert!(tubes.geometry.vertices.is_empty());
    tubes.line_to([0.0; 3]).unwrap();
    assert!(
        tubes
            .geometry
            .vertices
            .iter()
            .all(|v| (v.position.norm() - 0.1).abs() < 1e-6)
    );
}

#[test]
fn visible_ball_end_matches_the_subtraction_tool_dimensions() {
    let mut tubes = Tubes::new(0.25, [255; 3]).unwrap();
    tubes
        .tool(
            &super::super::cutter::Tool::ball(0.5, 1.0).unwrap(),
            Pose {
                tip: [0.0, 0.0, -0.25],
                axis: Axis::Z,
            },
            [255; 3],
            0.001,
        )
        .unwrap();
    assert_eq!(tubes.geometry.vertices[0].position.z, -0.25);
    assert_eq!(tubes.geometry.vertices.last().unwrap().position.z, 0.75);
    for v in &tubes.geometry.vertices {
        let p = v.position;
        assert!(p.z <= 0.75 && p.z >= -0.25);
        if p.z < 0.0 {
            assert!((p.norm() - 0.25).abs() < 1e-6);
        } else {
            assert!(p.xy().norm() <= 0.250001);
        }
    }
    for t in tubes.geometry.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| tubes.geometry.vertices[i as usize].position);
        let center = (a + b + c) / 3.0;
        let normal = (b - a).cross(&(c - a));
        let outward = if a.z == 0.75 && b.z == 0.75 && c.z == 0.75 {
            Vector3::z()
        } else if center.z < 0.0 {
            center
        } else {
            Vector3::new(center.x, center.y, 0.0)
        };
        assert!(normal.dot(&outward) > 0.0);
    }
}

#[test]
fn tool_mesh_checks_its_vertices_not_fidget_squared_radius_limits() {
    use super::super::cutter::Tool;
    let pose = Pose {
        tip: [0.0; 3],
        axis: Axis::Z,
    };
    let mut tubes = Tubes::new(0.01, [255; 3]).unwrap();
    let large = Tool::square(1e20, 1e20).unwrap();
    tubes.tool(&large, pose, [255; 3], 0.001).unwrap();
    assert!(!tubes.geometry.indices.is_empty());
    assert!(
        tubes
            .geometry
            .vertices
            .iter()
            .all(|v| v.position.iter().all(|n| n.is_finite()))
    );
    assert!(matches!(
        large.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001),
        Err(InvalidPath::CoordinateRange)
    ));

    let overflow = Tool::square(1e50, 1e50).unwrap();
    assert!(matches!(
        tubes.tool(&overflow, pose, [255; 3], 0.001),
        Err(InvalidPath::CoordinateRange)
    ));
}

#[test]
fn tilted_tool_vertices_lie_on_the_same_implicit_cutter() {
    use fidget_engine::{shape::EzShape, vm::VmShape};
    let tool = super::super::cutter::Tool::ball(0.125, 0.22).unwrap();
    let pose = Pose {
        tip: [0.1, -0.2, 0.4],
        axis: Axis::new([1.0, -1.0, 2.0]).unwrap(),
    };
    // Line thickness is intentionally unrelated to the tool diameter.
    let mut tubes = Tubes::new(0.001, [255; 3]).unwrap();
    tubes.tool(&tool, pose, [225, 94, 58], 0.00025).unwrap();
    let shape = VmShape::from(
        tool.sweep(pose.tip, pose.tip, pose.axis, 0.001)
            .unwrap()
            .unwrap(),
    );
    let tape = shape.ez_float_slice_tape();
    let mut evaluator = VmShape::new_float_slice_eval();
    let xyz: [Vec<f32>; 3] = std::array::from_fn(|i| {
        tubes
            .geometry
            .vertices
            .iter()
            .map(|v| v.position[i])
            .collect()
    });
    assert!(
        evaluator
            .eval(&tape, &xyz[0], &xyz[1], &xyz[2])
            .unwrap()
            .iter()
            .all(|v| v.abs() < 1e-6)
    );
}

#[test]
fn explicit_shoulders_make_annular_faces_with_outward_winding() {
    use super::super::cutter::{Point, Section, SectionKind, Segment, Tool};
    for (lower, upper) in [(0.25, 0.5), (0.5, 0.25)] {
        let tool = Tool::new(vec![Section {
            kind: SectionKind::Cutting,
            start: Point::new(lower, 0.0),
            profile: vec![
                Segment::Line(Point::new(lower, 0.5)),
                Segment::Shoulder(upper),
                Segment::Line(Point::new(upper, 1.0)),
            ],
        }])
        .unwrap();
        let mut tubes = Tubes::new(0.01, [255; 3]).unwrap();
        tubes
            .tool(
                &tool,
                Pose {
                    tip: [0.0; 3],
                    axis: Axis::Z,
                },
                [255; 3],
                0.001,
            )
            .unwrap();
        let mut shoulder_triangles = 0;
        for t in tubes.geometry.indices.chunks_exact(3) {
            let [a, b, c] =
                [t[0], t[1], t[2]].map(|i| tubes.geometry.vertices[i as usize].position);
            if [a, b, c].iter().all(|p| p.z == 0.5) {
                shoulder_triangles += 1;
                assert!(
                    [a, b, c].iter().all(|p| (p.xy().norm() - 0.25).abs() < 1e-6
                        || (p.xy().norm() - 0.5).abs() < 1e-6),
                    "annulus, not a disk"
                );
                assert!((b - a).cross(&(c - a)).z * (lower - upper) as f32 > 0.0);
            }
        }
        assert_eq!(shoulder_triangles, 24);
    }
}

#[test]
fn mesh_preview_validates_depth_and_retains_an_ordinary_model_declaration() {
    let stack = crate::stack::load();
    for (depth, valid) in [(5, true), (0, false), (9, false)] {
        let result = ::grap::evaluate(
            &::grap::call(
                PREVIEW_MESH.into(),
                [
                    (PROGRAM, ::grap::lambda([], Value::record([]))),
                    (LINE_RADIUS, f64::value(0.01)),
                    (
                        presentation::vocabulary::VALUE,
                        crate::libraries::f32::value(1.0),
                    ),
                    (
                        fidget::vocabulary::MESH_DEPTH,
                        crate::libraries::u64::value(depth),
                    ),
                    (
                        fidget::vocabulary::COLOR,
                        Value::record([(
                            crate::libraries::color::vocabulary::RGB,
                            vec![255, 128, 0].into(),
                        )]),
                    ),
                ],
            ),
            &stack.libraries,
            1000,
        );
        assert!(result.completed);
        if valid {
            let fields = result
                .result
                .as_record()
                .unwrap()
                .get(&PREVIEW_MESH)
                .unwrap()
                .as_record()
                .unwrap();
            let (model, actual) =
                fidget::mesh::read(fields.get(&presentation::vocabulary::VALUE).unwrap()).unwrap();
            assert_eq!(actual, depth as u8);
            assert_eq!(model.objects.len(), 1);
        } else {
            assert_eq!(
                result.result,
                absent::with_reason(fidget::vocabulary::INVALID_MESH_DEPTH)
            );
        }
    }
}
