use super::{paths::*, *};
use crate::libraries::{control, number};

mod geometry;
mod orientation;
mod scopes;

impl Recording {
    fn commands(&self) -> Vec<Command> {
        self.moves().map(|(command, _)| command).collect()
    }
}

fn cube_geometry(sources: &crate::sources::Sources<'_>, names: &crate::gid_text::Binders) -> Value {
    let result = ::grap::evaluate(&call(names["cube"], []), sources, 10_000);
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    result.result
}

fn top_face(sources: &crate::sources::Sources<'_>, names: &crate::gid_text::Binders) -> Value {
    cube_geometry(sources, names)
        .as_record()
        .unwrap()
        .get(&names["face"])
        .unwrap()
        .clone()
}

#[test]
fn example_view_uses_refinement_with_the_playback_and_tool_diameter() {
    use crate::libraries::{controls::vocabulary as ui, layout::vocabulary as l, presentation};
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let (value, viewport) =
        presentation::viewport(sources.resolve_path(&pane.path).unwrap()).unwrap();
    let controls = ::grap::apply(
        viewport,
        [
            (presentation::vocabulary::VALUE, value.clone()),
            (l::WIDTH, f64::value(400.0)),
            (l::HEIGHT, f64::value(500.0)),
        ],
        &sources,
        10_000,
    );
    assert!(controls.completed);
    let fields = controls
        .result
        .as_record()
        .unwrap()
        .get(&ui::WITH_CONTROLS)
        .unwrap()
        .as_record()
        .unwrap();
    let preview = ::grap::apply(
        fields.get(&ui::VIEW).unwrap(),
        [
            (
                presentation::vocabulary::VALUE,
                fields
                    .get(&presentation::vocabulary::VALUE)
                    .unwrap()
                    .clone(),
            ),
            (l::WIDTH, f64::value(400.0)),
            (l::HEIGHT, f64::value(400.0)),
            (ui::PARAMETERS, Value::record([(PROGRESS, f64::value(0.7))])),
        ],
        &sources,
        10_000,
    );
    assert!(preview.completed, "{:?}", preview.result);
    let fields = preview
        .result
        .as_record()
        .unwrap()
        .get(&PREVIEW_REFINED)
        .expect("refined preview callable")
        .as_record()
        .unwrap();
    let playback = fields.get(&PLAYBACK).unwrap();
    assert!(super::playback::Settings::read(playback).is_some());
    let playback = playback.as_record().unwrap();
    assert!(!playback.contains_key(&super::cutter::vocabulary::TOOL));
    assert_eq!(f64::read(playback.get(&PROGRESS).unwrap()), Some(0.7));
}

fn call(function: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::call(function.into(), fields)
}

fn sequence(expressions: Vec<Value>) -> Value {
    call(
        control::vocabulary::DO,
        [(control::vocabulary::EXPRESSIONS, Value::list(expressions))],
    )
}

fn tool_scope(tool: &cutter::Tool, expression: Value) -> Value {
    call(
        WITH_TOOL,
        [
            (cutter::vocabulary::TOOL, tool.value()),
            (::grap::vocabulary::EXPRESSION, expression),
        ],
    )
}

pub(super) fn tool_program(program: Value) -> Value {
    ::grap::lambda(
        [],
        tool_scope(
            &cutter::Tool::ball(0.2, 0.4).unwrap(),
            ::grap::call(program, []),
        ),
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
                (names["feed_direction"], f64::value(1.0)),
                (names["row_direction"], f64::value(1.0)),
                (TOOL_AXIS, point_value(Axis::Z.vector())),
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
        fn end_path(&mut self) {}
        fn start_at(&mut self, _: Point3, _: Axis) -> Result<(), Self::Error> {
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
        .commands()
        .iter()
        .filter_map(|command| match *command {
            Command::StartAt(p, _) => Some(p),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        starts,
        vec![[0.0, 0.5, 0.0], [0.0, 0.0, 0.0], [0.5, 0.0, 0.0]]
    );
    let mut prior = None;
    let mut ends = Vec::new();
    for command in recording.commands() {
        match command {
            Command::StartAt(point, _) => {
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
    scaled.start_at([1.0, 0.0, 0.0], Axis::Z).unwrap();
    scaled.line_to([2.0, 0.0, 0.0]).unwrap();
    scaled.start_at([3.0, 0.0, 0.0], Axis::Z).unwrap();
    scaled.line_to([4.0, 0.0, 0.0]).unwrap();
    assert_eq!(
        recording.commands(),
        vec![
            Command::StartAt([12.0, 0.0, 0.0], Axis::Z),
            Command::LineTo([14.0, 0.0, 0.0]),
            Command::StartAt([16.0, 0.0, 0.0], Axis::Z),
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
        recording.commands(),
        vec![
            Command::StartAt([12.0, 0.0, 0.0], Axis::Z),
            Command::StartAt([1.0, 0.0, 0.0], Axis::Z)
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
        assert!(recording.commands().is_empty());
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
        recording.commands(),
        [
            Command::StartAt([0.0; 3], Axis::Z),
            Command::StartAt([2.0; 3], Axis::Z),
            Command::LineTo([3.0; 3])
        ]
    );

    let (result, recording) = evaluate(
        &sequence(vec![point_call(START_AT, [0.0; 3]), failed_map]),
        1000,
    );
    assert!(result.completed);
    assert_eq!(result.result, failure);
    assert_eq!(recording.commands(), [Command::StartAt([0.0; 3], Axis::Z)]);
}

#[test]
fn generation_is_fueled_and_requires_a_scoped_sink() {
    let mut recording = Recording::default();
    let result = diagonals(&mut recording, 1_000_000.0, 0.0001, 100);
    assert!(!result.completed);
    assert!(recording.commands().len() <= 100);
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
        assert!(recording.commands().is_empty());
    }
    let mut recording = Recording::default();
    let result = diagonals(&mut recording, 0.0, 0.1, 1000);
    assert!(result.completed && !absent::is_absent(&result.result));
    assert!(recording.commands().is_empty());

    let result = diagonals(&mut recording, 1.0, f64::from_bits(1), 1000);
    assert!(!result.completed);
}

#[test]
fn example_tool_tips_compensate_contact_normal_and_spindle_axis() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut contact = Recording::default();
    let a = run(&mut contact, |scope| {
        ::grap::apply_scoped(
            &names["crosshatch"].into(),
            [
                (names["face"], top_face(&sources, &names)),
                (
                    names["compensation"],
                    ::grap::evaluate(
                        &::grap::lambda(
                            [TOOL_AXIS],
                            ::grap::lambda(
                                [X, Y, Z],
                                ::grap::call(
                                    POINT.into(),
                                    [X, Y, Z].map(|id| (id, Value::from(id))),
                                ),
                            ),
                        ),
                        &sources,
                        100,
                    )
                    .result,
                ),
                (names["orientation"], names["top"].into()),
                (names["setup_up"], point_value(Axis::Z.vector())),
            ],
            &sources,
            scope,
            500_000,
        )
    });
    let mut centers = Recording::default();
    let b = run(&mut centers, |scope| {
        ::grap::apply_scoped(&names["ball_path"].into(), [], &sources, scope, 500_000)
    });
    assert!(
        a.completed && b.completed,
        "contact {:?}, centers {:?}",
        a.result,
        b.result
    );
    assert!(!absent::is_absent(&b.result), "{:?}", b.result);
    assert_eq!(contact.commands().len(), centers.commands().len());
    let radius = f64::read(doc.cells.value(names["tool_diameter"]).unwrap()).unwrap() / 2.0;
    let mut axis = Axis::Z;
    for (a, b) in contact.commands().iter().zip(&centers.commands()) {
        if let Command::StartAt(_, next) = b {
            axis = *next;
        }
        let (a, b) = match (a, b) {
            (Command::StartAt(a, _), Command::StartAt(b, _))
            | (Command::LineTo(a), Command::LineTo(b)) => (a, b),
            _ => panic!("path boundaries must survive compensation"),
        };
        let [x, y, _] = *a;
        let u = (x + 0.4) / 0.8;
        let v = (y + 0.4) / 0.8;
        let nx = 2.5 * (1.0 - 2.0 * u) * v * (1.0 - v);
        let ny = 2.5 * (1.0 - 2.0 * v) * u * (1.0 - u);
        let length = nx.hypot(ny).hypot(1.0);
        for i in 0..3 {
            assert!(
                (b[i] + radius * axis.vector()[i] - a[i] - radius * [nx, ny, 1.0][i] / length)
                    .abs()
                    < 1e-12
            );
        }
    }
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
        ::grap::apply_scoped(
            &names["crosshatch"].into(),
            [
                (names["face"], top_face(&sources, &names)),
                (
                    names["compensation"],
                    ::grap::evaluate(
                        &::grap::lambda(
                            [TOOL_AXIS],
                            ::grap::lambda(
                                [X, Y, Z],
                                ::grap::call(
                                    POINT.into(),
                                    [X, Y, Z].map(|id| (id, Value::from(id))),
                                ),
                            ),
                        ),
                        &sources,
                        100,
                    )
                    .result,
                ),
                (names["orientation"], names["top"].into()),
                (names["setup_up"], point_value(Axis::Z.vector())),
            ],
            &sources,
            scope,
            500_000,
        )
    });
    assert!(
        evaluation.completed && !absent::is_absent(&evaluation.result),
        "{:?}",
        evaluation.result
    );
    let mut starts = 0;
    for command in recording.commands() {
        let [x, y, z] = match command {
            Command::StartAt(p, _) => {
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

fn example_ball_path() -> (Recording, f64) {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut recording = Recording::default();
    let result = run(&mut recording, |scope| {
        ::grap::apply_scoped(&names["ball_path"].into(), [], &sources, scope, 500_000)
    });
    assert!(result.completed && !absent::is_absent(&result.result));
    let radius = f64::read(doc.cells.value(names["tool_diameter"]).unwrap()).unwrap() / 2.0;
    (recording, radius)
}

#[test]
fn example_stock_removal_is_seekable_and_only_removes_completed_cuts() {
    let (recording, radius) = example_ball_path();
    let tool = super::cutter::Tool::ball(radius * 2.0, 0.22).unwrap();
    let points: Vec<[f32; 3]> = (0..=20)
        .flat_map(|y| {
            (0..=20).flat_map(move |x| {
                [0.38, 0.4, 0.45, 0.49]
                    .map(|z| [x as f32 * 0.045 - 0.45, y as f32 * 0.045 - 0.45, z])
            })
        })
        .collect();
    let simulate = |progress| {
        let mut stock = super::stock::Stock::block([-0.5; 3], [0.5; 3]).unwrap();
        recording
            .playback::<InvalidPath>(progress, |a, b, axis, _, completed| {
                if completed {
                    stock.cut(&tool, a, b, axis, 0.001)?;
                }
                Ok(())
            })
            .unwrap();
        use fidget_engine::{shape::EzShape, vm::VmShape};
        let shape = VmShape::from(stock.into_field());
        let mut evaluator = VmShape::new_float_slice_eval();
        let tape = shape.ez_float_slice_tape();
        let coordinates: [Vec<_>; 3] =
            std::array::from_fn(|i| points.iter().map(|p| p[i]).collect());
        evaluator
            .eval(&tape, &coordinates[0], &coordinates[1], &coordinates[2])
            .unwrap()
            .to_vec()
    };
    let start = simulate(0.0);
    let middle = simulate(0.35);
    let end = simulate(1.0);
    let back = simulate(0.35);
    let mut middle_changes = 0;
    let mut final_changes = 0;
    for (i, ((a, b), c)) in start.iter().zip(&middle).zip(&end).enumerate() {
        assert!(*a < 0.0);
        assert!(*b < 0.0 || *c >= 0.0, "removed stock must stay removed");
        assert_eq!(*b, back[i]);
        middle_changes += usize::from(*b > 0.0);
        final_changes += usize::from(*c > 0.0);
    }
    assert!(middle_changes > 0 && final_changes > middle_changes);
    assert!(end[0] < 0.0, "the corner remains stock");
}

#[test]
#[ignore = "measures Fidget stock expression construction, compilation, and meshing"]
fn stock_meshing_profile() {
    use fidget_engine::{
        mesh::{Octree, Settings},
        vm::VmShape,
    };
    use nalgebra::{Scale3, Translation3};
    use std::time::Instant;
    let now = Instant::now();
    let (recording, radius) = example_ball_path();
    eprintln!(
        "Fixture setup + Grap path generation: {:.2?}; {} segments",
        now.elapsed(),
        recording.segments().count()
    );
    let tool = super::cutter::Tool::ball(radius * 2.0, 0.22).unwrap();
    for progress in [0.0, 0.35, 1.0] {
        for depth in [5, 6, 7] {
            let now = Instant::now();
            let mut stock = super::stock::Stock::block([-0.5; 3], [0.5; 3]).unwrap();
            recording
                .playback::<InvalidPath>(progress, |a, b, axis, _, completed| {
                    if completed {
                        stock.cut(&tool, a, b, axis, 0.001)?;
                    }
                    Ok(())
                })
                .unwrap();
            let expression = now.elapsed();
            let now = Instant::now();
            let shape = VmShape::from(stock.into_field()).try_into().unwrap();
            let compile = now.elapsed();
            let settings = Settings {
                depth,
                world_to_model: Translation3::new(0.0, 0.0, 0.125).to_homogeneous()
                    * Scale3::new(0.6, 0.6, 0.725).to_homogeneous(),
                ..Default::default()
            };
            let now = Instant::now();
            let mesh = Octree::build(&shape, &settings).unwrap().walk_dual();
            eprintln!(
                "stock {progress:.2}, depth {depth}: expression {expression:.2?}, compile {compile:.2?}, mesh {:.2?}, {} triangles",
                now.elapsed(),
                mesh.triangles.len()
            );
            assert!(!mesh.triangles.is_empty());
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.iter().all(|x| x.is_finite()))
            );
        }
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn example_tubes_compile_without_gpu_memory_operations() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut tubes = super::fidget::Tubes::new(0.005).unwrap();
    let evaluation = run(&mut tubes, |scope| {
        ::grap::apply_scoped(
            &names["crosshatch"].into(),
            [
                (names["face"], top_face(&sources, &names)),
                (
                    names["compensation"],
                    ::grap::evaluate(
                        &::grap::lambda(
                            [TOOL_AXIS],
                            ::grap::lambda(
                                [X, Y, Z],
                                ::grap::call(
                                    POINT.into(),
                                    [X, Y, Z].map(|id| (id, Value::from(id))),
                                ),
                            ),
                        ),
                        &sources,
                        100,
                    )
                    .result,
                ),
                (names["orientation"], names["top"].into()),
                (names["setup_up"], point_value(Axis::Z.vector())),
            ],
            &sources,
            scope,
            500_000,
        )
    });
    assert!(evaluation.completed && !absent::is_absent(&evaluation.result));
    let paths = tubes.scene();
    assert_eq!(paths.len(), 42);
    for (index, object) in paths.into_iter().enumerate() {
        let shape = fidget_engine::vm::VmShape::from(object.tree);
        assert!(
            !shape.inner().data().iter_asm().any(|op| matches!(
                op,
                fidget_engine::compiler::RegOp::Load(..)
                    | fidget_engine::compiler::RegOp::Store(..)
            )),
            "path {index} requires spilling, which Fidget's GPU interpreter does not implement"
        );
        fidget_engine::wgpu::RenderShape::new(&shape).unwrap();
    }
}
