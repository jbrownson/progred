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
        apply_groups(&names[name].into(), [], sources, scope, 300_000)
    });
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    path
}

fn square_tool(sources: &Sources<'_>, names: &Binders) -> Tool {
    let result = ::grap::evaluate_value(&names["square_tool"].into(), sources, 1000);
    assert!(result.completed);
    Tool::read(&result.result).unwrap()
}

fn set_strategy(doc: &mut gid::Document, names: &Binders, strategy: &str) {
    let configured = match strategy {
        "crosswise_chamfer" => names["chamfer_pass"],
        "contour_chamfer" => names["contour_pass"],
        _ => panic!("unknown test recipe"),
    };
    let definition = doc.cells.value(names["cube_chamfers"]).unwrap();
    let bindings = definition
        .as_record()
        .unwrap()
        .get(&names["body"])
        .unwrap()
        .as_record()
        .unwrap()
        .get(&names["bindings"])
        .unwrap()
        .as_list()
        .unwrap();
    let path = [
        gid::Step::Key(names["body"]),
        gid::Step::Key(names["bindings"]),
        gid::Step::Element(bindings.keys().next().unwrap().clone()),
        gid::Step::Key(names["subject"]),
    ];
    let value = crate::spine::set(Some(definition), &path, configured.into()).unwrap();
    doc.cells.set_value(names["cube_chamfers"], value);
}

#[test]
fn configured_recipes_work_on_an_independent_planar_strip() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let curve = ::grap::lambda(
        [names["t"]],
        call(
            POINT,
            [
                (
                    X,
                    call(
                        f64::vocabulary::SUBTRACT,
                        [
                            (number::vocabulary::LEFT, f64::value(10.0)),
                            (
                                number::vocabulary::RIGHT,
                                call(
                                    f64::vocabulary::MULTIPLY,
                                    [
                                        (number::vocabulary::LEFT, f64::value(4.0)),
                                        (number::vocabulary::RIGHT, names["t"].into()),
                                    ],
                                ),
                            ),
                        ],
                    ),
                ),
                (Y, f64::value(20.0)),
                (Z, f64::value(30.0)),
            ],
        ),
    );
    let curve = ::grap::evaluate_value(&curve, &sources, 1000).result;
    let strip = Value::record([
        (names["curve"], curve),
        (names["length"], f64::value(4.0)),
        (names["width"], f64::value(0.3)),
        (names["normal"], point_value([0.0, 1.0, 0.0])),
        (names["across"], point_value([0.0, 0.0, 1.0])),
    ]);
    for (recipe, expected) in [
        (
            call(
                names["crosswise_chamfer"],
                [
                    (names["stepover"], f64::value(2.0)),
                    (names["cutter_diameter"], f64::value(0.2)),
                ],
            ),
            vec![
                ([10.0, 20.0, 29.75], [10.0, 20.0, 30.25], [0.0, 1.0, 0.0]),
                ([8.0, 20.0, 29.75], [8.0, 20.0, 30.25], [0.0, 1.0, 0.0]),
                ([6.0, 20.0, 29.75], [6.0, 20.0, 30.25], [0.0, 1.0, 0.0]),
            ],
        ),
        (
            call(
                names["contour_chamfer"],
                [
                    (names["cutter_diameter"], f64::value(0.2)),
                    (names["contact_height"], f64::value(0.4)),
                ],
            ),
            vec![([10.0, 20.1, 29.6], [6.0, 20.1, 29.6], [0.0, 0.0, 1.0])],
        ),
    ] {
        // Configuration is pure and can be evaluated without any path output.
        let configured = ::grap::evaluate_value(&recipe, &sources, 1000);
        assert!(configured.completed && !absent::is_absent(&configured.result));
        let mut path = Recording::default();
        let result = run(&mut path, |scope| {
            apply_groups(
                &configured.result,
                [(names["strip"], strip.clone())],
                &sources,
                scope,
                10_000,
            )
        });
        assert!(
            result.completed && !absent::is_absent(&result.result),
            "{:?}",
            result.result
        );
        let actual: Vec<_> = path
            .segments()
            .map(|(a, b, axis)| (a, b, axis.vector()))
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn stroke_extension_composes_without_a_tool_or_output_scope() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    // The callback returns its endpoints as ordinary data rather than emitting a path.
    let stroke = ::grap::lambda(
        [names["start"], names["end"]],
        call(
            names["quote"],
            [(
                names["expression"],
                Value::record(["start", "end"].map(|name| {
                    (
                        names[name],
                        Value::record([(names["unquote"], names[name].into())]),
                    )
                })),
            )],
        ),
    );
    let extend = |stroke, before, after| {
        let result = ::grap::evaluate_value(
            &call(
                names["extend_stroke"],
                [
                    (names["stroke"], stroke),
                    (names["start_extension"], f64::value(before)),
                    (names["end_extension"], f64::value(after)),
                ],
            ),
            &sources,
            1000,
        );
        assert!(result.completed && !absent::is_absent(&result.result));
        result.result
    };
    for (before, after) in [(0.0, 0.0), (7.0, 0.0), (0.0, 14.0), (7.0, 14.0)] {
        for nested in [false, true] {
            let configured = if nested {
                extend(
                    extend(stroke.clone(), before / 2.0, after / 2.0),
                    before / 2.0,
                    after / 2.0,
                )
            } else {
                extend(stroke.clone(), before, after)
            };
            for (start, end) in [
                ([1.0, 2.0, 3.0], [3.0, 5.0, 9.0]),
                ([3.0, 5.0, 9.0], [1.0, 2.0, 3.0]),
            ] {
                let result = ::grap::apply_value(
                    &configured,
                    [
                        (names["start"], point_value(start)),
                        (names["end"], point_value(end)),
                    ],
                    &sources,
                    10_000,
                );
                assert!(
                    result.completed && !absent::is_absent(&result.result),
                    "{:?}",
                    result.result
                );
                let fields = result.result.as_record().unwrap();
                let direction = (Vector3::from(end) - Vector3::from(start)).normalize();
                for (name, expected) in [
                    ("start", Vector3::from(start) - before * direction),
                    ("end", Vector3::from(end) + after * direction),
                ] {
                    let actual = read_point(fields.get(&names[name]).unwrap()).unwrap();
                    assert!((Vector3::from(actual) - expected).norm() < 1e-12);
                }
            }
        }
    }
}

#[test]
fn extending_a_zero_length_stroke_fails_without_emitting_a_path() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let configured = ::grap::evaluate_value(
        &call(
            names["extend_stroke"],
            [
                (names["start_extension"], f64::value(0.1)),
                (names["end_extension"], f64::value(0.1)),
                (
                    names["stroke"],
                    ::grap::lambda(
                        [names["start"], names["end"]],
                        call(
                            names["line"],
                            [
                                (names["start"], names["start"].into()),
                                (names["end"], names["end"].into()),
                                (TOOL_AXIS, point_value([0.0, 0.0, 1.0])),
                            ],
                        ),
                    ),
                ),
            ],
        ),
        &sources,
        1000,
    );
    assert!(configured.completed && !absent::is_absent(&configured.result));
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::apply_value_scoped(
            &configured.result,
            [
                (names["start"], point_value([1.0, 2.0, 3.0])),
                (names["end"], point_value([1.0, 2.0, 3.0])),
            ],
            &sources,
            scope,
            10_000,
        )
    });
    assert!(result.completed && absent::is_absent(&result.result));
    assert!(path.commands().is_empty());
}

#[test]
fn spacing_and_rotated_repetition_do_not_require_a_chamfer_or_tool() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let expression = call(
        names["evenly_spaced"],
        [
            (names["length"], f64::value(0.37)),
            (names["stepover"], f64::value(0.1)),
            (
                names["action"],
                ::grap::lambda(
                    [names["t"]],
                    call(
                        START_AT,
                        [
                            (X, names["t"].into()),
                            (Y, f64::value(0.0)),
                            (Z, f64::value(0.0)),
                        ],
                    ),
                ),
            ),
        ],
    );
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::evaluate_value_scoped(
            &call(SEQUENCE, [(PROGRAM, collect_groups(expression.clone()))]),
            &sources,
            scope,
            10_000,
        )
    });
    assert!(result.completed && !absent::is_absent(&result.result));
    assert_eq!(
        path.commands(),
        [0.0, 0.25, 0.5, 0.75, 1.0].map(|t| Command::StartAt([t, 0.0, 0.0], Axis::Z))
    );

    let expression = call(
        names["chamfer_ring"],
        [(
            PROGRAM,
            ::grap::lambda(
                [],
                call(
                    crate::libraries::tree::vocabulary::LEAF,
                    [(
                        crate::libraries::presentation::vocabulary::VALUE,
                        ::grap::lambda(
                            [],
                            call(
                                names["line"],
                                [
                                    (names["start"], point_value([1.0, 0.0, 0.0])),
                                    (names["end"], point_value([2.0, 0.0, 0.0])),
                                    (TOOL_AXIS, point_value([0.0, 1.0, 0.0])),
                                ],
                            ),
                        ),
                    )],
                ),
            ),
        )],
    );
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::evaluate_value_scoped(
            &call(SEQUENCE, [(PROGRAM, collect_groups(expression.clone()))]),
            &sources,
            scope,
            10_000,
        )
    });
    assert!(result.completed && !absent::is_absent(&result.result));
    let actual: Vec<_> = path
        .segments()
        .map(|(a, b, axis)| (a, b, axis.vector()))
        .collect();
    assert_eq!(
        actual,
        vec![
            ([1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 2.0, 0.0], [-1.0, 0.0, 0.0]),
            ([-1.0, 0.0, 0.0], [-2.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            ([0.0, -1.0, 0.0], [0.0, -2.0, 0.0], [1.0, 0.0, 0.0]),
        ]
    );
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
fn crosswise_passes_clear_stock_at_both_ends_and_include_both_edge_ends() {
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
        let stroke_length = chamfer * 2.0_f64.sqrt() + diameter;
        let mut normals = Vec::new();
        for (name, edges) in [("op1_chamfers", 8), ("op2_chamfers", 4)] {
            let path = program(&sources, &names, name);
            assert_eq!(path.commands().len(), 2 * rows * edges);
            let segments: Vec<_> = path.tool_segments().collect();
            assert_eq!(segments.len(), rows * edges);
            assert!((path.length().unwrap() - (rows * edges) as f64 * stroke_length).abs() < 1e-10);
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
                    assert!((delta.norm() - stroke_length).abs() < 1e-12);
                    assert!(feed.dot(&axis).abs() < 1e-12);
                    assert!(
                        (feed.cross(&axis) - advance).norm() < 1e-12,
                        "consistent row direction, no alternating cuts"
                    );
                    let center = (Vector3::from(a) + Vector3::from(b)) / 2.0;
                    // At either endpoint the entire flat cutting cylinder lies
                    // outside one stock face (at most tangent to its boundary).
                    // The spindle axis points outward, so the rest of its cutting
                    // length moves farther from that face, not back into stock.
                    for tip in [a, b] {
                        assert!((0..3).any(|i| {
                            let sign = signature[i] as f64;
                            let radial_extent = diameter / 2.0 * (1.0 - axis[i] * axis[i]).sqrt();
                            sign != 0.0 && sign * tip[i] - radial_extent >= size / 2.0 - 1e-12
                        }));
                    }
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
            apply_groups(&names["op1_chamfers"].into(), [], &sources, scope, 300_000)
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
fn changing_strategy_stepover_and_diameter_invalidates_the_observed_program() {
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
        computations.runtime.input(::grap::lambda(
            [],
            call(
                SEQUENCE,
                [(PROGRAM, collect_groups(call(names["op1_chamfers"], [])))],
            ),
        )),
        computations.runtime.input(100_000),
    );
    let initial = computations.runtime.read(&memo).unwrap();
    assert_eq!(initial.path().unwrap().segments().count(), 8 * 21);
    assert!(Rc::ptr_eq(
        &initial,
        &computations.runtime.read(&memo).unwrap()
    ));
    for (strategy, step, diameter, segments) in [
        ("crosswise_chamfer", 0.1, 0.125, 8 * 11),
        ("crosswise_chamfer", 0.1, 0.16, 8 * 11),
        ("contour_chamfer", 0.1, 0.16, 8),
        ("crosswise_chamfer", 0.05, 0.125, 8 * 21),
    ] {
        set_strategy(&mut doc, &names, strategy);
        doc.cells
            .set_value(names["chamfer_stepover"], f64::value(step));
        doc.cells
            .set_value(names["square_diameter"], f64::value(diameter));
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
