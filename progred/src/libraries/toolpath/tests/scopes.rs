use super::*;

fn line(x: f64) -> Value {
    sequence(vec![
        point_call(START_AT, [x, 0.0, 0.0]),
        point_call(LINE_TO, [x, 1.0, 0.0]),
    ])
}

#[test]
fn nested_tools_are_scoped_recorded_and_replayable() {
    let ball = cutter::Tool::ball(0.125, 0.3).unwrap();
    let square = cutter::Tool::square(0.25, 0.4).unwrap();
    let expression = sequence(vec![
        tool_scope(
            &ball,
            sequence(vec![line(0.0), tool_scope(&square, line(10.0)), line(20.0)]),
        ),
        line(30.0),
    ]);
    let (result, path) = evaluate(&expression, 1000);
    assert!(result.completed && !absent::is_absent(&result.result));
    let tools: Vec<_> = path.tool_segments().map(|(_, _, _, tool)| tool).collect();
    assert_eq!(tools, [Some(&ball), Some(&square), Some(&ball), None]);
    assert_eq!(path.length().unwrap(), 4.0);
    let Entry::WithTool(_, children) = &path.entries[0] else {
        panic!("tool hierarchy missing")
    };
    assert!(matches!(children[2], Entry::WithTool(..)));
    let mut replayed = Recording::default();
    path.replay(&mut replayed).unwrap();
    assert_eq!(replayed, path);
    for (progress, x, tool) in [
        (0.25, 0.0, Some(&ball)),
        (0.375, 10.0, Some(&square)),
        (0.625, 20.0, Some(&ball)),
        (1.0, 30.0, None),
    ] {
        let (pose, actual) = path
            .playback::<InvalidPath>(progress, |_, _, _, _, _| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(pose.tip[0], x);
        assert_eq!(actual, tool);
    }
}

#[test]
fn tool_boundaries_require_a_new_start() {
    let ball = cutter::Tool::ball(0.125, 0.3).unwrap();
    for expression in [
        sequence(vec![
            line(0.0),
            tool_scope(&ball, point_call(LINE_TO, [1.0; 3])),
        ]),
        sequence(vec![
            tool_scope(&ball, line(0.0)),
            point_call(LINE_TO, [1.0; 3]),
        ]),
    ] {
        let (result, path) = evaluate(&expression, 1000);
        assert!(result.completed && absent::is_absent(&result.result));
        assert_eq!(path.segments().count(), 1);
    }
}

#[test]
fn invalid_or_absent_tools_do_not_run_the_body() {
    for tool in [Value::record([]), absent::with_reason(INVALID_INPUT)] {
        let expression = call(
            WITH_TOOL,
            [
                (cutter::vocabulary::TOOL, tool),
                (::grap::vocabulary::EXPRESSION, line(0.0)),
            ],
        );
        let (result, path) = evaluate(&expression, 1000);
        assert!(result.completed && absent::is_absent(&result.result));
        assert!(path.entries.is_empty());
    }
}

#[test]
fn scopes_restore_after_recovered_absents_and_evaluator_halts() {
    let ball = cutter::Tool::ball(0.125, 0.3).unwrap();
    let square = cutter::Tool::square(0.25, 0.4).unwrap();
    let binder = gid::new_cell_id();
    let recover = |expression| {
        call(
            control::vocabulary::MATCH,
            [
                (control::vocabulary::VALUE, expression),
                (
                    control::vocabulary::CASES,
                    Value::list([Value::record([
                        (
                            control::vocabulary::PATTERN,
                            Value::record([(control::vocabulary::BIND, binder.into())]),
                        ),
                        (::grap::vocabulary::EXPRESSION, Value::record([])),
                    ])]),
                ),
            ],
        )
    };
    let (result, path) = evaluate(
        &tool_scope(
            &ball,
            sequence(vec![
                recover(tool_scope(&square, point_call(LINE_TO, [0.0; 3]))),
                line(1.0),
            ]),
        ),
        1000,
    );
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    assert_eq!(path.tool_segments().next().unwrap().3, Some(&ball));

    let (result, mut path) = evaluate(
        &tool_scope(&ball, sequence((0..100).map(|i| line(i as f64)).collect())),
        100,
    );
    assert!(!result.completed);
    path.start_at([100.0; 3], Axis::Z).unwrap();
    path.line_to([101.0; 3]).unwrap();
    assert_eq!(path.tool_segments().last().unwrap().3, None);
    let mut replayed = Recording::default();
    path.replay(&mut replayed).unwrap();
    assert_eq!(replayed, path);
}

#[test]
fn point_mapping_preserves_tool_scopes() {
    let tool = cutter::Tool::square(0.25, 0.4).unwrap();
    let (result, path) = evaluate(&tool_scope(&tool, line(0.0)), 1000);
    assert!(result.completed);
    let mut mapped = MapPoints {
        sink: Recording::default(),
        map: |p: Point3| Ok(p.map(|v| v + 3.0)),
    };
    path.replay(&mut mapped).unwrap();
    assert_eq!(
        mapped.sink.tool_segments().next(),
        Some(([3.0; 3], [3.0, 4.0, 3.0], Axis::Z, Some(&tool)))
    );
}

#[test]
fn axis_mapping_composes_inside_out_and_is_independent_of_translation() {
    let negate = |value| {
        call(
            f64::vocabulary::SUBTRACT,
            [
                (number::vocabulary::LEFT, f64::value(0.0)),
                (number::vocabulary::RIGHT, value),
            ],
        )
    };
    let turn_y = ::grap::lambda(
        [X, Y, Z],
        call(POINT, [(X, Z.into()), (Y, Y.into()), (Z, negate(X.into()))]),
    );
    let turn_z = ::grap::lambda(
        [X, Y, Z],
        call(POINT, [(X, negate(Y.into())), (Y, X.into()), (Z, Z.into())]),
    );
    let translated = ::grap::lambda(
        [X, Y, Z],
        call(
            POINT,
            [X, Y, Z].map(|key| {
                (
                    key,
                    call(
                        f64::vocabulary::SUM,
                        [
                            (number::vocabulary::LEFT, key.into()),
                            (number::vocabulary::RIGHT, f64::value(5.0)),
                        ],
                    ),
                )
            }),
        ),
    );
    let mapped = |function, mapper, expression| {
        call(
            function,
            [
                (MAPPER, mapper),
                (::grap::vocabulary::EXPRESSION, expression),
            ],
        )
    };
    let tool = cutter::Tool::square(0.125, 0.25).unwrap();
    let expression = sequence(vec![
        mapped(
            MAP_POINTS,
            translated,
            mapped(
                MAP_AXES,
                turn_z,
                mapped(MAP_AXES, turn_y, tool_scope(&tool, line(0.0))),
            ),
        ),
        line(10.0),
    ]);
    let (result, path) = evaluate(&expression, 1000);
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    let segments: Vec<_> = path.tool_segments().collect();
    assert_eq!(
        segments,
        [
            (
                [5.0; 3],
                [5.0, 6.0, 5.0],
                Axis::new([0.0, 1.0, 0.0]).unwrap(),
                Some(&tool)
            ),
            ([10.0, 0.0, 0.0], [10.0, 1.0, 0.0], Axis::Z, None),
        ]
    );
}

#[test]
fn editing_a_tool_invalidates_the_recording_even_when_points_are_unchanged() {
    use crate::{computations::Computations, sources::Sources};
    let tool_id = gid::new_cell_id();
    let small = cutter::Tool::square(0.2, 0.4).unwrap();
    let large = cutter::Tool::square(0.4, 0.4).unwrap();
    let mut doc = gid::Document {
        root: None,
        cells: Cells::new(),
    };
    doc.cells.set_value(tool_id, small.value());
    let libraries = crate::stack::load().libraries;
    let computations = Computations::from_sources(Sources {
        doc: &doc,
        libraries: &libraries,
    });
    let program = ::grap::lambda(
        [],
        call(
            WITH_TOOL,
            [
                (cutter::vocabulary::TOOL, tool_id.into()),
                (::grap::vocabulary::EXPRESSION, line(0.0)),
            ],
        ),
    );
    let record = computation::recording(
        &computations,
        computations.runtime.input(program),
        computations.runtime.input(1000),
    );
    let first = computations.runtime.read(&record).unwrap();
    assert_eq!(
        first.path().unwrap().tool_segments().next().unwrap().3,
        Some(&small)
    );
    assert!(Rc::ptr_eq(
        &first,
        &computations.runtime.read(&record).unwrap()
    ));
    doc.cells.set_value(tool_id, large.value());
    computations.begin(Rc::new(doc), libraries);
    let second = computations.runtime.read(&record).unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(
        second.path().unwrap().tool_segments().next().unwrap().3,
        Some(&large)
    );
    assert_eq!(first.path.commands(), second.path.commands());
    assert_ne!(first.path, second.path);
}
