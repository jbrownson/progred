use super::*;
use ::grap::{RuntimeValue, SourceOrigin};
use std::{cell::RefCell, rc::Rc};

fn runtime_call(
    function: CellId,
    fields: impl IntoIterator<Item = (CellId, RuntimeValue)>,
) -> RuntimeValue {
    RuntimeValue::record(
        [
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::CALL).into(),
            ),
            (grap::vocabulary::FUNCTION, Value::from(function).into()),
        ]
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
    let tracer = Rc::new(::grap::Tracer::default());
    let unobserved = crate::test_support::TestHost(|cell| {
        if cell == inspect {
            let (origins, tracer) = (origins.clone(), tracer.clone());
            Some(crate::test_support::TestDefinition::foreign(
                inspect,
                ForeignFunction::new(move |context, call, environment| {
                    origins
                        .borrow_mut()
                        .push(tracer.trace().unwrap().origins().next().unwrap().clone());
                    context.eval(call.field(captured).unwrap(), environment)
                }),
            ))
        } else {
            crate::test_support::defined_in(&libraries, cell)
        }
    });
    let make = |body, marker, value| {
        let maker = ::grap::Evaluator::new(&unobserved, 1000)
            .observer(&*tracer)
            .evaluate_runtime(
                &::grap::RuntimeValue::from(::grap::lambda([captured], ::grap::lambda([], body))),
                Some(SourceOrigin::Root(vec![gid::Step::Key(marker)])),
            )
            .result;
        ::grap::Evaluator::new(&unobserved, 1000)
            .observer(&*tracer)
            .apply(&maker, [(captured, value)])
            .result
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
    let unobserved_receiver = crate::test_support::TestHost(|cell| {
        if cell == place {
            let (origins, tracer) = (origins.clone(), tracer.clone());
            Some(crate::test_support::TestDefinition::foreign(
                place,
                ForeignFunction::new(move |_, _, _| {
                    origins
                        .borrow_mut()
                        .push(tracer.trace().unwrap().origins().next().unwrap().clone());
                    Ok(RuntimeValue::record([]))
                }),
            ))
        } else {
            (unobserved.0)(cell)
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
        ::grap::Evaluator::new(&unobserved_receiver, 1000)
            .functions(scope)
            .observer(&*tracer)
            .evaluate_runtime(&expression, None)
    });
    assert!(
        result.result.absent_reason().is_none(),
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
        let SourceOrigin::Root(path) = origin else {
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
    let callback = ::grap::Evaluator::new(&host, 100)
        .evaluate_runtime(
            &::grap::RuntimeValue::from(::grap::lambda([], f64::value(7.0))),
            Some(SourceOrigin::Root(vec![gid::Step::Key(gid::new_cell_id())])),
        )
        .result;
    let reason = gid::new_cell_id();
    let detail = gid::new_cell_id();
    let failure = ::grap::absent::runtime(reason, [(detail, callback.clone())]);
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
                ::grap::Evaluator::new(&host, 1000)
                    .functions(scope)
                    .evaluate_runtime(&expression, None)
            });
            assert!(result.halt.is_none() && result.result.same_result(&payload));
        }
    }
    // Absents returned by a mapper or nested sequence keep their details too.
    let parameter = gid::new_cell_id();
    let maker = ::grap::Evaluator::new(&host, 100)
        .evaluate(&::grap::lambda(
            [parameter],
            ::grap::lambda([], parameter.into()),
        ))
        .result;
    let failing = ::grap::Evaluator::new(&host, 100)
        .apply(&maker, [(parameter, failure.clone())])
        .result;
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
            ::grap::Evaluator::new(&host, 1000)
                .functions(scope)
                .evaluate_runtime(&expression, None)
        });
        assert!(result.halt.is_none() && result.result.same_result(&failure));
        assert!(path.commands().is_empty());
    }
}

#[test]
fn preview_constructor_preserves_its_runtime_program() {
    let host = crate::stack::load().libraries;
    let program = ::grap::Evaluator::new(&host, 100)
        .evaluate_runtime(
            &::grap::RuntimeValue::from(::grap::lambda([], point_call(START_AT, [0.0; 3]))),
            Some(SourceOrigin::Root(vec![gid::Step::Key(gid::new_cell_id())])),
        )
        .result;
    let result = ::grap::Evaluator::new(&host, 100).apply(
        &Value::from(PREVIEW).into(),
        [
            (presentation::vocabulary::VALUE, program.clone()),
            (layout::vocabulary::WIDTH, RuntimeValue::f64(200.0)),
            (layout::vocabulary::HEIGHT, RuntimeValue::f64(150.0)),
        ],
    );
    assert_eq!(result.halt, None);
    assert_eq!(result.result.kind(), Some(kind::PATHS_PREVIEW));
    assert!(result.result.field(PROGRAM).unwrap().same_result(&program));
    // The three 3D constructors must keep the same executable leaves as the
    // 2D one; only the model/appearance data goes through domain decoders.
    for (function, kind) in [
        (PREVIEW_3D, kind::PATHS_VOLUME_PREVIEW),
        (PREVIEW_MESH, kind::PATHS_MESH_PREVIEW),
        (PREVIEW_REFINED, kind::PATHS_REFINED_PREVIEW),
    ] {
        let programs = RuntimeValue::list([RuntimeValue::list([program.clone()])]);
        let result = ::grap::Evaluator::new(&host, 1000).apply(
            &Value::from(function).into(),
            [
                (PROGRAM, programs.clone()),
                (
                    presentation::vocabulary::VALUE,
                    crate::libraries::f32::value(1.0).into(),
                ),
                (layout::vocabulary::WIDTH, RuntimeValue::f64(200.0)),
                (layout::vocabulary::HEIGHT, RuntimeValue::f64(150.0)),
                (LINE_RADIUS, RuntimeValue::f64(0.01)),
                (
                    crate::libraries::fidget::vocabulary::COLOR,
                    crate::libraries::color::tests::value(peniko::Color::WHITE).into(),
                ),
            ],
        );
        assert!(
            result.result.absent_reason().is_none(),
            "{:?}",
            result.result
        );
        assert_eq!(result.result.kind(), Some(kind));
        assert!(result.result.field(PROGRAM).unwrap().same_result(&programs));
    }
}

#[test]
#[ignore = "compares retained runtime mappers with their materialized GID equivalents"]
fn runtime_mapping_profile() {
    use std::time::{Duration, Instant};
    let host = crate::stack::load().libraries;
    let offset = gid::new_cell_id();
    let maker = ::grap::Evaluator::new(&host, 1000)
        .evaluate(&::grap::lambda(
            [offset],
            ::grap::lambda(
                [X, Y, Z],
                call(
                    POINT,
                    [X, Y, Z].map(|axis| {
                        (
                            axis,
                            call(
                                crate::libraries::grap::numbers::f64::SUM,
                                [
                                    (
                                        crate::libraries::grap::numbers::vocabulary::LEFT,
                                        axis.into(),
                                    ),
                                    (
                                        crate::libraries::grap::numbers::vocabulary::RIGHT,
                                        offset.into(),
                                    ),
                                ],
                            ),
                        )
                    }),
                ),
            ),
        ))
        .result;
    let mapper = ::grap::Evaluator::new(&host, 1000)
        .apply(&maker, [(offset, RuntimeValue::f64(0.5))])
        .result;
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
            ::grap::Evaluator::new(&host, 100_000)
                .functions(scope)
                .evaluate_runtime(program, None)
        });
        assert!(result.result.absent_reason().is_none());
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
