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
