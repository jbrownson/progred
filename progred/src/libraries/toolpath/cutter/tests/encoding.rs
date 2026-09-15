use super::*;
use vocabulary::*;

fn movement(fields: impl IntoIterator<Item = (gid::CellId, f64)>) -> Value {
    Value::record(
        fields
            .into_iter()
            .map(|(id, number)| (id, f64::value(number))),
    )
}

fn read(moves: impl IntoIterator<Item = Value>) -> Option<Tool> {
    Tool::read(&Value::record([(
        TOOL,
        Value::list([Value::record([(CUTTING, Value::list(moves))])]),
    )]))
}

#[test]
fn compact_square_and_bull_profiles_match_constructors() {
    assert_eq!(
        read([movement([(RADIUS, 0.0625)]), movement([(AXIAL, 0.22)])]),
        Tool::square(0.125, 0.22)
    );
    let tool = read([
        movement([(RADIUS, 0.0625 - 0.01)]),
        movement([(CONVEX_ARC, 0.01), (RADIUS, 0.0625), (AXIAL, 0.01)]),
        movement([(AXIAL, 0.22)]),
    ])
    .unwrap();
    assert_eq!(Some(tool.clone()), Tool::bull(0.125, 0.01, 0.22));
    assert_eq!(Tool::read(&tool.value()), Some(tool));
}

#[test]
fn the_same_endpoints_and_radius_have_two_distinct_bends() {
    let outline = |tag| {
        read([
            movement([(RADIUS, 0.2)]),
            movement([(tag, 0.1), (RADIUS, 0.3), (AXIAL, 0.1)]),
        ])
        .unwrap()
        .sections[0]
            .outline(0.0001)
            .unwrap()
    };
    for (tag, center) in [
        (CONVEX_ARC, Point::new(0.2, 0.1)),
        (CONCAVE_ARC, Point::new(0.3, 0.0)),
    ] {
        let points = outline(tag);
        assert_eq!(points.first(), Some(&Point::new(0.2, 0.0)));
        assert_eq!(points.last(), Some(&Point::new(0.3, 0.1)));
        for p in points {
            assert!(((p.radius - center.radius).hypot(p.axial - center.axial) - 0.1).abs() < 1e-14);
        }
    }
    let convex = outline(CONVEX_ARC);
    let concave = outline(CONCAVE_ARC);
    assert!(convex[convex.len() / 2].radius > concave[concave.len() / 2].radius);
}

#[test]
fn arcs_can_inherit_radius_and_reject_impossible_or_backtracking_geometry() {
    assert!(
        read([
            movement([(RADIUS, 0.25)]),
            movement([(CONVEX_ARC, 0.1), (AXIAL, 0.2)]),
        ])
        .is_some(),
        "a convex semicircle can keep its endpoint radius"
    );
    for arc in [
        vec![(CONVEX_ARC, 0.01), (RADIUS, 0.3), (AXIAL, 0.1)],
        vec![(CONVEX_ARC, -0.1), (RADIUS, 0.3), (AXIAL, 0.1)],
        vec![(CONVEX_ARC, 0.0), (RADIUS, 0.3), (AXIAL, 0.1)],
        vec![(CONVEX_ARC, f64::NAN), (AXIAL, 0.1)],
        vec![(CONVEX_ARC, f64::INFINITY), (AXIAL, 0.1)],
        vec![(CONVEX_ARC, 0.1)],                // no endpoint coordinate
        vec![(CONVEX_ARC, 0.1), (RADIUS, 0.2)], // zero chord
        vec![(CONVEX_ARC, 0.1), (RADIUS, 0.3)], // returns to its axial height
        vec![
            (CONVEX_ARC, 0.5_f64.sqrt() / 10.0),
            (RADIUS, 0.3),
            (AXIAL, 0.1),
        ], // bends backward
        vec![(CONVEX_ARC, 0.1), (CONCAVE_ARC, 0.1), (AXIAL, 0.1)],
    ] {
        assert!(
            read([movement([(RADIUS, 0.2)]), movement(arc.clone())]).is_none(),
            "{arc:?}"
        );
    }
    assert!(
        read([
            movement([(RADIUS, 0.05)]),
            movement([(CONCAVE_ARC, 0.1), (AXIAL, 0.2)]),
        ])
        .is_none(),
        "a concave semicircle may not cross the axis"
    );
}

#[test]
fn axis_travel_is_empty_and_each_profile_starts_at_the_origin() {
    let tool = read([
        movement([(AXIAL, 0.2)]),
        movement([(RADIUS, 0.1)]),
        movement([(AXIAL, 0.4)]),
        movement([(RADIUS, 0.0)]),
        movement([(AXIAL, 0.6)]),
        movement([(RADIUS, 0.2)]),
        movement([(AXIAL, 0.8)]),
    ])
    .unwrap();
    assert_eq!(tool.sections.len(), 2);
    assert_eq!(tool.sections[0].start, Point::new(0.1, 0.2));
    assert_eq!(tool.sections[1].start, Point::new(0.2, 0.6));
    let values = samples(
        tool.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001)
            .unwrap()
            .unwrap(),
        &[
            [0.0, 0.0, 0.1],
            [0.0, 0.0, 0.3],
            [0.0, 0.0, 0.5],
            [0.0, 0.0, 0.7],
        ],
    );
    assert!(values[0] > 0.0 && values[1] < 0.0 && values[2] > 0.0 && values[3] < 0.0);
    assert_eq!(Tool::read(&tool.value()), Some(tool));
}

#[test]
fn invalid_coordinates_are_not_treated_as_missing() {
    for moves in [
        vec![],
        vec![movement([])],
        vec![movement([(AXIAL, 0.2)])],  // no material
        vec![movement([(RADIUS, 0.2)])], // no axial extent
        vec![movement([(RADIUS, -0.2)]), movement([(AXIAL, 0.2)])],
        vec![movement([(RADIUS, 0.2)]), movement([(AXIAL, -0.2)])],
        vec![
            movement([(RADIUS, 0.2)]),
            movement([(AXIAL, 0.4)]),
            movement([(AXIAL, 0.2)]),
        ],
        vec![
            movement([(RADIUS, 0.2)]),
            Value::record([(AXIAL, Value::record([]))]),
        ],
        vec![
            movement([(RADIUS, f64::INFINITY)]),
            movement([(AXIAL, 0.2)]),
        ],
        vec![
            movement([(RADIUS, 1e300)]),
            movement([(RADIUS, 0.2)]),
            movement([(AXIAL, 0.2)]),
        ],
    ] {
        assert!(read(moves.clone()).is_none(), "{moves:?}");
    }
}

#[test]
fn redundant_caps_and_radial_steps_lower_without_extra_surfaces() {
    let tool = read([
        movement([(RADIUS, 0.1)]),
        movement([(RADIUS, 0.2)]),
        movement([(AXIAL, 0.4)]),
        movement([(RADIUS, 0.3)]),
        movement([(RADIUS, 0.4)]),
        movement([(AXIAL, 0.6)]),
        movement([(RADIUS, 0.0)]),
    ])
    .unwrap();
    assert_eq!(
        tool.sections[0].profile,
        [
            Segment::Line(Point::new(0.2, 0.4)),
            Segment::Shoulder(0.4),
            Segment::Line(Point::new(0.4, 0.6)),
        ]
    );
    assert!(
        samples(
            tool.sweep([0.0; 3], [0.0; 3], Axis::Z, 0.001)
                .unwrap()
                .unwrap(),
            &[[0.1, 0.0, 0.4]]
        )[0] < 0.0
    );
}

#[test]
fn decoded_ball_mills_keep_the_exact_sweep_independent_of_chord_tolerance() {
    for diameter in [0.125, 0.1, 3.0] {
        let tool = Tool::read(&Tool::ball(diameter, diameter * 3.0).unwrap().value()).unwrap();
        let points = [[0.01, 0.0, 0.01], [0.03, 0.01, 0.05]];
        let sweep = |tolerance| {
            samples(
                tool.sweep([0.0; 3], [0.02, 0.0, 0.01], Axis::Z, tolerance)
                    .unwrap()
                    .unwrap(),
                &points,
            )
        };
        assert_eq!(
            sweep(0.01),
            sweep(1e-100),
            "exact ball sweep does not sample its arc"
        );
    }
}
