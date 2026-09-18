//! Dependency-tracked path generation, shared by the rendering interpretations.

use super::{paths::Recording, run};
use crate::{computations::Computations, libraries::absent};
use gid::Value;
use incremental::{Input, Memo};
use std::sync::Arc;

pub(super) type Outcome<T> = Result<T, Value>;

#[derive(PartialEq)]
pub(super) struct Recorded {
    pub path: Arc<Recording>,
    pub evaluation: ::grap::Evaluation,
}

impl Recorded {
    pub fn path(&self) -> Outcome<&Recording> {
        if self.evaluation.completed && !absent::is_absent(&self.evaluation.result) {
            Ok(&self.path)
        } else {
            Err(self.evaluation.result.clone())
        }
    }
}

pub(super) fn recording(
    computations: &Computations,
    program: Input<Value>,
    fuel: Input<usize>,
) -> Memo<Recorded> {
    let definitions = computations.definitions.clone();
    computations.runtime.memo(move |read| {
        let program = program.read(read);
        let fuel = *fuel.read(read);
        let mut path = Recording::default();
        let evaluation = ::grap::memo::with_recorded_effects(&definitions, read, |host| {
            record_program(&program, host, fuel, &mut path)
        });
        Ok(Recorded {
            path: Arc::new(path),
            evaluation,
        })
    })
}

/// Interpret an ordinary nested list of zero-argument programs. A single
/// callable remains a one-part program. Fuel and dependency observation span
/// all leaves; each leaf records independently so focus never needs to rerun it.
fn record_program(
    program: &Value,
    host: &dyn ::grap::Host,
    fuel: usize,
    output: &mut Recording,
) -> ::grap::Evaluation {
    if program.as_list().is_none() {
        return run(output, |scope| {
            ::grap::apply_scoped(program, [], host, scope, fuel)
        });
    }
    let mut evaluation = ::grap::Evaluation {
        result: Value::record([]),
        remaining_fuel: fuel,
        completed: true,
    };
    let mut pending = vec![program];
    while let Some(program) = pending.pop() {
        if let Some(list) = program.as_list() {
            pending.extend(list.values().rev());
        } else {
            let mut part = Recording::default();
            evaluation = run(&mut part, |scope| {
                ::grap::apply_scoped(program, [], host, scope, evaluation.remaining_fuel)
            });
            if !evaluation.completed || absent::is_absent(&evaluation.result) {
                return evaluation;
            }
            output.append_part(part);
        }
    }
    evaluation
}

#[cfg(test)]
mod tests {
    use super::super::{paths::InvalidPath, vocabulary::*};
    use super::*;
    use crate::libraries::{control, f64};

    #[test]
    #[ignore = "manual uncached Grap construction benchmark"]
    fn profile_program_tree_construction() {
        use crate::libraries::{layout::vocabulary as l, presentation};
        use std::time::Instant;

        let (doc, names) =
            crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
        let libraries = crate::stack::load().libraries;
        let sources = crate::sources::Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
        let (value, viewport) =
            presentation::viewport(sources.resolve_path(&pane.path).unwrap()).unwrap();
        let declaration = ::grap::apply(
            viewport,
            [
                (presentation::vocabulary::VALUE, value.clone()),
                (l::WIDTH, f64::value(400.0)),
                (l::HEIGHT, f64::value(750.0)),
            ],
            &sources,
            10_000,
        )
        .result;
        let render = declaration
            .as_record()
            .unwrap()
            .get(&presentation::vocabulary::RENDER)
            .unwrap()
            .as_record()
            .unwrap();
        let expression = render.get(&::grap::vocabulary::EXPRESSION).unwrap();
        let computations = crate::computations::Computations::from_sources(sources);
        let root = crate::workspace::Root::document();
        for trial in 0..5 {
            let start = Instant::now();
            let tree = ::grap::apply(&names["program_tree"].into(), [], &sources, 300_000);
            let tree_time = start.elapsed();
            assert!(tree.completed && !absent::is_absent(&tree.result));
            let start = Instant::now();
            let selection =
                crate::libraries::controls::tree_range::Selection::new(&tree.result, None);
            let selection_time = start.elapsed();
            assert_eq!(selection.leaves, 0..504);
            let start = Instant::now();
            let view = ::grap::evaluate(expression, &sources, 300_000);
            let view_time = start.elapsed();
            assert!(view.completed && !absent::is_absent(&view.result));
            let start = Instant::now();
            let memoized = computations.evaluate(&root, &[], expression, 300_000);
            let memo_time = start.elapsed();
            assert!(!absent::is_absent(&memoized));
            eprintln!(
                "trial {trial}: tree {tree_time:?}; selectors {selection_time:?}; tree + controls declaration {view_time:?}; memo demand {memo_time:?}"
            );
        }
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
        let evaluation = record_program(&tree, &libraries, 10000, &mut recording);
        assert!(evaluation.completed && !absent::is_absent(&evaluation.result));
        let pose = recording
            .playback_parts::<InvalidPath>(0.5, Some(1..2), |_, _, _, _, _| Ok(()))
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(pose.tip, [5.5, 0.0, 0.0]);
        let mut direct = Recording::default();
        let evaluation = run(&mut direct, |scope| {
            ::grap::evaluate_scoped(
                &::grap::call(SEQUENCE.into(), [(PROGRAM, tree.clone())]),
                &libraries,
                scope,
                10000,
            )
        });
        assert!(evaluation.completed && !absent::is_absent(&evaluation.result));
        assert_eq!(
            recording.segments().collect::<Vec<_>>(),
            direct.segments().collect::<Vec<_>>()
        );

        let failure = absent::with_reason(INVALID_INPUT);
        let invalid = Value::list([line(0.0), ::grap::lambda([], failure.clone()), line(10.0)]);
        let mut recording = Recording::default();
        let result = record_program(&invalid, &libraries, 10000, &mut recording);
        assert_eq!(result.result, failure);
        assert_eq!(recording.segments().count(), 1);
        let mut recording = Recording::default();
        assert!(!record_program(&tree, &libraries, 1, &mut recording).completed);
    }

    #[test]
    fn cube_grouping_records_the_same_cuts_as_the_complete_program() {
        let (doc, names) =
            crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
        let libraries = crate::stack::load().libraries;
        let sources = crate::sources::Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let tree = ::grap::apply(&names["program_tree"].into(), [], &sources, 300_000);
        assert!(
            tree.completed && !absent::is_absent(&tree.result),
            "{:?}",
            tree.result
        );
        fn shape(value: &Value) -> (usize, usize) {
            value.as_list().map_or((1, 0), |list| {
                list.values()
                    .map(shape)
                    .fold((0, 0), |(leaves, height), (n, h)| {
                        (leaves + n, height.max(h + 1))
                    })
            })
        }
        assert_eq!(shape(&tree.result), (504, 5));
        let mut recording = Recording::default();
        let evaluation = record_program(&tree.result, &sources, 3_000_000, &mut recording);
        assert!(
            evaluation.completed && !absent::is_absent(&evaluation.result),
            "{:?}",
            evaluation.result
        );
        let mut direct = Recording::default();
        let evaluation = run(&mut direct, |scope| {
            ::grap::apply_scoped(
                &names["preview_operations"].into(),
                [],
                &sources,
                scope,
                3_000_000,
            )
        });
        assert!(
            evaluation.completed && !absent::is_absent(&evaluation.result),
            "{:?}",
            evaluation.result
        );
        assert_eq!(
            recording.tool_segments().collect::<Vec<_>>(),
            direct.tool_segments().collect::<Vec<_>>()
        );
        assert!(
            recording
                .playback_parts::<InvalidPath>(0.0, Some(6..8), |_, _, _, _, _| Ok(()))
                .unwrap()
                .is_some()
        );
    }
}
