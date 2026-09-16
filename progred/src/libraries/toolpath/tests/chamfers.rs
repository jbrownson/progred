use super::super::{
    cutter::{Point, Section, SectionKind, Tool},
    stock::Stock,
};
use super::*;
use crate::{gid_text::Binders, sources::Sources};
use fidget_engine::{shape::EzShape, vm::VmShape};
use nalgebra::Vector3;

fn program(sources: &Sources<'_>, names: &Binders, name: &str) -> Recording {
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::apply_scoped(&names[name].into(), [], sources, scope, 100_000)
    });
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    path
}

fn square_tool(sources: &Sources<'_>, names: &Binders) -> Tool {
    let result = ::grap::evaluate(&names["square_tool"].into(), sources, 1000);
    assert!(result.completed);
    Tool::read(&result.result).unwrap()
}

fn set_strategy(doc: &mut gid::Document, names: &Binders, strategy: &str) {
    let path = [
        gid::Step::Key(names["body"]),
        gid::Step::Key(names["expression"]),
        gid::Step::Key(names["strategy"]),
    ];
    let value = crate::spine::set(
        doc.cells.value(names["cube_chamfers"]),
        &path,
        names[strategy].into(),
    )
    .unwrap();
    doc.cells.set_value(names["cube_chamfers"], value);
}

#[test]
fn chamfer_groups_cover_each_edge_once_with_the_shared_square_tool_dimensions() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for (size, chamfer, diameter, length) in [(1.0, 0.1, 0.125, 0.22), (0.9, 0.14, 0.16, 0.3)] {
        let mut doc = original.clone();
        set_strategy(&mut doc, &names, "contour_chamfer");
        geometry::set_parameter(&mut doc, &names, "size", size);
        geometry::set_parameter(&mut doc, &names, "chamfer", chamfer);
        doc.cells
            .set_value(names["square_diameter"], f64::value(diameter));
        doc.cells
            .set_value(names["cutting_length"], f64::value(length));
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let tool = square_tool(&sources, &names);
        let cutting = &tool.sections[0];
        assert_eq!(
            cutting,
            &Section::taper(
                Point::new(diameter / 2.0, 0.0),
                Point::new(diameter / 2.0, length),
                SectionKind::Cutting,
            )
        );
        let mut normals = Vec::new();
        for (name, count) in [("op1_chamfers", 8), ("op2_chamfers", 4)] {
            let path = program(&sources, &names, name);
            assert_eq!(path.commands().len(), count * 2);
            assert_eq!(path.segments().count(), count);
            assert!((path.length().unwrap() - size * count as f64).abs() < 1e-12);
            for (a, b, axis, active) in path.tool_segments() {
                assert_eq!(active, Some(&tool));
                let feed = (Vector3::from(b) - Vector3::from(a)).normalize();
                let axis = Vector3::from(axis.vector());
                let normal = feed.cross(&axis);
                assert!((normal.norm() - 1.0).abs() < 1e-12);
                assert!(normal.dot(&axis).abs() < 1e-12);
                let signature: [i32; 3] =
                    std::array::from_fn(|i| (normal[i] * 2.0_f64.sqrt()).round() as i32);
                assert_eq!(signature.iter().filter(|&&v| v != 0).count(), 2);
                assert!(
                    !normals.contains(&signature),
                    "duplicate edge {signature:?}"
                );
                normals.push(signature);
                assert_eq!(signature[2] < 0, name == "op2_chamfers");
                assert!((axis.z - normal.z).abs() < 1e-12);
                let contact = (Vector3::from(a) + Vector3::from(b)) / 2.0 + axis * (length / 2.0)
                    - normal * (diameter / 2.0);
                let expected =
                    Vector3::from(signature.map(|sign| sign as f64 * (size - chamfer) / 2.0));
                assert!(
                    (contact - expected).norm() < 1e-12,
                    "{contact:?} != {expected:?}"
                );
            }
        }
        assert_eq!(normals.len(), 12);
    }
}

#[test]
fn both_square_strategies_subtract_the_twelve_chamfer_planes_without_gouging() {
    let (mut doc, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for strategy in ["contour_chamfer", "crosswise_chamfer"] {
        set_strategy(&mut doc, &names, strategy);
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let mut stock = Stock::block([-0.5; 3], [0.5; 3]).unwrap();
        for name in ["op1_chamfers", "op2_chamfers"] {
            let path = program(&sources, &names, name);
            for (a, b, axis, tool) in path.tool_segments() {
                stock.cut(tool.unwrap(), a, b, axis, 0.001).unwrap();
            }
        }
        let shape = VmShape::from(stock.into_field());
        let tape = shape.ez_float_slice_tape();
        let mut eval = VmShape::new_float_slice_eval();
        // Offset the grid to avoid testing a sign exactly on a surface.
        let points: Vec<[f32; 3]> = (-20..=20)
            .flat_map(|x| {
                (-20..=20).flat_map(move |y| {
                    (-20..=20).map(move |z| {
                        [
                            x as f32 * 0.026 + 0.001,
                            y as f32 * 0.026 + 0.002,
                            z as f32 * 0.026 + 0.003,
                        ]
                    })
                })
            })
            .collect();
        let xyz: [Vec<f32>; 3] = std::array::from_fn(|i| points.iter().map(|p| p[i]).collect());
        let actual = eval.eval(&tape, &xyz[0], &xyz[1], &xyz[2]).unwrap();
        for (point, &actual) in points.iter().zip(actual) {
            let [x, y, z] = point.map(f32::abs);
            let expected = (x - 0.5)
                .max(y - 0.5)
                .max(z - 0.5)
                .max(x + y - 0.9)
                .max(x + z - 0.9)
                .max(y + z - 0.9);
            if expected.abs() > 1e-5 {
                assert_eq!(
                    actual < 0.0,
                    expected < 0.0,
                    "{strategy}, {point:?}: actual {actual}, expected {expected}"
                );
            }
        }
    }
}

#[test]
fn crosswise_passes_follow_chamfer_width_and_include_both_edge_ends() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for (size, chamfer, stepover, diameter) in [
        (1.0_f64, 0.1_f64, 0.05_f64, 0.125_f64),
        (0.9, 0.14, 0.07, 0.16),
        (1.0, 0.1, 2.0, 0.125),
    ] {
        let mut doc = original.clone();
        geometry::set_parameter(&mut doc, &names, "size", size);
        geometry::set_parameter(&mut doc, &names, "chamfer", chamfer);
        doc.cells
            .set_value(names["chamfer_stepover"], f64::value(stepover));
        doc.cells
            .set_value(names["square_diameter"], f64::value(diameter));
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let tool = square_tool(&sources, &names);
        let rows = (size / stepover).ceil() as usize + 1;
        let mut normals = Vec::new();
        for (name, edges) in [("op1_chamfers", 8), ("op2_chamfers", 4)] {
            let path = program(&sources, &names, name);
            assert_eq!(path.commands().len(), 2 * rows * edges);
            let segments: Vec<_> = path.tool_segments().collect();
            assert_eq!(segments.len(), rows * edges);
            assert!(
                (path.length().unwrap() - (rows * edges) as f64 * chamfer * 2.0_f64.sqrt()).abs()
                    < 1e-10
            );
            for edge in segments.chunks_exact(rows) {
                let mid = |s: &(Point3, Point3, Axis, Option<&Tool>)| {
                    (Vector3::from(s.0) + Vector3::from(s.1)) / 2.0
                };
                let first = mid(&edge[0]);
                let last = mid(edge.last().unwrap());
                let advance = (last - first).normalize();
                assert!(((last - first).norm() - size).abs() < 1e-12);
                let signature = edge[0]
                    .2
                    .vector()
                    .map(|v| (v * 2.0_f64.sqrt()).round() as i32);
                assert_eq!(signature.iter().filter(|&&v| v != 0).count(), 2);
                assert!(!normals.contains(&signature));
                normals.push(signature);
                assert_eq!(signature[2] < 0, name == "op2_chamfers");
                for (index, &(a, b, axis, active)) in edge.iter().enumerate() {
                    assert_eq!(active, Some(&tool));
                    let axis = Vector3::from(axis.vector());
                    let delta = Vector3::from(b) - Vector3::from(a);
                    let feed = delta.normalize();
                    assert!((delta.norm() - chamfer * 2.0_f64.sqrt()).abs() < 1e-12);
                    assert!(feed.dot(&axis).abs() < 1e-12);
                    assert!(
                        (feed.cross(&axis) - advance).norm() < 1e-12,
                        "consistent row direction, no alternating cuts"
                    );
                    let center = (Vector3::from(a) + Vector3::from(b)) / 2.0;
                    assert!(
                        (center - (first + (last - first) * index as f64 / (rows - 1) as f64))
                            .norm()
                            < 1e-12
                    );
                    for (i, sign) in signature.into_iter().enumerate() {
                        if sign != 0 {
                            assert!(
                                (center[i] - sign as f64 * (size - chamfer) / 2.0).abs() < 1e-12
                            );
                        } else {
                            assert!((first[i].abs() - size / 2.0).abs() < 1e-12);
                            assert!((last[i] + first[i]).abs() < 1e-12);
                        }
                    }
                }
                assert!(size / (rows - 1) as f64 <= stepover + 1e-12);
            }
        }
        assert_eq!(normals.len(), 12);
    }
}

#[test]
fn invalid_stepover_fails_before_emitting_but_contours_do_not_consume_it() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for step in [0.0, -0.05, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut doc = original.clone();
        doc.cells
            .set_value(names["chamfer_stepover"], f64::value(step));
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let mut path = Recording::default();
        let result = run(&mut path, |scope| {
            ::grap::apply_scoped(&names["op1_chamfers"].into(), [], &sources, scope, 100_000)
        });
        assert!(result.completed && absent::is_absent(&result.result));
        assert!(path.commands().is_empty());

        set_strategy(&mut doc, &names, "contour_chamfer");
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        assert_eq!(
            program(&sources, &names, "op1_chamfers").segments().count(),
            8
        );
    }
}

#[test]
fn wide_stepover_leaves_real_uncut_strips_instead_of_clamping_it() {
    let (mut doc, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for (stepover, gap_remains) in [(0.3, true), (0.05, false)] {
        doc.cells
            .set_value(names["chamfer_stepover"], f64::value(stepover));
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let path = program(&sources, &names, "op1_chamfers");
        let mut stock = Stock::block([-0.5; 3], [0.5; 3]).unwrap();
        for (a, b, axis, tool) in path.tool_segments() {
            stock.cut(tool.unwrap(), a, b, axis, 0.001).unwrap();
        }
        let shape = VmShape::from(stock.into_field());
        let mut eval = VmShape::new_float_slice_eval();
        let tape = shape.ez_float_slice_tape();
        let values = eval
            .eval(&tape, &[0.125, 0.25], &[0.48; 2], &[0.48; 2])
            .unwrap();
        assert_eq!(values[0] < 0.0, gap_remains);
        assert!(values[1] > 0.0, "the actual row cuts with either stepover");
    }
}

#[test]
fn changing_strategy_and_stepover_invalidates_the_observed_program() {
    use crate::computations::Computations;
    use std::rc::Rc;
    let (mut doc, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let computations = Computations::from_sources(Sources {
        doc: &doc,
        libraries: &libraries,
    });
    let memo = computation::recording(
        &computations,
        computations.runtime.input(names["op1_chamfers"].into()),
        computations.runtime.input(100_000),
    );
    let initial = computations.runtime.read(&memo).unwrap();
    assert_eq!(initial.path().unwrap().segments().count(), 8 * 21);
    assert!(Rc::ptr_eq(
        &initial,
        &computations.runtime.read(&memo).unwrap()
    ));
    for (strategy, step, segments) in [
        ("crosswise_chamfer", 0.1, 8 * 11),
        ("contour_chamfer", 0.1, 8),
        ("crosswise_chamfer", 0.05, 8 * 21),
    ] {
        set_strategy(&mut doc, &names, strategy);
        doc.cells
            .set_value(names["chamfer_stepover"], f64::value(step));
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        let cached = computations.runtime.read(&memo).unwrap();
        let path = cached.path().unwrap();
        assert_eq!(path.segments().count(), segments);
        assert_eq!(
            path,
            &program(
                &Sources {
                    doc: &doc,
                    libraries: &libraries
                },
                &names,
                "op1_chamfers"
            )
        );
        assert!(Rc::ptr_eq(
            &cached,
            &computations.runtime.read(&memo).unwrap()
        ));
    }
}
