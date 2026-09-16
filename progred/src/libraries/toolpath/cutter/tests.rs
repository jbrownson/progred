use super::*;
use crate::libraries::{absent, f64};
use fidget_engine::{shape::EzShape, vm::VmShape};
use gid::Value;

mod analytic;
mod encoding;
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
fn constructors_return_editable_profiles_not_shape_tags() {
    use vocabulary::*;
    let stack = crate::stack::load();
    for (function, tool) in [
        (BALL_MILL, Tool::ball(0.125, 0.22).unwrap()),
        (SQUARE_MILL, Tool::square(0.125, 0.22).unwrap()),
        (BULL_MILL, Tool::bull(0.125, 0.01, 0.22).unwrap()),
    ] {
        let value = ::grap::evaluate(
            &::grap::call(
                function.into(),
                [
                    (super::super::TOOL_DIAMETER, f64::value(0.125)),
                    (super::super::TOOL_LENGTH, f64::value(0.22)),
                    (CORNER_RADIUS, f64::value(0.01)),
                ],
            ),
            &stack.libraries,
            1000,
        )
        .result;
        assert_eq!(value, tool.value());
        assert_eq!(
            value.as_record().unwrap().len(),
            1,
            "constructors return geometry only"
        );
        assert_eq!(Tool::read(&value), Some(tool));
        let mut fields = value.as_record().unwrap().clone();
        fields.insert(
            crate::libraries::name::vocabulary::NAME,
            crate::libraries::text::value("user name"),
        );
        assert!(Tool::read(&Value::Record(fields)).is_some());
    }
    let value = ::grap::evaluate(
        &::grap::call(
            SQUARE_MILL.into(),
            [
                (super::super::TOOL_DIAMETER, f64::value(-0.125)),
                (super::super::TOOL_LENGTH, f64::value(0.22)),
            ],
        ),
        &stack.libraries,
        1000,
    )
    .result;
    assert!(absent::is_absent(&value));
}

#[test]
fn section_tags_are_explicit_exclusive_and_open_to_metadata() {
    use vocabulary::*;
    let tool = Tool::square(0.25, 0.4).unwrap();
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
        .unwrap();
    assert!(!section.contains_key(&NON_CUTTING));
    let shape = section.get(&CUTTING).unwrap().clone();
    assert_eq!(shape.as_list().unwrap().len(), 2);
    let name = crate::libraries::name::vocabulary::NAME;
    let read_section = |section| Tool::read(&Value::record([(TOOL, Value::list([section]))]));
    for (tag, kind) in [
        (CUTTING, SectionKind::Cutting),
        (NON_CUTTING, SectionKind::NonCutting),
    ] {
        let section = Value::record([
            (tag, shape.clone()),
            (name, crate::libraries::text::value("section")),
        ]);
        let decoded = read_section(section).unwrap();
        assert_eq!(decoded.sections[0].kind, kind);
        assert_eq!(decoded.sections[0].profile, tool.sections[0].profile);
        assert_eq!(Tool::read(&decoded.value()), Some(decoded));
    }
    for section in [
        Value::record([]),
        Value::record([(CUTTING, shape.clone()), (NON_CUTTING, shape)]),
        Value::record([(CUTTING, crate::libraries::logic::value(true))]),
    ] {
        assert!(read_section(section).is_none());
    }
}

#[test]
fn arcs_use_named_bend_tags_and_reject_ambiguous_segments() {
    use vocabulary::*;
    let read_segment = |start: Point, segment| {
        Tool::read(&Value::record([(
            TOOL,
            Value::list([Value::record([(
                CUTTING,
                Value::list([Value::record([(RADIUS, f64::value(start.radius))]), segment]),
            )])]),
        )]))
    };
    for (bend, tag, radius) in [
        (Bend::Concave, CONCAVE_ARC, 0.2),
        (Bend::Convex, CONVEX_ARC, 0.0),
    ] {
        let start = Point::new(radius, 0.0);
        let end = Point::new(0.1, 0.1);
        let decoded = read_segment(
            start,
            Value::record([
                (tag, f64::value(0.1)),
                (RADIUS, f64::value(end.radius)),
                (AXIAL, f64::value(end.axial)),
                (
                    crate::libraries::name::vocabulary::NAME,
                    crate::libraries::text::value("edge"),
                ),
            ]),
        )
        .unwrap();
        assert_eq!(
            decoded.sections[0].profile,
            vec![Segment::Arc {
                end,
                radius: 0.1,
                bend,
            }]
        );
        assert_eq!(Tool::read(&decoded.value()), Some(decoded.clone()));
        let outline = decoded.sections[0].outline(0.001).unwrap();
        assert_eq!(outline.first(), Some(&start));
        assert_eq!(outline.last(), Some(&end));
        assert!(outline.windows(2).all(|points| match bend {
            Bend::Concave => points[0].radius > points[1].radius,
            Bend::Convex => points[0].radius < points[1].radius,
        }));
        for segment in [
            Value::record([]),
            Value::record([
                (CONCAVE_ARC, f64::value(0.1)),
                (CONVEX_ARC, f64::value(0.1)),
                (AXIAL, f64::value(0.1)),
            ]),
            Value::record([(tag, crate::libraries::logic::value(false))]),
        ] {
            assert!(read_segment(start, segment).is_none());
        }
    }
}

#[test]
fn arc_tolerance_bounds_chord_error_and_preserves_endpoints() {
    let tool = Tool::bull(0.25, 0.05, 0.4).unwrap();
    let definition = tool.value();
    for tolerance in [0.01, 0.001, 0.0001] {
        let points = tool.sections[0].outline(tolerance).unwrap();
        assert_eq!(points[0], Point::new(0.075, 0.0));
        assert_eq!(*points.last().unwrap(), Point::new(0.125, 0.4));
        for pair in points.windows(2).take(points.len() - 2) {
            let mid = Point::new(
                (pair[0].radius + pair[1].radius) / 2.0,
                (pair[0].axial + pair[1].axial) / 2.0,
            );
            let error = 0.05 - (mid.radius - 0.075).hypot(mid.axial - 0.05);
            assert!(error >= -1e-15 && error <= tolerance + 1e-15);
        }
        assert_eq!(tool.value(), definition);
    }
    for tolerance in [0.0, -0.1, f64::NAN, 1e-100] {
        let tool = Tool::ball(1.0, 2.0).unwrap();
        assert!(tool.sections[0].outline(tolerance).is_none());
    }
    let bad = Section {
        kind: SectionKind::Cutting,
        start: Point::new(1.0, 0.0),
        profile: vec![Segment::Arc {
            end: Point::new(0.0, 2.0),
            radius: 1.0,
            bend: Bend::Convex,
        }],
    };
    assert!(Tool::new(vec![bad]).is_none());
}

#[test]
fn profile_validity_and_bounds_do_not_depend_on_sampling() {
    let tool = Tool::new(vec![Section {
        kind: SectionKind::Cutting,
        start: Point::new(1.0, 0.0),
        profile: vec![Segment::Arc {
            end: Point::new(1.0, 2.0),
            radius: 1.0,
            bend: Bend::Convex,
        }],
    }])
    .unwrap();
    let bounds = tool.sections[0].bounds().unwrap();
    assert_eq!(
        (bounds.radius, bounds.min_axial, bounds.max_axial),
        (2.0, 0.0, 2.0)
    );
    let coarse = tool.sections[0].outline(3.0).unwrap();
    assert_eq!(
        coarse.len(),
        2,
        "these endpoints miss the arc's radial maximum"
    );
    assert!(
        tool.sections[0].outline(1e-100).is_none(),
        "unreasonable sampling requests can fail"
    );
    assert_eq!(
        Tool::read(&tool.value()),
        Some(tool.clone()),
        "the geometry is still valid"
    );
    assert!(
        super::view::picture(&tool).is_some(),
        "display chooses its own quality"
    );
}

#[test]
fn valid_profiles_survive_fidget_range_failures() {
    for constructor in [Tool::square, Tool::ball] {
        for (diameter, length) in [
            (1e-30, 1e-29), // squared f32 radius underflows
            (1e30, 1e31),   // squared f32 radius overflows
            (1e50, 1e51),   // coordinates overflow f32
            (1.0, 1e50),    // axial coordinate alone overflows f32
        ] {
            let tool = constructor(diameter, length).unwrap();
            assert_eq!(Tool::read(&tool.value()), Some(tool.clone()));
            assert!(tool.sections[0].outline(diameter * 0.001).is_some());
            assert!(super::view::picture(&tool).is_some());
            assert!(matches!(
                tool.sweep([0.0; 3], [0.0; 3], Axis::Z, diameter * 0.001),
                Err(InvalidPath::CoordinateRange)
            ));
        }
    }
}

#[test]
fn f64_profile_sampling_does_not_require_f32_axial_separation() {
    let tool = Tool::new(vec![Section::taper(
        Point::new(0.1, 0.5),
        Point::new(0.1, 0.5 + 1e-10),
        SectionKind::Cutting,
    )])
    .unwrap();
    assert_eq!(Tool::read(&tool.value()), Some(tool.clone()));
    assert_eq!(tool.sections[0].outline(0.001).unwrap().len(), 2);
    assert!(super::view::picture(&tool).is_some());
    assert!(matches!(
        tool.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001),
        Err(InvalidPath::CoordinateRange)
    ));

    let arc = Tool::new(vec![Section {
        kind: SectionKind::Cutting,
        start: Point::new(1.0, 1.0),
        profile: vec![Segment::Arc {
            end: Point::new(1.0, 1.000001),
            radius: 0.0000005,
            bend: Bend::Convex,
        }],
    }])
    .unwrap();
    assert!(arc.sections[0].outline(1e-10).unwrap().len() > 8);
    assert!(matches!(
        arc.sweep([0.0; 3], [0.0; 3], Axis::Z, 1e-10),
        Err(InvalidPath::CoordinateRange)
    ));
}

#[test]
fn a_gap_that_collapses_in_fidget_does_not_invalidate_the_profile() {
    let tool = Tool::new(vec![
        Section::taper(
            Point::new(0.1, 0.0),
            Point::new(0.1, 0.5),
            SectionKind::Cutting,
        ),
        Section::taper(
            Point::new(0.1, 0.5 + 1e-10),
            Point::new(0.1, 1.0),
            SectionKind::Cutting,
        ),
    ])
    .unwrap();
    assert_eq!(Tool::read(&tool.value()), Some(tool.clone()));
    assert!(super::view::picture(&tool).is_some());
    assert!(matches!(
        tool.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001),
        Err(InvalidPath::CoordinateRange)
    ));
}

#[test]
fn one_tool_can_be_swept_at_different_accuracies() {
    let tool = Tool::bull(1.0, 0.2, 1.0).unwrap();
    let point = [[0.43, 0.0, 0.06]];
    let sweep = |tolerance| {
        samples(
            tool.sweep([0.0; 3], [0.0; 3], Axis::Z, tolerance)
                .unwrap()
                .unwrap(),
            &point,
        )[0]
    };
    assert!(
        sweep(0.1) > 0.0,
        "the coarse chord is inside the round corner"
    );
    assert!(
        sweep(0.001) < 0.0,
        "the finer approximation includes this point"
    );
    assert_eq!(Tool::read(&tool.value()), Some(tool));
}

#[test]
fn offset_profiles_and_example_tools_remain_ordinary_data() {
    let stack = crate::stack::load();
    let tool = Tool::new(vec![Section::taper(
        Point::new(0.125, 0.5),
        Point::new(0.125, 0.9),
        SectionKind::Cutting,
    )])
    .unwrap();
    assert_eq!(tool.sections[0].start, Point::new(0.125, 0.5));
    assert_eq!(
        tool.sections[0].profile,
        [Segment::Line(Point::new(0.125, 0.9))]
    );
    assert_eq!(Tool::read(&tool.value()), Some(tool));
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    assert_eq!(names["evaluate"], ::grap::vocabulary::EVALUATE);
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    assert!(
        Tool::read(&::grap::evaluate(&Value::from(names["ball_tool"]), &sources, 1000).result)
            .is_some()
    );
    let square =
        Tool::read(&::grap::evaluate(&Value::from(names["square_tool"]), &sources, 1000).result)
            .unwrap();
    assert_eq!(square.sections.len(), 2);
    assert_eq!(square.sections[0].kind, SectionKind::Cutting);
    assert_eq!(square.sections[1].kind, SectionKind::NonCutting);
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
    let outline = tool.sections[0].outline(0.001).unwrap();
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
    let shank = tool.sections[1]
        .sweep(0.001, [0.0; 3], [0.0; 3], Axis::Z)
        .unwrap();
    assert!(samples(shank, &[[0.2, 0.0, 0.3]])[0] < 0.0);
}
