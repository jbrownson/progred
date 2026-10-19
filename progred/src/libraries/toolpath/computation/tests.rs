use super::super::vocabulary::*;
use super::*;
use crate::libraries::{control, f64};
use cam::paths::{Cursor, InvalidPath};

/// The pane's occurrence at `path`, where the tree program's work is kept.
fn site(root: &crate::display::widget::view::Root, path: &[gid::Step]) -> crate::editing::Site {
    crate::editing::Site::new(root.clone(), std::rc::Rc::from(path), Default::default())
}

struct ViewportEnv<'a> {
    sources: crate::sources::Sources<'a>,
    computations: &'a Computations,
    root: &'a crate::display::widget::view::Root,
}

impl crate::display::Env for ViewportEnv<'_> {
    fn apply_expression_runtime(
        &self,
        function: &RuntimeValue,
        arguments: &[(gid::CellId, RuntimeValue)],
    ) -> RuntimeValue {
        self.sources.apply_expression_runtime(function, arguments)
    }

    fn evaluate_runtime(
        &self,
        expression: &RuntimeValue,
        fuel: usize,
        steps: &[gid::Step],
    ) -> RuntimeValue {
        self.sources.evaluate_runtime(expression, fuel, steps)
    }

    fn evaluate_runtime_memo(
        &self,
        expression: &RuntimeValue,
        fuel: usize,
        steps: &[gid::Step],
    ) -> RuntimeValue {
        self.sources.evaluate_runtime_memo(expression, fuel, steps)
    }

    fn apply_runtime_scoped(
        &self,
        function: &RuntimeValue,
        arguments: &[(gid::CellId, RuntimeValue)],
        scope: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation {
        self.sources
            .apply_runtime_scoped(function, arguments, scope)
    }

    fn apply_runtime_memo(
        &self,
        function: &Value,
        arguments: &[(gid::CellId, Value)],
        fuel: usize,
    ) -> RuntimeValue {
        self.computations
            .apply_runtime(self.root, &[], function, arguments, fuel)
    }
}

#[test]
fn cam_resize_retains_prepared_program_and_recording_but_edits_invalidate() {
    use crate::libraries::{controls::vocabulary as c, layout::vocabulary as l, presentation};
    use std::{rc::Rc, time::Instant};

    let (mut doc, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let computations = Computations::default();
    let root = crate::display::widget::view::Root::document();
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let program = computations.runtime.input(RuntimeValue::record([]));
    let paths = recording(
        &computations,
        program.clone(),
        computations.runtime.input(10_000_000),
    );
    let prepare = |doc: &gid::Document, width, height| {
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        let sources = crate::sources::Sources {
            doc,
            libraries: &libraries,
        };
        let env = ViewportEnv {
            sources,
            computations: &computations,
            root: &root,
        };
        let output = presentation::tests::viewport_output(
            sources.resolve_path(&pane.path).unwrap(),
            &env,
            width,
            height,
        )
        .unwrap();
        let controls = output
            .as_record()
            .unwrap()
            .get(&c::WITH_CONTROLS)
            .unwrap()
            .as_record()
            .unwrap();
        assert_eq!(controls.get(&l::WIDTH).and_then(f64::read), Some(width));
        assert_eq!(controls.get(&l::HEIGHT).and_then(f64::read), Some(height));
        let program = controls
            .get(&presentation::vocabulary::VALUE)
            .unwrap()
            .clone();
        crate::libraries::tree::prepared(
            &computations,
            &site(&root, &pane.path),
            program.into(),
            300_000,
        )
        .as_ref()
        .as_ref()
        .unwrap()
        .items
        .clone()
    };
    let tree = prepare(&doc, 400.0, 750.0);
    program.set_by(tree.clone(), RuntimeValue::same_result);
    let recorded = computations.runtime.read(&paths).unwrap();
    assert!(recorded.path().is_ok());
    for (width, height) in [
        (401.0, 750.0),
        (402.0, 749.0),
        (600.0, 900.0),
        (400.0, 750.0),
    ] {
        let start = Instant::now();
        let resized = prepare(&doc, width, height);
        assert!(
            tree.same_result(&resized),
            "resize must keep the exact prepared tree"
        );
        program.set_by(resized, RuntimeValue::same_result);
        assert!(Rc::ptr_eq(
            &recorded,
            &computations.runtime.read(&paths).unwrap()
        ));
        eprintln!(
            "resize {width}x{height}: prepare, controls declaration, and recording demand {:?}",
            start.elapsed()
        );
    }
    doc.cells.set_value(names["tilt"], f64::value(30.0));
    let edited = prepare(&doc, 400.0, 750.0);
    assert!(!edited.same_result(&tree));
    program.set_by(edited, RuntimeValue::same_result);
    let changed = computations.runtime.read(&paths).unwrap();
    assert!(changed.path().is_ok());
    assert!(!Rc::ptr_eq(&recorded, &changed));
}

#[test]
#[ignore = "manual uncached Grap construction benchmark"]
fn profile_program_tree_construction() {
    use crate::libraries::presentation;
    use std::time::Instant;

    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let declaration = sources.resolve_path(&pane.path).unwrap();
    let computations = crate::computations::Computations::from_sources(sources);
    let root = crate::display::widget::view::Root::document();
    let samples: Vec<_> = (0..5)
        .map(|trial| {
            let start = Instant::now();
            let tree = crate::libraries::tree::build(
                &Value::from(names["program_tree"]).into(),
                &sources,
                300_000,
            )
            .unwrap();
            let tree_time = start.elapsed();
            let start = Instant::now();
            let items = tree.items.as_value();
            let materialization_time = start.elapsed();
            let start = Instant::now();
            let selection = crate::libraries::controls::tree_range::Selection::new(items, None);
            let selection_time = start.elapsed();
            assert_eq!(selection.leaves, 0..504);
            let start = Instant::now();
            let view = presentation::tests::viewport_output(declaration, &sources, 400.0, 750.0).unwrap();
            let view_time = start.elapsed();
            assert!(!::grap::absent::is_absent(&view));
            let start = Instant::now();
            let memoized = crate::libraries::tree::prepared(
                &computations,
                &site(&root, &pane.path),
                Value::from(names["program_tree"]).into(),
                300_000,
            );
            let memo_time = start.elapsed();
            assert!(memoized.is_ok());
            eprintln!(
                "trial {trial}: tree {tree_time:?}; GID view {materialization_time:?}; selectors {selection_time:?}; controls declaration {view_time:?}; memo demand {memo_time:?}"
            );
            tree_time + materialization_time
        })
        .collect();
    crate::perf::record("", samples);
}

fn line(x: f64) -> Value {
    let point = |function, x| {
        ::grap::call(
            function,
            [
                (X, f64::value(x)),
                (Y, f64::value(0.0)),
                (Z, f64::value(0.0)),
            ],
        )
    };
    ::grap::lambda(
        [],
        ::grap::call(
            control::vocabulary::DO.into(),
            [(
                control::vocabulary::EXPRESSIONS,
                Value::list([point(START_AT.into(), x), point(LINE_TO.into(), x + 1.0)]),
            )],
        ),
    )
}

#[test]
fn grouped_recording_preserves_leaf_order_absents_and_fuel() {
    let libraries = crate::stack::load().libraries;
    let tree = Value::list([
        Value::list([line(0.0), line(5.0)]),
        Value::list([line(10.0)]),
    ]);
    let mut recording = Recording::default();
    let evaluation = record_program(&tree.clone().into(), &libraries, 10000, &mut recording);
    assert!(evaluation.completed && !evaluation.result.is_absent());
    let pose = recording
        .playback_cursor::<InvalidPath>(Cursor::Progress(0.5), Some(1..2), |_, _, _, _, _| Ok(()))
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(pose.tip, [5.5, 0.0, 0.0]);
    let mut direct = Recording::default();
    let evaluation = run(&mut direct, |scope| {
        ::grap::evaluate_runtime_scoped(
            &::grap::RuntimeValue::from(::grap::call(SEQUENCE.into(), [(PROGRAM, tree.clone())])),
            &libraries,
            scope,
            10000,
        )
    });
    assert!(evaluation.completed && !evaluation.result.is_absent());
    assert_eq!(
        recording.segments().collect::<Vec<_>>(),
        direct.segments().collect::<Vec<_>>()
    );

    let failure = ::grap::absent::value(INVALID_INPUT);
    let invalid = Value::list([line(0.0), ::grap::lambda([], failure.clone()), line(10.0)]);
    let mut recording = Recording::default();
    let result = record_program(&invalid.into(), &libraries, 10000, &mut recording);
    assert_eq!(result.result.as_value(), &failure);
    assert_eq!(recording.segments().count(), 1);
    let mut recording = Recording::default();
    assert!(!record_program(&tree.into(), &libraries, 1, &mut recording).completed);
}

#[test]
fn recording_memo_retains_runtime_results_and_source_distinctions() {
    use std::rc::Rc;
    let host = crate::stack::load().libraries;
    let doc = gid::Document::default();
    let computations = Computations::from_sources(crate::sources::Sources {
        doc: &doc,
        libraries: &host,
    });
    let callbacks = [X, Y].map(|source| {
        ::grap::evaluate_runtime_at(
            &::grap::RuntimeValue::from(::grap::lambda([], f64::value(7.0))),
            Some(::grap::SourceOrigin::Stored(vec![gid::Step::Key(source)])),
            &host,
            100,
        )
        .result
    });
    let maker = ::grap::evaluate(
        &::grap::lambda([X], ::grap::lambda([], X.into())),
        &host,
        100,
    )
    .result;
    for fail in [false, true] {
        let payloads = callbacks.clone().map(|callback| {
            if fail {
                RuntimeValue::record([
                    (::grap::absent::ABSENT, Value::from(INVALID_INPUT).into()),
                    (X, callback),
                ])
            } else {
                callback
            }
        });
        let programs = payloads.clone().map(|payload| {
            RuntimeValue::list([RuntimeValue::list([::grap::apply(
                &maker,
                [(X, payload)],
                &host,
                None,
                100,
            )
            .result])])
        });
        assert_ne!(programs[0].to_value(), programs[1].to_value());
        let input = computations.runtime.input(programs[0].clone());
        let memo = recording(
            &computations,
            input.clone(),
            computations.runtime.input(1000),
        );
        let first = computations.runtime.read(&memo).unwrap();
        input.set_by(programs[0].clone(), RuntimeValue::same_result);
        assert!(Rc::ptr_eq(
            &first,
            &computations.runtime.read(&memo).unwrap()
        ));
        input.set_by(programs[1].clone(), RuntimeValue::same_result);
        let second = computations.runtime.read(&memo).unwrap();
        assert!(!Rc::ptr_eq(&first, &second));
        for (recorded, expected) in [first, second].iter().zip(&payloads) {
            assert!(recorded.evaluation.completed);
            assert!(recorded.evaluation.result.same_result(expected));
            assert_eq!(recorded.path().is_err(), fail);
        }
    }
}

#[test]
fn cube_grouping_records_the_same_cuts_as_the_complete_program() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let tree = crate::libraries::tree::build(
        &Value::from(names["program_tree"]).into(),
        &sources,
        300_000,
    )
    .unwrap();
    fn shape(value: &Value) -> (usize, usize) {
        value.as_list().map_or((1, 0), |list| {
            list.values()
                .map(shape)
                .fold((0, 0), |(leaves, height), (n, h)| {
                    (leaves + n, height.max(h + 1))
                })
        })
    }
    assert_eq!(shape(tree.items.as_value()), (504, 5));
    let ops = tree.items.as_value().as_list().unwrap();
    assert_eq!(ops.len(), 2);
    for op in ops.values() {
        assert_eq!(
            op.as_list().unwrap().len(),
            2,
            "knurling and chamfers remain separate subgroups"
        );
    }
    fn links(node: &crate::libraries::tree::Node) -> usize {
        usize::from(node.source.is_some())
            + node
                .children
                .iter()
                .flat_map(|children| children.values())
                .map(links)
                .sum::<usize>()
    }
    assert!(
        links(&tree.root) > 504,
        "groups and leaves both carry source links"
    );
    let mut recording = Recording::default();
    let evaluation = record_program(&tree.items, &sources, 3_000_000, &mut recording);
    assert!(
        evaluation.completed && !evaluation.result.is_absent(),
        "{:?}",
        evaluation.result
    );
    let mut direct = Recording::default();
    let evaluation = run(&mut direct, |scope| {
        ::grap::apply_expression(
            &Value::from(names["preview_operations"]).into(),
            [],
            &sources,
            Some(scope),
            3_000_000,
        )
    });
    assert!(
        evaluation.completed && !evaluation.result.is_absent(),
        "{:?}",
        evaluation.result
    );
    assert_eq!(
        recording.tool_segments().collect::<Vec<_>>(),
        direct.tool_segments().collect::<Vec<_>>()
    );
    assert!(
        recording
            .playback_cursor::<InvalidPath>(Cursor::Progress(0.0), Some(6..8), |_, _, _, _, _| Ok(
                ()
            ))
            .unwrap()
            .is_some()
    );
}
