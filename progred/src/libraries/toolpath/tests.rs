use super::{paths::*, *};
use crate::libraries::{control, number};

fn call(function: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::call(function.into(), fields)
}

fn sequence(expressions: Vec<Value>) -> Value {
    call(
        control::vocabulary::DO,
        [(control::vocabulary::EXPRESSIONS, Value::list(expressions))],
    )
}

fn point_call(function: CellId, point: Point3) -> Value {
    call(function, [X, Y, Z].into_iter().zip(point.map(f64::value)))
}

fn evaluate(expression: &Value, fuel: usize) -> (Evaluation, Recording) {
    let stack = crate::stack::load();
    let mut recording = Recording::default();
    let evaluation = run(&mut recording, |scope| {
        ::grap::evaluate_scoped(expression, &stack.libraries, scope, fuel)
    });
    (evaluation, recording)
}

fn diagonals(
    sink: &mut dyn Sink<Error = InvalidPath>,
    rows: f64,
    spacing: f64,
    fuel: usize,
) -> Evaluation {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    run(sink, |scope| {
        ::grap::apply_scoped(
            &names["diagonals"].into(),
            [
                (names["rows"], f64::value(rows)),
                (names["spacing"], f64::value(spacing)),
            ],
            &sources,
            scope,
            fuel,
        )
    })
}

#[test]
fn streaming_and_recording_are_interchangeable_consumers() {
    #[derive(Default)]
    struct Count(usize, usize);
    impl Sink for Count {
        type Error = InvalidPath;
        fn start_at(&mut self, _: Point3) -> Result<(), Self::Error> {
            self.0 += 1;
            Ok(())
        }
        fn line_to(&mut self, _: Point3) -> Result<(), Self::Error> {
            self.1 += 1;
            Ok(())
        }
    }
    let mut direct = Count::default();
    let result = diagonals(&mut direct, 21.0, 0.03, 100_000);
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    let mut recording = Recording::default();
    let result = diagonals(&mut recording, 21.0, 0.03, 100_000);
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    let mut replayed = Count::default();
    recording.replay(&mut replayed).unwrap();
    assert_eq!((direct.0, direct.1), (replayed.0, replayed.1));
    assert_eq!(direct.0, 21);
}

#[test]
fn diagonal_uv_endpoints_spacing_and_boundaries_match_rhino() {
    let mut recording = Recording::default();
    let result = diagonals(&mut recording, 3.0, 0.1, 100_000);
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    let starts = recording
        .commands
        .iter()
        .filter_map(|command| match *command {
            Command::StartAt(p) => Some(p),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        starts,
        vec![[0.0, 0.5, 0.0], [0.0, 0.0, 0.0], [0.5, 0.0, 0.0]]
    );
    let mut prior = None;
    let mut ends = Vec::new();
    for command in recording.commands {
        match command {
            Command::StartAt(point) => {
                if let Some(p) = prior {
                    ends.push(p);
                }
                prior = Some(point);
            }
            Command::LineTo(point) => {
                let p = prior.unwrap();
                assert!((point[0] - p[0]).hypot(point[1] - p[1]) <= 0.1 + 1e-12);
                assert!(point.into_iter().all(|v| (0.0..=1.0).contains(&v)));
                prior = Some(point);
            }
        }
    }
    ends.push(prior.unwrap());
    assert_eq!(
        ends,
        vec![[0.5, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.5, 0.0]]
    );
}

#[test]
fn point_adapters_compose_and_keep_path_breaks() {
    let mut recording = Recording::default();
    let mut translated = MapPoints {
        sink: &mut recording,
        map: |[x, y, z]: Point3| Ok([x + 10.0, y, z]),
    };
    let mut scaled = MapPoints {
        sink: &mut translated,
        map: |[x, y, z]: Point3| Ok([x * 2.0, y, z]),
    };
    scaled.start_at([1.0, 0.0, 0.0]).unwrap();
    scaled.line_to([2.0, 0.0, 0.0]).unwrap();
    scaled.start_at([3.0, 0.0, 0.0]).unwrap();
    scaled.line_to([4.0, 0.0, 0.0]).unwrap();
    assert_eq!(
        recording.commands,
        vec![
            Command::StartAt([12.0, 0.0, 0.0]),
            Command::LineTo([14.0, 0.0, 0.0]),
            Command::StartAt([16.0, 0.0, 0.0]),
            Command::LineTo([18.0, 0.0, 0.0])
        ]
    );
}

#[test]
fn grap_mapping_is_scoped_and_applies_in_composition_order() {
    let arithmetic = |function, rhs| {
        call(
            function,
            [
                (number::vocabulary::LEFT, X.into()),
                (number::vocabulary::RIGHT, f64::value(rhs)),
            ],
        )
    };
    let mapping = |expression| {
        ::grap::lambda(
            [X, Y, Z],
            call(POINT, [(X, expression), (Y, Y.into()), (Z, Z.into())]),
        )
    };
    let map = |mapping, expression| {
        call(
            MAP_POINTS,
            [
                (MAPPER, mapping),
                (::grap::vocabulary::EXPRESSION, expression),
            ],
        )
    };
    let start = point_call(START_AT, [1.0, 0.0, 0.0]);
    let expression = sequence(vec![
        map(
            mapping(arithmetic(f64::vocabulary::SUM, 10.0)),
            map(
                mapping(arithmetic(f64::vocabulary::MULTIPLY, 2.0)),
                start.clone(),
            ),
        ),
        start,
    ]);
    let (result, recording) = evaluate(&expression, 1000);
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    assert_eq!(
        recording.commands,
        vec![
            Command::StartAt([12.0, 0.0, 0.0]),
            Command::StartAt([1.0, 0.0, 0.0])
        ]
    );
}

#[test]
fn invalid_commands_stop_do_before_later_emissions() {
    for expression in [
        point_call(LINE_TO, [1.0, 0.0, 0.0]),
        point_call(START_AT, [f64::NAN, 0.0, 0.0]),
        call(
            MAP_POINTS,
            [
                (MAPPER, ::grap::lambda([X, Y, Z], Value::record([]))),
                (
                    ::grap::vocabulary::EXPRESSION,
                    point_call(START_AT, [0.0; 3]),
                ),
            ],
        ),
    ] {
        let (result, recording) = evaluate(
            &sequence(vec![expression, point_call(START_AT, [0.0; 3])]),
            1000,
        );
        assert!(result.completed);
        assert!(absent::is_absent(&result.result));
        assert!(recording.commands.is_empty());
    }
}

#[test]
fn a_handled_failure_does_not_poison_output_or_leak_a_mapping_scope() {
    let reason = gid::new_cell_id();
    let failure = ::grap::absent::with_detail(reason, X, f64::value(42.0));
    let failed_map = call(
        MAP_POINTS,
        [
            (MAPPER, ::grap::lambda([X, Y, Z], failure.clone())),
            (
                ::grap::vocabulary::EXPRESSION,
                point_call(START_AT, [1.0; 3]),
            ),
        ],
    );
    let caught = call(
        control::vocabulary::MATCH,
        [
            (control::vocabulary::VALUE, failed_map.clone()),
            (
                control::vocabulary::CASES,
                Value::list([Value::record([
                    (
                        control::vocabulary::PATTERN,
                        Value::record([(absent::vocabulary::ABSENT, reason.into())]),
                    ),
                    (
                        ::grap::vocabulary::EXPRESSION,
                        point_call(START_AT, [2.0; 3]),
                    ),
                ])]),
            ),
        ],
    );
    let (result, recording) = evaluate(
        &sequence(vec![
            point_call(START_AT, [0.0; 3]),
            caught,
            point_call(LINE_TO, [3.0; 3]),
        ]),
        1000,
    );
    assert!(result.completed);
    assert_eq!(result.result, Value::record([]));
    assert_eq!(
        recording.commands,
        [
            Command::StartAt([0.0; 3]),
            Command::StartAt([2.0; 3]),
            Command::LineTo([3.0; 3])
        ]
    );

    let (result, recording) = evaluate(
        &sequence(vec![point_call(START_AT, [0.0; 3]), failed_map]),
        1000,
    );
    assert!(result.completed);
    assert_eq!(result.result, failure);
    assert_eq!(recording.commands, [Command::StartAt([0.0; 3])]);
}

#[test]
fn generation_is_fueled_and_requires_a_scoped_sink() {
    let mut recording = Recording::default();
    let result = diagonals(&mut recording, 1_000_000.0, 0.0001, 100);
    assert!(!result.completed);
    assert!(recording.commands.len() <= 100);
    let stack = crate::stack::load();
    let unscoped = ::grap::evaluate(&point_call(START_AT, [0.0; 3]), &stack.libraries, 100);
    assert_eq!(unscoped.result, absent::with_reason(OUTPUT_REQUIRED));
}

#[test]
fn example_rejects_invalid_sampling_and_accepts_zero_rows() {
    for (rows, spacing) in [
        (-1.0, 0.1),
        (2.5, 0.1),
        (f64::NAN, 0.1),
        (f64::INFINITY, 0.1),
        (2.0, 0.0),
        (2.0, -0.1),
        (2.0, f64::NAN),
        (2.0, f64::INFINITY),
    ] {
        let mut recording = Recording::default();
        let result = diagonals(&mut recording, rows, spacing, 1000);
        assert!(
            result.completed && absent::is_absent(&result.result),
            "{:?}",
            result.result
        );
        assert!(recording.commands.is_empty());
    }
    let mut recording = Recording::default();
    let result = diagonals(&mut recording, 0.0, 0.1, 1000);
    assert!(result.completed && !absent::is_absent(&result.result));
    assert!(recording.commands.is_empty());

    let result = diagonals(&mut recording, 1.0, f64::from_bits(1), 1000);
    assert!(!result.completed);
}

#[test]
fn example_is_two_crossing_sweeps_on_the_rhino_top_face() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut recording = Recording::default();
    let evaluation = run(&mut recording, |scope| {
        ::grap::apply_scoped(&names["crosshatch"].into(), [], &sources, scope, 100_000)
    });
    assert!(
        evaluation.completed && !absent::is_absent(&evaluation.result),
        "{:?}",
        evaluation.result
    );
    let mut starts = 0;
    for command in recording.commands {
        let [x, y, z] = match command {
            Command::StartAt(p) => {
                starts += 1;
                p
            }
            Command::LineTo(p) => p,
        };
        let u = (x / 0.4 + 1.0) / 2.0;
        let v = (y / 0.4 + 1.0) / 2.0;
        // The displaced center control point has Bernstein weight
        // 2u(1-u) * 2v(1-v), not the full control-point displacement.
        let expected = 0.5 - 0.5 * (2.0 * u * (1.0 - u)) * (2.0 * v * (1.0 - v));
        assert!((expected - z).abs() < 1e-12);
    }
    assert_eq!(starts, 42);
}
