use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::projection::test_support::ProjectionInput;

use crate::display::Env;
use crate::display::test_support::{EvaluatesTo, ProjectionCall, inspect, unshared};
use crate::identity::short_id;
use crate::libraries::test_widgets::{project_runtime, projection_input};
use crate::library::Libraries;
use crate::test_support::{
    activated, completion_entries_with, core_libraries, library_offers, src,
};
use gid::{Cells, Document, new_cell_id};

#[test]
fn every_built_in_declares_its_parameters() {
    let stack = crate::stack::load();
    let undeclared: Vec<String> = stack
        .libraries
        .iter()
        .flat_map(|(library, definitions)| {
            let library = definitions
                .value(library)
                .and_then(name::read)
                .map(str::to_owned)
                .unwrap_or_else(|| short_id(library));
            definitions
                .iter()
                .filter(|(_, definition)| {
                    matches!(definition, ::grap::Definition::Foreign(_))
                        && !definition
                            .value()
                            .as_record()
                            .is_some_and(|fields| fields.contains_key(&PARAMS))
                })
                .map(move |(cell, definition)| {
                    let name = name::read(definition.value())
                        .map(str::to_owned)
                        .unwrap_or_else(|| short_id(cell));
                    format!("{library}: {name}")
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        undeclared.is_empty(),
        "built-ins without declared parameters: {undeclared:#?}"
    );
}

#[test]
fn call_suggestions_order_document_before_library_and_by_name_not_identity() {
    use crate::display::{CompletionRequest, CompletionScope};
    let library = new_cell_id();
    let mut cells = [new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id()];
    cells.sort();
    let definitions = [
        (gid::Resolution::Library(library), "Zoo"),
        (gid::Resolution::Document, "sun"),
        (gid::Resolution::Library(library), "apple"),
        (gid::Resolution::Document, "Clouds"),
    ]
    .map(|(source, name)| {
        (
            source,
            name::record(name, [(PARAMS, Value::list([])), (BODY, Value::record([]))]),
        )
    });
    for order in [cells.to_vec(), cells.iter().rev().copied().collect()] {
        let resolve = |cell| {
            cells
                .iter()
                .position(|candidate| *candidate == cell)
                .map(|index| ResolvedCell {
                    resolution: definitions[index].0,
                    value: &definitions[index].1,
                    native: false,
                })
        };
        let enumerate = || order.clone();
        let request = CompletionRequest {
            resolve: &resolve,
            cells: &enumerate,
            ..crate::test_support::bare_request(
                CompletionKind::Value,
                CompletionScope::Everything,
                "",
            )
        };
        assert_eq!(
            call_completions(&request)
                .iter()
                .map(|offer| offer.test_value())
                .collect::<Vec<_>>(),
            [cells[3], cells[1], cells[2], cells[0]].map(|cell| ::grap::call(cell.into(), [])),
        );
    }
}

fn wrapper(expression: Value, extra: impl IntoIterator<Item = (gid::CellId, Value)>) -> Value {
    Value::record(std::iter::once((EVALUATE, expression)).chain(extra))
}

fn env() -> EvaluatesTo {
    EvaluatesTo(Value::from(vec![1]))
}

#[test]
fn generated_call_and_lambda_shapes_use_runtime_fields_and_parameters() {
    use ::grap::RuntimeValue as R;
    let parameter = new_cell_id();
    let extra = new_cell_id();
    let metadata = new_cell_id();
    let lambda = R::record([
        (PARAMS, R::list([Value::from(parameter).into()])),
        (BODY, Value::from(parameter).into()),
        (metadata, R::list([R::f64(3.0)])),
    ]);
    let call = R::record([
        (FUNCTION, lambda.clone()),
        (extra, R::list([lambda.clone()])),
    ]);
    let env = env();
    let source = Value::record([]);
    let input = projection_input(&env, &source);
    let layout = call_display(&input.with_value(Some(&call))).unwrap();
    assert_eq!(argument_order(&layout), [parameter, extra]);
    assert!(lambda_display(&input.with_value(Some(&lambda))).is_some());

    let malformed = R::record([(PARAMS, R::list([R::f64(3.0)]))]);
    assert!(lambda_display(&input.with_value(Some(&malformed))).is_none());
    let wrapper = R::record([(VALUE, call)]);
    assert!(value_display(&input.with_value(Some(&wrapper))).is_some());
}

fn projected(
    env: &dyn Env,
    value: &Value,
) -> Option<Recorded<crate::Editor, crate::frame::Hovered>> {
    project_runtime(evaluate_display, &projection_input(env, value))
        .map(|layout| layout.record().content().clone())
}

#[test]
fn declaration_cells_follow_their_source_and_edit_the_name_field() {
    struct DefinitionEnv(Value, gid::Resolution);
    impl Env for DefinitionEnv {
        fn apply_expression_runtime(
            &self,
            _: &::grap::RuntimeValue,
            _: &[(CellId, ::grap::RuntimeValue)],
        ) -> ::grap::RuntimeValue {
            panic!("declarations do not evaluate");
        }

        fn evaluate_runtime(
            &self,
            _: &::grap::RuntimeValue,
            _: usize,
            _: &[Step],
        ) -> ::grap::RuntimeValue {
            panic!("declarations do not evaluate");
        }

        fn evaluate_runtime_memo(
            &self,
            _: &::grap::RuntimeValue,
            _: usize,
            _: &[Step],
        ) -> ::grap::RuntimeValue {
            panic!("declarations do not evaluate");
        }

        fn apply_runtime_scoped(
            &self,
            _: &::grap::RuntimeValue,
            _: &[(CellId, ::grap::RuntimeValue)],
            _: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("declarations do not evaluate");
        }

        fn apply_runtime_memo(
            &self,
            _: &Value,
            _: &[(CellId, Value)],
            _: usize,
        ) -> ::grap::RuntimeValue {
            panic!("declarations do not evaluate");
        }

        fn resolve(&self, _: CellId) -> Option<ResolvedCell<'_>> {
            Some(ResolvedCell {
                value: &self.0,
                resolution: self.1,
                native: false,
            })
        }
    }

    for source in [
        gid::Resolution::Document,
        gid::Resolution::Library(new_cell_id()),
    ] {
        for spelling in ["size", ""] {
            let env = DefinitionEnv(name::record(spelling, []), source);
            let cell = Value::from(new_cell_id());
            let declaration =
                declaration_cell(&projection_input(&env, &cell).with_value(Some(&(&cell).into())))
                    .unwrap()
                    .record()
                    .content()
                    .clone();
            let (_, child, _) = crate::display::test_support::delimited(&declaration);
            let ProjectionCall::Descend {
                steps,
                projection: Some(projection),
                ..
            } = &inspect(child)
            else {
                panic!("a declaration follows the cell definition");
            };
            assert_eq!(*steps, [Step::Follow(source)]);
            let ProjectionCall::Descend {
                steps,
                projection: Some(projection),
                ..
            } = &inspect(
                &(projection(&projection_input(&env, &env.0).with_value(Some(&(&env.0).into())))
                    .unwrap()),
            )
            else {
                panic!("the compact definition descends to its actual name");
            };
            assert_eq!(*steps, [Step::Key(name::vocabulary::NAME)]);
            let value = env
                .0
                .as_record()
                .unwrap()
                .get(&name::vocabulary::NAME)
                .unwrap();
            assert!(
                projection(&projection_input(&env, value).with_value(Some(&value.into())))
                    .is_some()
            );
            let line = name::editor(value).unwrap();
            assert_eq!(line.text, spelling);
            assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("", ""));
            assert_eq!(
                (line.update)("next", Some(value)),
                Some(crate::libraries::text::value("next"))
            );
            assert!(
                projection(
                    &projection_input(&env, &Value::record([]))
                        .with_value(Some(&Value::record([]).into()))
                )
                .is_none()
            );
        }
    }
}

#[test]
fn compact_declarations_ignore_metadata_but_decline_invalid_names_and_active_insertions() {
    let env = env();
    for value in [
        Value::record([]),
        Value::record([(name::vocabulary::NAME, Value::from(vec![1]))]),
    ] {
        assert!(declaration_name(&projection_input(&env, &value)).is_none());
    }
    let value = name::record("size", [(new_cell_id(), Value::record([]))]);
    let mut input = projection_input(&env, &value);
    assert!(declaration_name(&input).is_some());
    input.pending = Some(Pending::Field);
    assert!(declaration_name(&input).is_none());
    input.pending = Some(Pending::Child(Step::Key(new_cell_id())));
    assert!(declaration_name(&input).is_none());
}

#[test]
fn value_projection_leaves_competing_forms_to_composition() {
    let env = env();
    let malformed_lambda = Value::record([
        (PARAMS, Value::record([])),
        (BODY, Value::record([])),
        (VALUE, Value::record([])),
    ]);
    let input = projection_input(&env, &malformed_lambda);
    assert!(project_runtime(lambda_display, &input).is_none());
    assert!(project_runtime(value_display, &input).is_some());
    assert!(project_runtime(&*library().projection.unwrap(), &input).is_some());

    let call = Value::record([(FUNCTION, Value::record([])), (VALUE, Value::record([]))]);
    let input = ProjectionInput {
        value: Some(&call),
        ..input
    };
    assert!(project_runtime(call_display, &input).is_some());
    assert!(project_runtime(value_display, &input).is_some());
}

#[test]
fn parameter_metadata_accepts_lambdas_closures_and_aliases_but_not_unknown_callables() {
    let function = new_cell_id();
    let alias = new_cell_id();
    let parameter = new_cell_id();
    let lambda = ::grap::lambda([parameter], Value::record([]));
    let closure = Value::record([(
        ::grap::vocabulary::CLOSURE,
        Value::record([
            (PARAMS, Value::list([parameter.into()])),
            (BODY, Value::record([])),
            (::grap::vocabulary::ENVIRONMENT, Value::record([])),
        ]),
    )]);
    for value in [&lambda, &closure] {
        assert_eq!(
            function_parameters(&value.into(), &|_| panic!(
                "inline function needs no lookup"
            )),
            Some(vec![parameter])
        );
    }
    let reference = Value::from(function);
    let resolve = |cell| {
        Some(ResolvedCell {
            resolution: gid::Resolution::Library(ID),
            value: if cell == alias { &reference } else { &lambda },
            native: false,
        })
    };
    assert_eq!(
        function_parameters(&Value::from(alias).into(), &resolve),
        Some(vec![parameter])
    );
    assert_eq!(function_parameters(&(&reference).into(), &|_| None), None);
    assert_eq!(
        function_parameters(&(&reference).into(), &|_| Some(ResolvedCell {
            resolution: gid::Resolution::Document,
            value: &reference,
            native: false,
        })),
        None
    );
    let declared = Value::record([(PARAMS, Value::list([parameter.into()]))]);
    for (description, expected) in [
        (&Value::record([]), None),
        (&declared, Some(vec![parameter])),
    ] {
        assert_eq!(
            function_parameters(&(&reference).into(), &|_| Some(ResolvedCell {
                resolution: gid::Resolution::Library(ID),
                value: description,
                native: true,
            })),
            expected
        );
    }
    for invalid in [
        Value::record([]),
        ::grap::call(reference.clone(), []),
        Value::record([
            (PARAMS, Value::list([Value::record([])])),
            (BODY, Value::record([])),
        ]),
    ] {
        assert_eq!(
            function_parameters(&(&invalid).into(), &|_| panic!(
                "metadata must not evaluate a call"
            )),
            None
        );
    }
}

fn arms(
    layout: &Recorded<crate::Editor, crate::frame::Hovered>,
) -> (
    &Recorded<crate::Editor, crate::frame::Hovered>,
    &Recorded<crate::Editor, crate::frame::Hovered>,
) {
    let Recorded::Alternatives(options) = layout.content() else {
        panic!("expected alternatives");
    };
    let Some(Recorded::Row { children, .. }) = options.first().map(Recorded::content) else {
        panic!("expected a row first");
    };
    assert_eq!(children.len(), 3);
    (unshared(&children[0]), unshared(&children[2]))
}

fn argument_order(layout: &impl Recordable<crate::Editor, crate::frame::Hovered>) -> Vec<CellId> {
    let Recorded::Alternatives(call_options) = layout.record().content().clone() else {
        panic!("call has responsive forms");
    };
    let Recorded::Row { children, .. } = call_options[0].content() else {
        panic!("flat call first");
    };
    let (_, child, _) = crate::display::test_support::delimited(unshared(&children[1]));
    let Recorded::Alternatives(argument_options) = child else {
        panic!("arguments have responsive forms");
    };
    let Recorded::Row { children, .. } = argument_options[0].content() else {
        panic!("flat arguments first");
    };
    children
        .iter()
        .step_by(2)
        .map(|argument| {
            let Recorded::Row { children, .. } = argument.content() else {
                panic!("argument has a label and value");
            };
            let ProjectionCall::Descend { steps, .. } = &inspect(&(unshared(&children[2]))) else {
                panic!("argument value retains its path");
            };
            let [Step::Key(field)] = steps.as_slice() else {
                panic!("argument path is its field");
            };
            *field
        })
        .collect()
}

#[test]
fn a_record_with_the_field_is_expression_then_result() {
    let expression = Value::from(vec![0]);
    let layout = projected(&env(), &wrapper(expression.clone(), [])).unwrap();
    let (shown, result) = arms(&layout);
    assert!(matches!(&inspect(&(shown)),
        ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(EVALUATE)]
    ));
    assert!(matches!(&inspect(&(result)),
        ProjectionCall::At { value, steps, .. } if *value == Value::from(vec![1])
            && *steps == [Step::Key(crate::libraries::presentation::vocabulary::RESULT)]
    ));
}

#[test]
fn wrapped_evaluation_keeps_the_arrow_with_its_source_and_indents_the_result() {
    let layout = projected(&env(), &wrapper(Value::from(vec![0]), [])).unwrap();
    let Recorded::Alternatives(options) = layout.content() else {
        panic!("evaluation has responsive forms");
    };
    let Recorded::Row {
        children: inline, ..
    } = options[0].content()
    else {
        panic!("inline evaluation");
    };
    let Recorded::Col {
        baseline: 0,
        children: wrapped,
        ..
    } = options[1].content()
    else {
        panic!("wrapped evaluation uses its source baseline");
    };
    let Recorded::Row { children: head, .. } = wrapped[0].content() else {
        panic!("source and arrow stay on the same line");
    };
    let Recorded::Pad {
        left,
        child: result,
        ..
    } = wrapped[1].content()
    else {
        panic!("result is indented below the source");
    };
    assert!(*left > 0.0);
    assert_eq!(inline.len(), 3);
    assert_eq!(wrapped.len(), 2);
    assert_eq!(head.len(), 2);
    // Both alternatives reuse the same projected occurrences and arrow.
    for (inline, wrapped) in inline.iter().zip([&head[0], &head[1], &**result]) {
        let (Recorded::Shared { id: a, .. }, Recorded::Shared { id: b, .. }) = (inline, wrapped)
        else {
            panic!("evaluation children are shared across alternatives");
        };
        assert_eq!(a, b);
    }
}

#[test]
fn other_fields_do_not_block_recognition() {
    assert!(matches!(
        projected(
            &env(),
            &wrapper(
                Value::from(vec![0]),
                [(new_cell_id(), Value::from(vec![2]))]
            ),
        ),
        Some(layout) if matches!(layout.content(), Recorded::Alternatives(_))
    ));
}

#[test]
fn an_evaluate_shaped_result_is_another_projection() {
    let inner = Value::from(vec![2]);
    let result = wrapper(inner.clone(), []);
    let layout = projected(
        &EvaluatesTo(result.clone()),
        &wrapper(Value::from(vec![0]), []),
    )
    .unwrap();
    let (_, shown) = arms(&layout);
    assert!(matches!(&inspect(&(shown)),
        ProjectionCall::At { value, .. } if *value == result
    ));
    let layout = projected(&env(), &result).unwrap();
    let (nested, _) = arms(&layout);
    assert!(matches!(&inspect(&(nested)),
        ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(EVALUATE)]
    ));
}

#[test]
fn a_call_projects_its_function_cell_shallowly() {
    let function = new_cell_id();
    let argument = new_cell_id();
    let layout = project_runtime(
        call_display,
        &projection_input(
            &env(),
            &::grap::call(Value::from(function), [(argument, Value::from(vec![1]))]),
        ),
    )
    .unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("call has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("flat call first");
    };
    assert!(matches!(&inspect(&(unshared(&children[0]))),
        ProjectionCall::Descend {
            steps,
            projection: Some(_),
            ..
        } if *steps == [Step::Key(FUNCTION)]
    ));
}

#[test]
fn a_call_argument_label_targets_its_value() {
    let function = new_cell_id();
    let argument = new_cell_id();
    let call = ::grap::call(Value::from(function), [(argument, Value::from(vec![1]))]);
    let layout = project_runtime(call_display, &projection_input(&env(), &call)).unwrap();
    let Recorded::Alternatives(call_options) = layout.record().content().clone() else {
        panic!("call has responsive forms");
    };
    let Recorded::Row { children, .. } = call_options[0].content() else {
        panic!("flat call first");
    };
    let (left, child, right) = crate::display::test_support::delimited(unshared(&children[1]));
    crate::libraries::test_widgets::assert_delimiter(
        left,
        crate::display::Delim::Brace,
        puri::delim::Side::Open,
    );
    crate::libraries::test_widgets::assert_delimiter(
        right,
        crate::display::Delim::Brace,
        puri::delim::Side::Close,
    );
    let Recorded::Alternatives(argument_options) = child else {
        panic!("arguments have responsive forms");
    };
    let Recorded::Row { children, .. } = argument_options[0].content() else {
        panic!("flat arguments first");
    };
    let Recorded::Row { children, .. } = children[0].content() else {
        panic!("argument has a label and value");
    };
    let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
        panic!("field head contains its label and colon");
    };
    let Recorded::Before { child, .. } = &head[0] else {
        panic!("the label targets its argument");
    };
    assert_eq!(
        crate::libraries::test_widgets::claim(&head[0]),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![Step::Key(argument)])
        ))
    );
    assert!(matches!(child.as_ref(), Recorded::Before { .. }));
}

#[test]
fn a_call_uses_its_stored_lambdas_parameter_order_then_stable_extras() {
    const FUNCTION_CELL: CellId = CellId::from_u128(10);
    const FIRST_PARAMETER: CellId = CellId::from_u128(30);
    const SECOND_PARAMETER: CellId = CellId::from_u128(20);
    const FIRST_EXTRA: CellId = CellId::from_u128(40);
    const SECOND_EXTRA: CellId = CellId::from_u128(50);

    struct DefinitionEnv {
        definition: Value,
        native: bool,
    }

    impl Env for DefinitionEnv {
        fn apply_expression_runtime(
            &self,
            _: &::grap::RuntimeValue,
            _: &[(CellId, ::grap::RuntimeValue)],
        ) -> ::grap::RuntimeValue {
            panic!("unexpected projection application")
        }

        fn evaluate_runtime(
            &self,
            _: &::grap::RuntimeValue,
            _: usize,
            _: &[Step],
        ) -> ::grap::RuntimeValue {
            Value::record([]).into()
        }

        fn evaluate_runtime_memo(
            &self,
            _: &::grap::RuntimeValue,
            _: usize,
            _: &[Step],
        ) -> ::grap::RuntimeValue {
            Value::record([]).into()
        }

        fn apply_runtime_scoped(
            &self,
            _: &::grap::RuntimeValue,
            _: &[(CellId, ::grap::RuntimeValue)],
            _: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("unexpected projection application")
        }

        fn apply_runtime_memo(
            &self,
            _: &Value,
            _: &[(CellId, Value)],
            _: usize,
        ) -> ::grap::RuntimeValue {
            panic!("unexpected projection application")
        }

        fn resolve(&self, cell: CellId) -> Option<ResolvedCell<'_>> {
            (cell == FUNCTION_CELL).then_some(ResolvedCell {
                resolution: gid::Resolution::Document,
                value: &self.definition,
                native: self.native,
            })
        }
    }

    let env = DefinitionEnv {
        native: false,
        definition: ::grap::lambda(
            [FIRST_PARAMETER, SECOND_PARAMETER],
            Value::from(FIRST_PARAMETER),
        ),
    };
    let call = ::grap::call(
        Value::from(FUNCTION_CELL),
        [
            (SECOND_EXTRA, Value::from(vec![5])),
            (SECOND_PARAMETER, Value::from(vec![2])),
            (FIRST_EXTRA, Value::from(vec![4])),
            (FIRST_PARAMETER, Value::from(vec![3])),
        ],
    );
    let layout = project_runtime(call_display, &projection_input(&env, &call)).unwrap();

    assert_eq!(
        argument_order(&layout),
        [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA, SECOND_EXTRA,]
    );
    let incomplete = ::grap::call(
        FUNCTION_CELL.into(),
        [
            (SECOND_PARAMETER, Value::from(vec![2])),
            (FIRST_EXTRA, Value::from(vec![4])),
        ],
    );
    let initial = projection_input(&env, &incomplete);
    assert_eq!(
        argument_order(&project_runtime(call_display, &initial).unwrap()),
        [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA]
    );
    let editing = ProjectionInput {
        pending: Some(Pending::Child(Step::Key(FIRST_PARAMETER))),
        ..initial
    };
    assert_eq!(
        argument_order(&project_runtime(call_display, &editing).unwrap()),
        [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA]
    );
    let unknown = new_cell_id();
    let editing = ProjectionInput {
        pending: Some(Pending::Child(Step::Key(unknown))),
        ..editing
    };
    assert_eq!(
        argument_order(&project_runtime(call_display, &editing).unwrap()),
        [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA, unknown]
    );
    let declared = DefinitionEnv {
        native: true,
        definition: Value::record([(
            PARAMS,
            Value::list([FIRST_PARAMETER.into(), SECOND_PARAMETER.into()]),
        )]),
    };
    assert_eq!(
        argument_order(
            &project_runtime(call_display, &projection_input(&declared, &call)).unwrap()
        ),
        [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA, SECOND_EXTRA],
    );
    assert_eq!(
        argument_order(
            &project_runtime(call_display, &projection_input(&declared, &incomplete)).unwrap()
        ),
        [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA]
    );
    let undeclared = DefinitionEnv {
        native: true,
        definition: Value::record([]),
    };
    assert_eq!(
        argument_order(
            &project_runtime(call_display, &projection_input(&undeclared, &call)).unwrap()
        ),
        [SECOND_PARAMETER, FIRST_PARAMETER, FIRST_EXTRA, SECOND_EXTRA],
    );
    assert_eq!(
        argument_order(
            &project_runtime(call_display, &projection_input(&undeclared, &incomplete)).unwrap()
        ),
        [SECOND_PARAMETER, FIRST_EXTRA]
    );
}

#[test]
fn a_call_uses_an_inline_lambdas_parameter_order() {
    const FIRST_PARAMETER: CellId = CellId::from_u128(2);
    const SECOND_PARAMETER: CellId = CellId::from_u128(1);
    let call = ::grap::call(
        ::grap::lambda(
            [FIRST_PARAMETER, SECOND_PARAMETER],
            Value::from(FIRST_PARAMETER),
        ),
        [
            (SECOND_PARAMETER, Value::from(vec![1])),
            (FIRST_PARAMETER, Value::from(vec![2])),
        ],
    );
    let layout = project_runtime(call_display, &projection_input(&env(), &call)).unwrap();

    assert_eq!(argument_order(&layout), [FIRST_PARAMETER, SECOND_PARAMETER]);
    let empty = ::grap::call(
        ::grap::lambda(
            [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_PARAMETER],
            Value::record([]),
        ),
        [],
    );
    assert_eq!(
        argument_order(&project_runtime(call_display, &projection_input(&env(), &empty)).unwrap()),
        [FIRST_PARAMETER, SECOND_PARAMETER]
    );
}

#[test]
fn a_lambda_targets_its_syntax_and_projects_its_body_as_grap() {
    let parameter = new_cell_id();
    let definition = ::grap::lambda([parameter], Value::from(parameter));
    let layout = project_runtime(lambda_display, &projection_input(&env(), &definition)).unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("lambda has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("flat lambda first");
    };
    let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
        panic!("lambda has a syntax head");
    };
    assert!(matches!(&inspect(&(&head[0])),
        ProjectionCall::Descend {
            steps,
            projection: Some(_),
            ..
        } if *steps == [Step::Key(name::vocabulary::NAME)]
    ));
    assert!(matches!(&inspect(&(&head[1])),
        ProjectionCall::Descend {
            steps,
            projection: Some(_),
            ..
        } if *steps == [Step::Key(PARAMS)]
    ));
    let Recorded::Before { child, .. } = &head[2] else {
        panic!("lambda arrow targets its body");
    };
    assert_eq!(
        crate::libraries::test_widgets::claim(&head[2]),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![Step::Key(BODY)])
        ))
    );
    assert!(matches!(child.as_ref(), Recorded::Before { .. }));
    assert!(matches!(
        &inspect(&(unshared(&children[1]))),
        ProjectionCall::Descend {
            steps,
            projection: Some(_),
            ..
        } if *steps == [Step::Key(BODY)]
    ));
}

#[test]
fn an_unfinished_lambda_keeps_its_missing_body_visible() {
    let env = env();
    let definition = Value::record([
        (PARAMS, Value::list([])),
        (new_cell_id(), Value::record([])),
    ]);
    let mut input = projection_input(&env, &definition);
    assert!(project_runtime(lambda_display, &input).is_some());
    input.pending = Some(Pending::Child(Step::Key(BODY)));
    assert!(project_runtime(lambda_display, &input).is_some());
    input.pending = Some(Pending::Child(Step::Key(new_cell_id())));
    assert!(project_runtime(lambda_display, &input).is_none());

    for malformed in [
        Value::record([]),
        Value::record([(PARAMS, Value::record([]))]),
        Value::record([(
            PARAMS,
            Value::list([crate::libraries::text::value("not a cell")]),
        )]),
    ] {
        assert!(project_runtime(lambda_display, &projection_input(&env, &malformed)).is_none());
    }
}

#[test]
fn a_named_lambda_projects_its_editable_name_in_place_of_the_marker() {
    let definition = name::record(
        "tree",
        [
            (PARAMS, Value::list([])),
            (BODY, Value::from(vec![1])),
            (new_cell_id(), Value::record([])),
        ],
    );
    let layout = project_runtime(lambda_display, &projection_input(&env(), &definition)).unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("lambda has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("flat lambda first");
    };
    let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
        panic!("lambda has a syntax head");
    };
    let ProjectionCall::Descend {
        steps,
        projection: Some(projection),
        ..
    } = &inspect(&(&head[0]))
    else {
        panic!("lambda name is projected contextually");
    };
    assert_eq!(*steps, [Step::Key(name::vocabulary::NAME)]);
    let value = definition
        .as_record()
        .unwrap()
        .get(&name::vocabulary::NAME)
        .unwrap();
    assert!(projection(&projection_input(&env(), value).with_value(Some(&value.into()))).is_some());
    let line = name::editor(value).unwrap();
    assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("", ""));
}

#[test]
fn the_lambda_name_partial_handles_missing_names_or_text() {
    let env = env();
    let value = Value::record([]);
    let mut input = projection_input(&env, &value);
    input.value = None;
    assert!(lambda_name(&input).is_some());

    // Selected, a missing name is a hole to type it into.
    let selected = crate::selection::payload::empty_pending();
    input.selection = Some(&selected);
    assert!(lambda_name(&input).is_some());
    input.selection = None;
    input.value = Some(&value);
    assert!(lambda_name(&input).is_none());

    let absent = ::grap::absent::value(gid::new_cell_id());
    input.value = Some(&absent);
    assert!(lambda_name(&input).is_none());
}

#[test]
fn an_explicit_empty_lambda_name_projects_as_an_empty_string() {
    let definition = name::record(
        "",
        [(PARAMS, Value::list([])), (BODY, Value::from(vec![1]))],
    );
    let layout = project_runtime(lambda_display, &projection_input(&env(), &definition)).unwrap();
    let Recorded::Alternatives(options) = layout.record().content().clone() else {
        panic!("lambda has responsive forms");
    };
    let Recorded::Row { children, .. } = options[0].content() else {
        panic!("flat lambda first");
    };
    let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
        panic!("lambda has a syntax head");
    };
    let ProjectionCall::Descend {
        projection: Some(projection),
        ..
    } = &inspect(&(&head[0]))
    else {
        panic!("lambda name is projected contextually");
    };
    let value = definition
        .as_record()
        .unwrap()
        .get(&name::vocabulary::NAME)
        .unwrap();
    assert!(projection(&projection_input(&env(), value).with_value(Some(&value.into()))).is_some());
    let line = name::editor(value).unwrap();
    assert_eq!(line.text, "");
    assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("", ""));
}

#[test]
fn the_library_describes_grap_and_owns_evaluate() {
    let library = library::<crate::Editor>();
    assert_eq!(
        library
            .definitions
            .value(::grap::vocabulary::FUNCTION)
            .and_then(name::read),
        Some("function")
    );
    assert_eq!(
        library
            .definitions
            .value(::grap::absent::MISSING_CELL)
            .and_then(name::read),
        Some("missing cell")
    );

    let input = gid::new_cell_id();
    let expression = ::grap::call(
        ::grap::lambda([input], Value::from(input)),
        [(input, Value::from(b"evaluated".to_vec()))],
    );
    let evaluation = crate::test_support::test_evaluate(
        &::grap::call(
            Value::from(::grap::vocabulary::EVALUATE),
            [
                (::grap::vocabulary::EXPRESSION, expression),
                (::grap::vocabulary::ENVIRONMENT, Value::record([])),
            ],
        ),
        |_| None,
        &crate::test_support::foreign_functions(&library),
        40,
    );
    assert_eq!(evaluation.result, Value::from(b"evaluated".to_vec()));
}

#[test]
fn document_function_completion_inserts_a_call_and_opens_its_first_argument() {
    let (document, names) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../website/public/lessons/growing-forest.gid"
    )))
    .unwrap();
    let clouds = names["clouds"];
    let time = names["sun_time"];
    let libraries = core_libraries();
    let sources = src(&document, &libraries);
    let everything = library_offers(
        crate::stack::load().completions,
        crate::display::CompletionScope::Everything,
    );
    let entries = completion_entries_with(
        &sources,
        &CompletionKind::Value,
        "clouds",
        None,
        Some(&everything),
    );
    let call = entries.first().unwrap();
    assert_eq!(call.detail.as_deref(), Some("call"));
    let result = activated(call);
    assert_eq!(result.value, Some(::grap::call(clouds.into(), [])));
    let (selected, payload) = result.selected.unwrap();
    assert_eq!(selected, vec![Step::Key(time)]);
    assert_eq!(
        payload
            .as_record()
            .unwrap()
            .get(&crate::libraries::selection::vocabulary::STAGE),
        Some(&crate::libraries::selection::vocabulary::PENDING.into())
    );
}

#[test]
fn call_offers_follow_resolved_definitions_without_evaluating_them() {
    let function = new_cell_id();
    let mut cells = Cells::new();
    let library = crate::library::Library::<crate::Editor, crate::frame::Hovered>::new(
        crate::library::Definitions::from_parts(
            {
                let mut definitions = Cells::new();
                definitions.set_value(function, name::record("probe", []));
                definitions
            },
            ::grap::ForeignFunctions::default().register(
                function,
                ::grap::ForeignFunction::from_value(|_, _, _| {
                    panic!("completion must not run code")
                }),
            ),
        ),
    );
    let libraries = Libraries::from_contributions([(new_cell_id(), library)]).0;
    let root = Some(Value::list([]));
    let document = Document {
        root: root.clone(),
        cells: cells.clone(),
    };
    let everything = library_offers(
        crate::stack::load().completions,
        crate::display::CompletionScope::Everything,
    );
    let offers = |document: &Document| {
        completion_entries_with(
            &src(document, &libraries),
            &CompletionKind::Value,
            "probe",
            None,
            Some(&everything),
        )
    };
    let entries = offers(&document);
    assert_eq!(
        activated(entries.first().unwrap()).value,
        Some(::grap::call(function.into(), []))
    );

    cells.set_value(function, name::record("probe", []));
    let document = Document { root, cells };
    assert!(
        !offers(&document)
            .iter()
            .any(|entry| entry.detail.as_deref() == Some("call"))
    );
}

#[test]
fn built_in_call_completion_opens_its_first_argument() {
    use crate::libraries::grap::numbers::{f64::SUM, vocabulary::LEFT};
    let document = Document::default();
    let libraries = core_libraries();
    let everything = library_offers(
        crate::stack::load().completions,
        crate::display::CompletionScope::Everything,
    );
    let entries = completion_entries_with(
        &src(&document, &libraries),
        &CompletionKind::Value,
        "+",
        None,
        Some(&everything),
    );
    let call = entries
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("call · grap"))
        .expect("+ is offered as Grap's call");
    let result = activated(call);
    assert_eq!(result.value, Some(::grap::call(SUM.into(), [])));
    let (selected, payload) = result.selected.unwrap();
    assert_eq!(selected, vec![Step::Key(LEFT)]);
    assert_eq!(
        payload
            .as_record()
            .unwrap()
            .get(&crate::libraries::selection::vocabulary::STAGE),
        Some(&crate::libraries::selection::vocabulary::PENDING.into())
    );
}

#[test]
fn grap_results_take_no_typing() {
    use crate::libraries::{absent, f64, name, presentation, text};
    let (doc, names) = crate::gid_text::parse(include_str!("../../../fixtures/grap.gid")).unwrap();
    let mut editor = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[name::ID, text::ID, absent::ID, ID, f64::ID]).unwrap(),
    );
    crate::test_support::set_tutorial_slots(
        &mut editor,
        &[names["first"], names["second"], names["third"]],
    );
    let mut driver = crate::driver::Driver::new(editor, crate::driver::Driver::VIEWPORT);
    driver.select(&[
        Step::Key(names["second"]),
        Step::Key(presentation::vocabulary::RESULT),
    ]);
    assert!(
        driver
            .editor()
            .model
            .selection
            .as_ref()
            .unwrap()
            .location()
            .is_none()
    );
    let before = driver.editor().model.doc.clone();
    assert!(!driver.keystroke(
        ui_events::keyboard::Key::Character("9".into()),
        ui_events::keyboard::Modifiers::empty()
    ));
    assert!(std::rc::Rc::ptr_eq(&driver.editor().model.doc, &before));
}

#[test]
fn a_parameter_slot_offers_a_new_parameter_named_as_typed() {
    use ::grap::vocabulary::PARAMS;
    use ui_events::keyboard::{Key, NamedKey};
    let function = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(function, Value::record([(PARAMS, Value::list([]))]));
    let mut driver = crate::driver::Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([(vocabulary::GRAP, function.into())])),
            cells,
        }),
        crate::driver::Driver::VIEWPORT,
    );
    let params = [
        Step::Key(vocabulary::GRAP),
        Step::Follow(gid::Resolution::Document),
        Step::Key(PARAMS),
    ];
    driver.select(&params);
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), crate::test_support::command()));
    driver.type_text("x");
    let entries = driver
        .picker_entries()
        .expect("the empty parameter shows its picker");
    assert_eq!(entries[0], "x");
    assert_eq!(driver.picker_detail("x").as_deref(), Some("new parameter"));
    driver.press(NamedKey::Enter);
    let added = driver
        .value(&params)
        .as_list()
        .and_then(|list| list.values().next().cloned())
        .and_then(|value| value.as_cell())
        .expect("the list holds the new parameter");
    assert_eq!(
        driver
            .editor()
            .sources()
            .resolve(added)
            .map(|definition| definition.value.clone()),
        Some(crate::libraries::name::record("x", []))
    );
}

mod declarations;
