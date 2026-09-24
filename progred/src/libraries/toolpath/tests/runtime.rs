use super::*;
use ::grap::{Host, RuntimeValue, SourceOrigin};
use std::{cell::RefCell, rc::Rc};

fn runtime_call(
    function: CellId,
    fields: impl IntoIterator<Item = (CellId, RuntimeValue)>,
) -> RuntimeValue {
    RuntimeValue::record(
        [(grap::vocabulary::FUNCTION, Value::from(function).into())]
            .into_iter()
            .chain(fields),
    )
}

#[test]
fn nested_programs_and_mappers_retain_captures_and_original_call_sites() {
    let libraries = crate::stack::load().libraries;
    let inspect = gid::new_cell_id();
    let captured = gid::new_cell_id();
    let place = gid::new_cell_id();
    let origins = Rc::new(RefCell::new(Vec::new()));
    let host = crate::libraries::TestHost(|cell| {
        if cell == inspect {
            let origins = origins.clone();
            vec![(
                gid::Resolution::Document,
                ::grap::Definition::foreign(
                    Value::record([]),
                    ForeignFunction::new(move |context, call, environment| {
                        origins.borrow_mut().push(
                            context
                                .call_trace()
                                .unwrap()
                                .origins()
                                .next()
                                .unwrap()
                                .clone(),
                        );
                        context.eval(context.field(call, captured).unwrap(), environment)
                    }),
                ),
            )]
        } else {
            libraries.resolve(cell).into_iter().collect()
        }
    });
    let make = |body, marker, value| {
        let maker = ::grap::evaluate_at(
            &::grap::lambda([captured], ::grap::lambda([], body)),
            Some(SourceOrigin::Stored(vec![gid::Step::Key(marker)])),
            &host,
            1000,
        )
        .result;
        ::grap::apply(&maker, [(captured, value)], &host, 1000).result
    };
    let mapper_origin = gid::new_cell_id();
    let mapper = make(
        call(inspect, [(captured, captured.into())]),
        mapper_origin,
        runtime_point_value([0.0, 1.0, 0.0]),
    );
    let program_origin = gid::new_cell_id();
    let program = make(
        sequence(vec![
            call(place, []),
            point_call(START_AT, [0.0; 3]),
            point_call(LINE_TO, [1.0; 3]),
        ]),
        program_origin,
        RuntimeValue::record([]),
    );
    // Reuse the closure in a receiving host; its saved code must call the new capability.
    let receiver = crate::libraries::TestHost(|cell| {
        if cell == place {
            let origins = origins.clone();
            vec![(
                gid::Resolution::Document,
                ::grap::Definition::foreign(
                    Value::record([]),
                    ForeignFunction::new(move |context, _, _| {
                        origins.borrow_mut().push(
                            context
                                .call_trace()
                                .unwrap()
                                .origins()
                                .next()
                                .unwrap()
                                .clone(),
                        );
                        Ok(RuntimeValue::record([]))
                    }),
                ),
            )]
        } else {
            host.resolve(cell).into_iter().collect()
        }
    });
    let sequence = runtime_call(
        SEQUENCE,
        [(
            PROGRAM,
            RuntimeValue::list([program.clone(), RuntimeValue::list([program])]),
        )],
    );
    let expression = runtime_call(
        MAP_POINTS,
        [
            (MAPPER, mapper.clone()),
            (
                ::grap::vocabulary::EXPRESSION,
                runtime_call(
                    MAP_AXES,
                    [(MAPPER, mapper), (::grap::vocabulary::EXPRESSION, sequence)],
                ),
            ),
        ],
    );
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::evaluate_runtime_scoped(&expression, &receiver, scope, 1000)
    });
    assert!(
        result.completed && !result.result.is_absent(),
        "{:?}",
        result.result
    );
    assert_eq!(path.commands().len(), 4);
    assert_eq!(
        path.commands()
            .iter()
            .filter(|c| matches!(c, Command::StartAt(_, _)))
            .count(),
        2
    );
    for command in path.commands() {
        match command {
            Command::StartAt(point, axis) => {
                assert_eq!(point, [0.0, 1.0, 0.0]);
                assert_eq!(axis, Axis::new(point).unwrap());
            }
            Command::LineTo(point) => assert_eq!(point, [0.0, 1.0, 0.0]),
        }
    }
    let origins = origins.borrow();
    assert_eq!(
        origins.len(),
        8,
        "two program entries and six point/axis mapping calls"
    );
    for origin in origins.iter() {
        let SourceOrigin::Stored(path) = origin else {
            panic!("lost stored call origin")
        };
        assert!(
            path.starts_with(&[gid::Step::Key(mapper_origin)])
                || path.starts_with(&[gid::Step::Key(program_origin)])
        );
    }
}

#[test]
fn scopes_and_failures_keep_runtime_payloads() {
    let host = crate::stack::load().libraries;
    let callback = ::grap::evaluate_at(
        &::grap::lambda([], f64::value(7.0)),
        Some(SourceOrigin::Stored(vec![gid::Step::Key(
            gid::new_cell_id(),
        )])),
        &host,
        100,
    )
    .result;
    let reason = gid::new_cell_id();
    let detail = gid::new_cell_id();
    let failure = RuntimeValue::record([
        (::grap::absent::ABSENT, Value::from(reason).into()),
        (detail, callback.clone()),
    ]);
    let tool = cutter::Tool::ball(0.125, 0.5).unwrap();
    for payload in [callback.clone(), failure.clone()] {
        for function in [MAP_POINTS, MAP_AXES, WITH_TOOL] {
            let expression = runtime_call(
                function,
                [
                    (MAPPER, callback.clone()),
                    (cutter::vocabulary::TOOL, tool.value().into()),
                    (::grap::vocabulary::EXPRESSION, payload.clone()),
                ],
            );
            let mut path = Recording::default();
            let result = run(&mut path, |scope| {
                ::grap::evaluate_runtime_scoped(&expression, &host, scope, 1000)
            });
            assert!(result.completed && result.result.same_result(&payload));
        }
    }
    // Absents returned by a mapper or nested sequence keep their details too.
    let parameter = gid::new_cell_id();
    let maker = ::grap::evaluate(
        &::grap::lambda([parameter], ::grap::lambda([], parameter.into())),
        &host,
        100,
    )
    .result;
    let failing = ::grap::apply(&maker, [(parameter, failure.clone())], &host, 100).result;
    for expression in [
        runtime_call(
            MAP_POINTS,
            [
                (MAPPER, failing.clone()),
                (
                    ::grap::vocabulary::EXPRESSION,
                    point_call(START_AT, [0.0; 3]).into(),
                ),
            ],
        ),
        runtime_call(
            SEQUENCE,
            [(PROGRAM, RuntimeValue::list([RuntimeValue::list([failing])]))],
        ),
    ] {
        let mut path = Recording::default();
        let result = run(&mut path, |scope| {
            ::grap::evaluate_runtime_scoped(&expression, &host, scope, 1000)
        });
        assert!(result.completed && result.result.same_result(&failure));
        assert!(path.commands().is_empty());
    }
}

#[test]
fn preview_constructor_preserves_its_runtime_program() {
    let host = crate::stack::load().libraries;
    let program = ::grap::evaluate_at(
        &::grap::lambda([], point_call(START_AT, [0.0; 3])),
        Some(SourceOrigin::Stored(vec![gid::Step::Key(
            gid::new_cell_id(),
        )])),
        &host,
        100,
    )
    .result;
    let result = ::grap::apply(
        &Value::from(PREVIEW).into(),
        [
            (presentation::vocabulary::VALUE, program.clone()),
            (layout::vocabulary::WIDTH, RuntimeValue::f64(200.0)),
            (layout::vocabulary::HEIGHT, RuntimeValue::f64(150.0)),
        ],
        &host,
        100,
    );
    assert!(result.completed);
    assert!(
        result
            .result
            .field(PREVIEW)
            .unwrap()
            .field(PROGRAM)
            .unwrap()
            .same_result(&program)
    );
}

#[test]
#[ignore = "compares retained runtime mappers with their materialized GID equivalents"]
fn runtime_mapping_profile() {
    use std::time::{Duration, Instant};
    let host = crate::stack::load().libraries;
    let offset = gid::new_cell_id();
    let maker = ::grap::evaluate(
        &::grap::lambda(
            [offset],
            ::grap::lambda(
                [X, Y, Z],
                call(
                    POINT,
                    [X, Y, Z].map(|axis| {
                        (
                            axis,
                            call(
                                f64::vocabulary::SUM,
                                [
                                    (number::vocabulary::LEFT, axis.into()),
                                    (number::vocabulary::RIGHT, offset.into()),
                                ],
                            ),
                        )
                    }),
                ),
            ),
        ),
        &host,
        1000,
    )
    .result;
    let mapper = ::grap::apply(&maker, [(offset, RuntimeValue::f64(0.5))], &host, 1000).result;
    let body: RuntimeValue = sequence(
        (0..=256)
            .map(|i| {
                point_call(
                    if i == 0 { START_AT } else { LINE_TO },
                    [i as f64, 0.0, 0.0],
                )
            })
            .collect(),
    )
    .into();
    let programs = [mapper.clone(), mapper.to_value().into()].map(|mapper| {
        runtime_call(
            MAP_POINTS,
            [
                (MAPPER, mapper.clone()),
                (
                    ::grap::vocabulary::EXPRESSION,
                    runtime_call(
                        MAP_POINTS,
                        [
                            (MAPPER, mapper),
                            (::grap::vocabulary::EXPRESSION, body.clone()),
                        ],
                    ),
                ),
            ],
        )
    });
    let record = |program: &RuntimeValue| {
        let mut path = Recording::default();
        let result = run(&mut path, |scope| {
            ::grap::evaluate_runtime_scoped(program, &host, scope, 100_000)
        });
        assert!(result.completed && !result.result.is_absent());
        path
    };
    let expected = record(&programs[0]);
    assert_eq!(expected, record(&programs[1]));
    assert_eq!(expected.segments().count(), 256);
    let mut samples: [Vec<Duration>; 2] = [vec![], vec![]];
    for iteration in 0..35 {
        for index in [iteration % 2, 1 - iteration % 2] {
            let start = Instant::now();
            std::hint::black_box(record(&programs[index]));
            let elapsed = start.elapsed();
            if iteration >= 5 {
                samples[index].push(elapsed);
            }
        }
    }
    for (label, mut samples) in ["runtime mappers", "materialized GID mappers"]
        .into_iter()
        .zip(samples)
    {
        samples.sort();
        eprintln!(
            "{label}, 256 segments, two nested maps: median {:?}, p95 {:?}",
            samples[15], samples[28]
        );
    }
}
