use super::*;

#[test]
fn axes_are_normalized_and_invalid_axes_do_not_emit() {
    assert!(Axis::new([0.0; 3]).is_none());
    assert!(Axis::new([f64::NAN, 0.0, 1.0]).is_none());
    assert!(Axis::new([f64::INFINITY, 0.0, 1.0]).is_none());
    assert_eq!(Axis::new([0.0, 0.0, f64::MAX]), Some(Axis::Z));
    let axis = Axis::new([3.0, 0.0, 4.0]).unwrap();
    assert_eq!(axis.vector(), [0.6, 0.0, 0.8]);
    let [x, y, z] = axis.basis();
    assert!((x.cross(&y) - z).norm() < 1e-6);

    let stack = crate::stack::load();
    let doc = gid::Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    for vector in [[3.0, 0.0, 4.0], [0.0; 3], [f64::NAN, 0.0, 1.0]] {
        let mut recording = Recording::default();
        let result = run(&mut recording, |scope| {
            ::grap::evaluate_scoped(
                &call(
                    START_AT,
                    [
                        (X, f64::value(0.0)),
                        (Y, f64::value(0.0)),
                        (Z, f64::value(0.0)),
                        (TOOL_AXIS, point_value(vector)),
                    ],
                ),
                &sources,
                scope,
                1000,
            )
        });
        if vector[0] == 3.0 {
            assert_eq!(recording.commands, [Command::StartAt([0.0; 3], axis)]);
        } else {
            assert!(absent::is_absent(&result.result));
            assert!(recording.commands.is_empty());
        }
    }
}

#[test]
fn playback_changes_axis_at_path_breaks_without_an_interpolated_link() {
    let x = Axis::new([1.0, 0.0, 0.0]).unwrap();
    let mut path = Recording::default();
    path.start_at([0.0; 3], Axis::Z).unwrap();
    path.line_to([1.0, 0.0, 0.0]).unwrap();
    path.start_at([0.0; 3], x).unwrap();
    path.line_to([0.0, 1.0, 0.0]).unwrap();
    assert_eq!(path.length().unwrap(), 2.0);
    for (progress, center, axis) in [
        (0.25, [0.5, 0.0, 0.0], Axis::Z),
        (0.5, [1.0, 0.0, 0.0], Axis::Z),
        (0.75, [0.0, 0.5, 0.0], x),
    ] {
        let pose = path
            .playback::<InvalidPath>(progress, |_, _, _, _| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(pose, Pose { tip: center, axis });
    }
    let mut mapped = Recording::default();
    path.replay(&mut MapPoints {
        sink: &mut mapped,
        map: |p: Point3| Ok(p.map(|v| v + 5.0)),
    })
    .unwrap();
    assert_eq!(
        mapped
            .segments()
            .map(|(_, _, axis)| axis)
            .collect::<Vec<_>>(),
        [Axis::Z, x]
    );
}

fn rotate(face: usize, [x, y, z]: Point3) -> Point3 {
    match face {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [x, z, -y],
        3 => [-z, y, x],
        4 => [x, -z, y],
        5 => [x, -y, -z],
        _ => unreachable!(),
    }
}

fn example_program(name: &str) -> Recording {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::apply_scoped(&names[name].into(), [], &sources, scope, 3_000_000)
    });
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    path
}

#[test]
fn tilt_accepts_finite_angles_without_a_machining_policy_range() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    for angle in [
        -45.0,
        0.0,
        10.0,
        60.0,
        90.0,
        135.0,
        180.0,
        360.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        let mut doc = original.clone();
        doc.cells.set_value(names["tilt"], f64::value(angle));
        let sources = crate::sources::Sources {
            doc: &doc,
            libraries: &stack.libraries,
        };
        let mut path = Recording::default();
        let result = run(&mut path, |scope| {
            ::grap::apply_scoped(&names["ball_path"].into(), [], &sources, scope, 500_000)
        });
        assert!(result.completed);
        if angle.is_finite() {
            assert!(!absent::is_absent(&result.result));
            let Command::StartAt(_, axis) = path.commands[0] else {
                panic!("first path")
            };
            assert!((axis.vector()[2] - angle.to_radians().cos()).abs() < 1e-12);
            for command in &path.commands {
                let point = match command {
                    Command::StartAt(point, axis) => {
                        assert!(axis.vector().into_iter().all(f64::is_finite));
                        if angle == 0.0 {
                            assert_eq!(*axis, Axis::Z);
                        }
                        point
                    }
                    Command::LineTo(point) => point,
                };
                assert!(point.iter().all(|v| v.is_finite()));
            }
        } else {
            assert!(absent::is_absent(&result.result));
            assert!(
                path.commands.is_empty(),
                "invalid tilt must not emit cutting moves"
            );
        }
    }
}

#[test]
fn zero_tilt_runs_both_operations_without_changing_ball_center_paths() {
    let (mut doc, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let mut paths = Vec::new();
    for angle in [45.0, 0.0] {
        doc.cells.set_value(names["tilt"], f64::value(angle));
        let sources = crate::sources::Sources {
            doc: &doc,
            libraries: &stack.libraries,
        };
        let mut path = Recording::default();
        let result = run(&mut path, |scope| {
            ::grap::apply_scoped(
                &names["preview_operations"].into(),
                [],
                &sources,
                scope,
                3_000_000,
            )
        });
        assert!(result.completed && !absent::is_absent(&result.result));
        let mut axis = Axis::Z;
        paths.push(
            path.commands
                .iter()
                .map(|command| {
                    let point = match command {
                        Command::StartAt(point, next_axis) => {
                            axis = *next_axis;
                            if angle == 0.0 {
                                let components = axis.vector().map(f64::abs);
                                assert_eq!(components.into_iter().filter(|v| *v == 1.0).count(), 1);
                                assert_eq!(components.into_iter().filter(|v| *v == 0.0).count(), 2);
                            }
                            point
                        }
                        Command::LineTo(point) => point,
                    };
                    std::array::from_fn::<_, 3, _>(|i| point[i] + 0.0625 * axis.vector()[i])
                })
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(paths[0].len(), paths[1].len());
    assert!(!paths[0].is_empty());
    for (a, b) in paths[0].iter().zip(&paths[1]) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-12);
        }
    }
}

fn assert_face_copies(path: &Recording, faces: &[usize]) {
    let (top, _) = example_ball_path();
    assert_eq!(path.commands.len(), top.commands.len() * faces.len());
    assert_eq!(
        path.commands
            .iter()
            .filter(|c| matches!(c, Command::StartAt(..)))
            .count(),
        42 * faces.len()
    );
    for (&face, commands) in faces
        .iter()
        .zip(path.commands.chunks_exact(top.commands.len()))
    {
        // Ordering may reverse for access and climb, but the sampled geometry
        // must remain the same. Quantize only for this floating-point comparison.
        let points = |commands: &[Command], rotation: usize| {
            let mut axis = Axis::Z;
            let mut points: Vec<_> = commands
                .iter()
                .map(|c| {
                    let (Command::StartAt(p, _) | Command::LineTo(p)) = *c;
                    if let Command::StartAt(_, next) = c {
                        axis = *next;
                    }
                    let center = std::array::from_fn(|i| p[i] + 0.0625 * axis.vector()[i]);
                    rotate(rotation, center).map(|x| (x * 1e10).round() as i64)
                })
                .collect();
            points.sort();
            points
        };
        assert_eq!(points(commands, 0), points(&top.commands, face));
        assert_pull_and_climb(commands, face);
    }
}

fn assert_pull_and_climb(commands: &[Command], face: usize) {
    use nalgebra::Vector3;
    let normal = Vector3::from(rotate(face, [0.0, 0.0, 1.0]));
    let up = if face == 5 {
        -Vector3::z()
    } else {
        Vector3::z()
    };
    let mut passes: Vec<(Axis, Vec<Point3>)> = Vec::new();
    for command in commands {
        match *command {
            Command::StartAt(p, axis) => passes.push((axis, vec![p])),
            Command::LineTo(p) => passes.last_mut().unwrap().1.push(p),
        }
    }
    assert_eq!(passes.len(), 42);
    for family in passes.chunks_exact(21) {
        let mut previous_midpoint = None;
        for (axis, points) in family {
            let axis = Vector3::from(axis.vector());
            assert!(axis.dot(&up) > 0.0, "spindle must point away from the vise");
            let a = Vector3::from(points[0]);
            let b = Vector3::from(*points.last().unwrap());
            let delta = b - a;
            let feed = (delta - normal * delta.dot(&normal)).normalize();
            let expected = (normal + feed) * std::f64::consts::FRAC_1_SQRT_2;
            assert!((axis - expected).norm() < 1e-10, "pull along the lean");
            let midpoint = (a + b) / 2.0;
            if let Some(previous) = previous_midpoint {
                let step: Vector3<f64> = midpoint - previous;
                assert!(
                    step.dot(&feed.cross(&normal)) > 0.0,
                    "clockwise climb row order"
                );
            }
            previous_midpoint = Some(midpoint);
        }
    }
}

// Evaluate actual Fidget subtraction at each face center and the interior.
fn stock_samples(path: &Recording) -> Vec<f32> {
    let tool = super::super::cutter::Tool::ball(0.125, 0.22).unwrap();
    let mut stock = super::super::stock::Stock::block([-0.5; 3], [0.5; 3]).unwrap();
    for (a, b, axis) in path.segments() {
        stock.cut(&tool, a, b, axis, 0.001).unwrap();
    }
    use fidget_engine::{shape::EzShape, vm::VmShape};
    let shape = VmShape::from(stock.into_field());
    let mut evaluator = VmShape::new_float_slice_eval();
    let points: Vec<Point3> = (0..6)
        .map(|f| rotate(f, [0.0, 0.0, 0.49]))
        .chain([[0.0; 3]])
        .collect();
    let xyz: [Vec<f32>; 3] = std::array::from_fn(|i| points.iter().map(|p| p[i] as f32).collect());
    let tape = shape.ez_float_slice_tape();
    evaluator
        .eval(&tape, &xyz[0], &xyz[1], &xyz[2])
        .unwrap()
        .to_vec()
}

#[test]
fn op1_copies_the_compensated_paths_and_tilt_to_five_faces_leaving_the_bottom() {
    let path = example_program("op1");
    assert_face_copies(&path, &[0, 1, 2, 3, 4]);
    let values = stock_samples(&path);
    assert!(values[..5].iter().all(|v| *v > 0.0), "{values:?}");
    assert!(values[5..].iter().all(|v| *v < 0.0), "{values:?}");
}

#[test]
fn op2_is_an_independent_bottom_program_and_preview_combines_without_a_link() {
    let op1 = example_program("op1");
    let op2 = example_program("op2");
    assert_face_copies(&op2, &[5]);
    let values = stock_samples(&op2);
    assert!(values[..5].iter().all(|v| *v < 0.0), "{values:?}");
    assert!(values[5] > 0.0 && values[6] < 0.0, "{values:?}");

    let preview = example_program("preview_operations");
    assert_eq!(
        preview.commands,
        op1.commands
            .iter()
            .chain(&op2.commands)
            .copied()
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        preview.commands[op1.commands.len()],
        Command::StartAt(..)
    ));
    assert!(
        (preview.length().unwrap() - op1.length().unwrap() - op2.length().unwrap()).abs() < 1e-10
    );
    assert_eq!(preview.segments().count(), 6336);
    let values = stock_samples(&preview);
    assert!(values[..6].iter().all(|v| *v > 0.0), "{values:?}");
    assert!(values[6] < 0.0, "the cube interior must remain: {values:?}");
}
