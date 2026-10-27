use super::*;
use gid::new_cell_id;
use std::cell::Cell;

fn definitions_from_parts<'a>(
    resolve: impl Fn(CellId) -> Option<Value> + 'a,
    foreign: &'a ForeignFunctions,
) -> impl Host + 'a {
    TestHost(move |cell| {
        match (resolve(cell), foreign.get(cell)) {
            (value, Some(function)) => Some(Definition::foreign(
                cell,
                value.unwrap_or_else(|| Value::record([])),
                function.clone(),
            )),
            (Some(value), None) => Some(Definition::Value(value)),
            (None, None) => None,
        }
        .into_iter()
        .collect()
    })
}

fn evaluate(
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> Evaluation<Value> {
    super::evaluate_value(expression, &definitions_from_parts(resolve, foreign), fuel)
}

fn apply(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> Evaluation<Value> {
    super::apply_value(
        function,
        arguments,
        &definitions_from_parts(resolve, foreign),
        None,
        fuel,
    )
}

#[test]
fn direct_cell_calls_return_decline_without_trying_another_definition() {
    let function = new_cell_id();
    let expression = call(Value::from(function), []);
    let evaluation = super::evaluate_value(
        &expression,
        &TestHost(|cell| {
            assert_eq!(cell, function);
            vec![
                Definition::foreign(
                    cell,
                    gid::Value::record([]),
                    ForeignFunction::from_value(|_, _, _| Ok(absent::decline())),
                ),
                Definition::Value(Value::record([])),
                Definition::Value(lambda([], Value::from(b"grap".to_vec()))),
                Definition::foreign(
                    cell,
                    gid::Value::record([]),
                    ForeignFunction::from_value(|_, _, _| Ok(Value::from(b"too late".to_vec()))),
                ),
            ]
        }),
        20,
    );

    assert_eq!(evaluation.result, absent::decline());
}

#[test]
fn an_ordinary_absent_stops_dispatch_with_its_details() {
    let function = new_cell_id();
    let first_missing = new_cell_id();
    let second_missing = new_cell_id();
    let evaluation = super::evaluate_value(
        &call(Value::from(function), []),
        &TestHost(|cell| match cell {
            cell if cell == function => vec![
                Definition::Value(Value::from(first_missing)),
                Definition::Value(Value::from(second_missing)),
            ],
            cell if cell == first_missing => Vec::new(),
            _ => unreachable!(),
        }),
        20,
    );

    assert_eq!(
        evaluation.result,
        absent::with_detail(absent::MISSING_CELL, absent::CELL, first_missing.into()),
    );
}

#[test]
fn host_apply_returns_the_selected_definitions_decline() {
    let function = new_cell_id();
    let evaluation = super::apply_value(
        &Value::from(function),
        [],
        &TestHost(|_| {
            vec![
                Definition::foreign(
                    function,
                    gid::Value::record([]),
                    ForeignFunction::from_value(|_, _, _| Ok(absent::decline())),
                ),
                Definition::Value(lambda([], Value::from(b"grap".to_vec()))),
            ]
        }),
        None,
        20,
    );

    assert_eq!(evaluation.result, absent::decline());
}

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

#[test]
fn ordinary_structures_are_inert_data() {
    let field = new_cell_id();
    let cell = new_cell_id();
    let value = Value::record([(field, Value::list([Value::from(cell)]))]);
    let evaluation = evaluate(
        &value,
        |_| panic!("inert data must not resolve cells"),
        &ForeignFunctions::default(),
        10,
    );
    assert_eq!(evaluation.result, value);
}

#[test]
fn value_wrappers_evaluate_only_their_expression_in_the_calling_environment() {
    let parameter = new_cell_id();
    let metadata = new_cell_id();
    let wrapper = Value::record([
        (isa::ISA, vocabulary::kind::VALUE_WRAPPER.into()),
        (vocabulary::VALUE, parameter.into()),
        (metadata, new_cell_id().into()),
    ]);
    let expression = call(lambda([parameter], wrapper), [(parameter, blob("local"))]);
    let evaluation = evaluate(
        &expression,
        |_| panic!("neither the bound parameter nor wrapper metadata needs resolving"),
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(evaluation.result, blob("local"));
    assert!(evaluation.completed);
}

#[test]
fn value_wrappers_can_be_anonymous_nested_and_callable() {
    let parameter = new_cell_id();
    let cell = new_cell_id();
    let wrapper = isa::record(
        vocabulary::kind::VALUE_WRAPPER,
        [(
            vocabulary::VALUE,
            isa::record(
                vocabulary::kind::VALUE_WRAPPER,
                [(vocabulary::VALUE, lambda([parameter], parameter.into()))],
            ),
        )],
    );
    let evaluation = evaluate(
        &call(cell.into(), [(parameter, blob("argument"))]),
        |queried| (queried == cell).then(|| wrapper.clone()),
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(evaluation.result, blob("argument"));
}

#[test]
fn referencing_a_value_wrapper_again_reruns_its_expression() {
    let cell = new_cell_id();
    let function = new_cell_id();
    let first = new_cell_id();
    let second = new_cell_id();
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let foreign = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(move |context, _, _| {
            Ok(context.effect(|| {
                count.set(count.get() + 1);
                blob("result")
            }))
        }),
    );
    let wrapper = isa::record(
        vocabulary::kind::VALUE_WRAPPER,
        [(vocabulary::VALUE, call(function.into(), []))],
    );
    let evaluation = evaluate(
        &call(
            lambda([first, second], first.into()),
            [(first, cell.into()), (second, cell.into())],
        ),
        |queried| (queried == cell).then(|| wrapper.clone()),
        &foreign,
        40,
    );
    assert_eq!(evaluation.result, blob("result"));
    assert_eq!(calls.get(), 2);
}

#[test]
fn value_wrappers_do_not_evaluate_inside_data_or_foreign_results() {
    let wrapper = isa::record(
        vocabulary::kind::VALUE_WRAPPER,
        [(vocabulary::VALUE, new_cell_id().into())],
    );
    let foreign_cell = new_cell_id();
    let result = wrapper.clone();
    let foreign = ForeignFunctions::default().register(
        foreign_cell,
        ForeignFunction::from_value(move |_, _, _| Ok(result.clone())),
    );
    for data in [
        Value::list([wrapper.clone()]),
        Value::record([(new_cell_id(), wrapper.clone())]),
    ] {
        assert_eq!(
            evaluate(&data, |_| panic!("inert data"), &foreign, 10).result,
            data,
        );
    }
    assert_eq!(
        evaluate(&call(foreign_cell.into(), []), |_| None, &foreign, 10).result,
        wrapper,
    );
}

#[test]
fn value_wrappers_propagate_absence_cycles_and_fuel_exhaustion() {
    let foreign = ForeignFunctions::default();
    let absent = absent::value(absent::MISSING_ARGUMENT);
    let wrapper = isa::record(
        vocabulary::kind::VALUE_WRAPPER,
        [(vocabulary::VALUE, absent.clone())],
    );
    let evaluation = evaluate(&wrapper, |_| None, &foreign, 10);
    assert_eq!(evaluation.result, absent);
    assert!(evaluation.completed);
    let exhausted = evaluate(&wrapper, |_| None, &foreign, 2);
    assert_eq!(exhausted.result, absent::value(absent::FUEL_EXHAUSTED));
    assert!(!exhausted.completed);

    let cell = new_cell_id();
    let wrapper = isa::record(
        vocabulary::kind::VALUE_WRAPPER,
        [(vocabulary::VALUE, cell.into())],
    );
    let cycle = evaluate(&cell.into(), |_| Some(wrapper.clone()), &foreign, 20);
    assert_eq!(absent::reason(&cycle.result), Some(absent::CELL_CYCLE));
}

#[test]
fn value_wrappers_preserve_the_inner_expression_source() {
    let cell = new_cell_id();
    let function = new_cell_id();
    let functions = [function];
    let origin = RefCell::new(None);
    let scoped = |_, context: &mut Context<'_>, call: &Expression, _: &Environment| {
        *origin.borrow_mut() = context.source_origin(call);
        Ok(blob("result"))
    };
    let wrapper = isa::record(
        vocabulary::kind::VALUE_WRAPPER,
        [(vocabulary::VALUE, call(function.into(), []))],
    );
    let evaluation = super::evaluate_runtime_scoped(
        &RuntimeValue::from(Value::from(cell)),
        &definitions_from_parts(
            |queried| (queried == cell).then(|| wrapper.clone()),
            &ForeignFunctions::default(),
        ),
        &ForeignOverlay::from_value(&functions, &scoped),
        20,
    )
    .into_value();
    assert_eq!(evaluation.result, blob("result"));
    assert_eq!(
        origin.into_inner(),
        Some(SourceOrigin::Cell {
            cell,
            path: vec![gid::Step::Key(vocabulary::VALUE)],
        }),
    );
}

#[test]
fn consuming_the_entire_fuel_allowance_is_exhaustion() {
    let value = blob("one step");
    let exhausted = evaluate(&value, |_| None, &ForeignFunctions::default(), 1);
    assert_eq!(exhausted.result, absent::value(absent::FUEL_EXHAUSTED));
    assert_eq!(exhausted.remaining_fuel, 0);
    let completed = evaluate(&value, |_| None, &ForeignFunctions::default(), 2);
    assert_eq!(completed.result, value);
    assert_eq!(completed.remaining_fuel, 1);
}

#[test]
fn a_staged_foreign_prepares_once_per_compiled_call_site() {
    let staged = new_cell_id();
    let repeat = new_cell_id();
    let argument = new_cell_id();
    let preparations = Rc::new(Cell::new(0));
    let count = preparations.clone();
    let foreign = ForeignFunctions::default()
        .register(
            staged,
            ForeignFunction::staged(move |_, _| {
                count.set(count.get() + 1);
                Rc::new(|_, _| Ok(RuntimeValue::from(blob("staged"))))
            }),
        )
        .register(
            repeat,
            ForeignFunction::new(move |context, call, environment| {
                let Some(argument) = context.field(call, argument) else {
                    return Ok(context.missing_runtime_argument(call, argument));
                };
                context.eval(argument.clone(), environment)?;
                context.eval(argument.clone(), environment)
            }),
        );
    let expression = call(
        Value::from(repeat),
        [(argument, call(Value::from(staged), []))],
    );

    assert_eq!(
        evaluate(&expression, |_| None, &foreign, 20).result,
        blob("staged"),
    );
    assert_eq!(preparations.get(), 1);
}

#[test]
fn a_staged_call_reprepares_when_a_scoped_function_changes() {
    let staged = new_cell_id();
    let run_scoped = new_cell_id();
    let argument = new_cell_id();
    let base_preparations = Rc::new(Cell::new(0));
    let scoped_preparations = Rc::new(Cell::new(0));
    let base_count = base_preparations.clone();
    let scoped_count = scoped_preparations.clone();
    let scoped = ForeignFunctions::default().register(
        staged,
        ForeignFunction::staged(move |_, _| {
            scoped_count.set(scoped_count.get() + 1);
            Rc::new(|_, _| Ok(RuntimeValue::from(blob("scoped"))))
        }),
    );
    let foreign = ForeignFunctions::default()
        .register(
            staged,
            ForeignFunction::staged(move |_, _| {
                base_count.set(base_count.get() + 1);
                Rc::new(|_, _| Ok(RuntimeValue::from(blob("base"))))
            }),
        )
        .register(
            run_scoped,
            ForeignFunction::new(move |context, call, environment| {
                let Some(argument) = context.field(call, argument) else {
                    return Ok(context.missing_runtime_argument(call, argument));
                };
                let before = context.eval(argument.clone(), environment)?;
                let during = context.with_foreign_functions(scoped.clone(), |context| {
                    context.eval(argument.clone(), environment)
                })?;
                let after = context.eval(argument.clone(), environment)?;
                Ok(RuntimeValue::list([before, during, after]))
            }),
        );
    let expression = call(
        Value::from(run_scoped),
        [(argument, call(Value::from(staged), []))],
    );

    assert_eq!(
        evaluate(&expression, |_| None, &foreign, 30).result,
        Value::list([blob("base"), blob("scoped"), blob("base")]),
    );
    assert_eq!(base_preparations.get(), 1);
    assert_eq!(scoped_preparations.get(), 1);
}

#[test]
fn cells_evaluate_when_they_are_the_expression() {
    let first = new_cell_id();
    let second = new_cell_id();
    let evaluation = evaluate(
        &Value::from(first),
        |cell| match cell {
            cell if cell == first => Some(Value::from(second)),
            cell if cell == second => Some(blob("done")),
            _ => None,
        },
        &ForeignFunctions::default(),
        10,
    );
    assert_eq!(evaluation.result, blob("done"));
}

#[test]
fn a_call_of_an_absent_function_answers_that_absent() {
    let reason = new_cell_id();
    let called = evaluate(
        &crate::call(absent::value(reason), []),
        |_| None,
        &ForeignFunctions::default(),
        10,
    );
    assert_eq!(
        called.result,
        absent::value(reason),
        "what failed before the call is the reason, not that it isn't callable"
    );
}

#[test]
fn a_record_is_a_call_or_a_lambda_only_when_its_isa_says_so() {
    let function = new_cell_id();
    let resolve = |cell| (cell == function).then(|| crate::lambda([], blob("called")));
    let shaped = Value::record([(vocabulary::FUNCTION, function.into())]);
    assert_eq!(
        evaluate(&shaped, resolve, &ForeignFunctions::default(), 10).result,
        shaped,
        "without an isa, a record with a function is data"
    );
    assert_eq!(
        evaluate(
            &crate::call(function.into(), []),
            resolve,
            &ForeignFunctions::default(),
            10
        )
        .result,
        blob("called")
    );
}

#[test]
fn a_call_or_a_lambda_missing_what_its_kind_requires_answers_why() {
    let unfinished = isa::record(vocabulary::kind::CALL, []);
    assert_eq!(
        evaluate(&unfinished, |_| None, &ForeignFunctions::default(), 10).result,
        absent::missing_field(vocabulary::FUNCTION)
    );
    let parameter = new_cell_id();
    let bodiless = isa::record(
        vocabulary::kind::LAMBDA,
        [(vocabulary::PARAMS, Value::list([Value::from(parameter)]))],
    );
    assert_eq!(
        evaluate(
            &crate::call(bodiless, [(parameter, blob("argument"))]),
            |_| None,
            &ForeignFunctions::default(),
            10
        )
        .result,
        absent::missing_field(vocabulary::BODY)
    );
}

#[test]
fn graph_functions_match_parameter_labelled_fields() {
    let function_cell = new_cell_id();
    let x = new_cell_id();
    let y = new_cell_id();
    let definition = lambda([x, y], Value::from(x));
    let argument = new_cell_id();
    let reads = std::cell::RefCell::new(Vec::new());
    let expression = call(
        Value::from(function_cell),
        [(x, blob("x")), (y, Value::from(argument))],
    );
    let evaluation = evaluate(
        &expression,
        |cell| {
            reads.borrow_mut().push(cell);
            (cell == function_cell).then(|| definition.clone())
        },
        &ForeignFunctions::default(),
        30,
    );
    assert_eq!(evaluation.result, blob("x"));
    assert_eq!(*reads.borrow(), [function_cell, argument]);
}

#[test]
fn parameters_shadow_document_cells() {
    let parameter = new_cell_id();
    let expression = call(
        lambda([parameter], Value::from(parameter)),
        [(parameter, blob("local"))],
    );
    let evaluation = evaluate(
        &expression,
        |_| panic!("a lexical binding must not consult the document"),
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(evaluation.result, blob("local"));
}

#[test]
fn parameters_shadow_foreign_functions() {
    let function = new_cell_id();
    let foreign = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(|_, _, _| Ok(blob("foreign"))),
    );
    let shadowed = call(
        lambda([function], Value::from(function)),
        [(function, blob("bound"))],
    );
    assert_eq!(
        evaluate(&shadowed, |_| None, &foreign, 30).result,
        blob("bound"),
    );
    let unshadowed = evaluate(&call(Value::from(function), []), |_| None, &foreign, 30);
    assert_eq!(unshadowed.result, blob("foreign"));
}

#[test]
fn a_foreign_call_outside_a_shadowing_scope_still_uses_the_registry() {
    let function = new_cell_id();
    let ignored = new_cell_id();
    let foreign = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(|_, _, _| Ok(blob("foreign"))),
    );
    let expression = call(
        lambda([ignored], call(function.into(), [])),
        [(
            ignored,
            call(
                lambda([function], Value::from(function)),
                [(function, blob("local"))],
            ),
        )],
    );
    assert_eq!(
        evaluate(&expression, |_| Some(blob("ordinary data")), &foreign, 100).result,
        blob("foreign")
    );
}

#[test]
fn cell_reads_never_invoke_native_code_or_scoped_capabilities() {
    struct DataHost(CellId, Vec<Definition>);
    impl Host for DataHost {
        fn resolve(&self, cell: CellId) -> Option<Definition> {
            assert_eq!(cell, self.0);
            self.1.first().cloned()
        }
    }
    let cell = new_cell_id();
    let functions = [cell];
    let invoke = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
        panic!("reading data must not invoke a scoped capability")
    };
    let overlay = ForeignOverlay::from_value(&functions, &invoke);
    let first = Definition::Value(blob("document"));
    let second = Definition::foreign(
        cell,
        Value::record([(cell, blob("library"))]),
        ForeignFunction::from_value(|_, _, _| {
            panic!("reading a native definition must not invoke it")
        }),
    );
    // A foreign function's value is its definition, or where only a scope
    // supplies it, a foreign function implemented under its cell.
    for (values, expected) in [
        (vec![], super::foreign_function(cell)),
        (vec![first.clone()], first.value().clone()),
        (vec![second.clone()], second.value().clone()),
        (vec![first.clone(), second], first.value().clone()),
    ] {
        let result = super::evaluate_runtime_scoped(
            &RuntimeValue::from(Value::from(cell)),
            &DataHost(cell, values),
            &overlay,
            20,
        )
        .into_value();
        assert!(result.completed);
        assert_eq!(result.result, expected);
    }
}

#[test]
fn anonymous_functions_capture_their_lexical_environment() {
    let x = new_cell_id();
    let y = new_cell_id();
    let inner = lambda([y], Value::from(x));
    let outer = lambda([x], call(inner, [(y, blob("ignored"))]));
    assert_eq!(
        evaluate(
            &call(outer, [(x, blob("captured"))]),
            |_| None,
            &ForeignFunctions::default(),
            50,
        )
        .result,
        blob("captured")
    );
}

#[test]
fn rust_functions_control_evaluation_while_graph_functions_are_strict() {
    const INPUT: CellId = CellId::from_u128(0x6a0d2c8e1f934b70a5c14e8d2b07f391);
    let hold = new_cell_id();
    let input = INPUT;
    let field = new_cell_id();
    let call_shaped_data = call(Value::from(new_cell_id()), []);
    let held = call(Value::from(hold), [(input, call_shaped_data.clone())]);
    let foreign = ForeignFunctions::default().register(
        hold,
        ForeignFunction::from_value(|context, call, _| match context.field(call, INPUT) {
            Some(value) => Ok(context.value(&value).clone()),
            None => Ok(context.missing_argument(call, INPUT)),
        }),
    );
    let graph = lambda([input], Value::from(input));
    assert_eq!(
        evaluate(&held, |_| None, &foreign, 30,).result,
        call_shaped_data
    );
    assert_eq!(
        evaluate(&call(graph, [(input, held)]), |_| None, &foreign, 30,).result,
        call_shaped_data
    );
    let inert = Value::record([(field, call(Value::from(hold), []))]);
    assert_eq!(evaluate(&inert, |_| None, &foreign, 30).result, inert);
}

#[test]
fn a_foreign_function_reads_as_its_definition_which_says_what_it_is() {
    const INPUT: CellId = CellId::from_u128(0x2e9c4a71b8d0563f91a0c7e4d15b6820);
    let echo = new_cell_id();
    let input = INPUT;
    let foreign = ForeignFunctions::default().register(
        echo,
        ForeignFunction::from_value(|context, call, environment| {
            match context.field(call, INPUT) {
                Some(value) => context.eval_to_value(value, environment),
                None => Ok(context.missing_argument(call, INPUT)),
            }
        }),
    );

    let description = Value::record([(input, blob("its description"))]);
    let defined = isa::record(
        vocabulary::kind::FOREIGN_FUNCTION,
        [
            (vocabulary::IMPLEMENTATION, Value::from(echo)),
            (input, blob("its description")),
        ],
    );
    let evaluation = evaluate(
        &Value::from(echo),
        |_| Some(description.clone()),
        &foreign,
        10,
    );
    assert_eq!(evaluation.result, defined);
    // The value it reads as calls it, as its cell does.
    assert_eq!(
        evaluate(
            &call(defined.clone(), [(input, blob("passed"))]),
            |_| Some(description.clone()),
            &foreign,
            10,
        )
        .result,
        blob("passed")
    );
    assert_eq!(
        evaluate(
            &call(Value::from(echo), [(input, Value::from(echo))]),
            |_| None,
            &foreign,
            10,
        )
        .result,
        super::foreign_function(echo),
        "a foreign function's value is its definition"
    );
}

#[test]
fn foreign_callables_pass_through_grap_bindings_as_values() {
    const INPUT: CellId = CellId::from_u128(0x94b7e20c5d1a836f4e09c2a7b6d3581f);
    let echo = new_cell_id();
    let callable = new_cell_id();
    let input = INPUT;
    let foreign = ForeignFunctions::default().register(
        echo,
        ForeignFunction::from_value(|context, call, environment| {
            match context.field(call, INPUT) {
                Some(value) => context.eval_to_value(value, environment),
                None => Ok(context.missing_argument(call, INPUT)),
            }
        }),
    );
    let apply = lambda(
        [callable, input],
        call(Value::from(callable), [(input, Value::from(input))]),
    );
    let evaluation = evaluate(
        &call(
            apply,
            [(callable, Value::from(echo)), (input, blob("passed"))],
        ),
        |_| None,
        &foreign,
        40,
    );
    assert_eq!(evaluation.result, blob("passed"));
}

#[test]
fn rust_functions_choose_which_raw_operands_to_evaluate() {
    const CONDITION: CellId = CellId::from_u128(0x0c8f3e5a7192b4d6e1a047c59b83d20e);
    const YES: CellId = CellId::from_u128(0x5d21a9c0e8473f6b1a4c80d2e59f37b6);
    const NO: CellId = CellId::from_u128(0x81e4b07c3a952d6f4c10e8a7b5d6392a);
    let choose = new_cell_id();
    let condition = CONDITION;
    let yes = YES;
    let no = NO;
    let parameter = new_cell_id();
    let missing = new_cell_id();
    let foreign = ForeignFunctions::default().register(
        choose,
        ForeignFunction::from_value(|context, call, environment| {
            let Some(condition) = context.field(call, CONDITION) else {
                return Ok(context.missing_argument(call, CONDITION));
            };
            let Some(yes) = context.field(call, YES) else {
                return Ok(context.missing_argument(call, YES));
            };
            let Some(no) = context.field(call, NO) else {
                return Ok(context.missing_argument(call, NO));
            };
            if context.eval_to_value(condition, environment)? == blob("true") {
                context.eval_to_value(yes, environment)
            } else {
                context.eval_to_value(no, environment)
            }
        }),
    );
    let select_parameter = lambda(
        [parameter],
        call(
            Value::from(choose),
            [
                (condition, blob("true")),
                (yes, Value::from(parameter)),
                (no, Value::from(missing)),
            ],
        ),
    );
    let evaluation = evaluate(
        &call(select_parameter, [(parameter, blob("selected"))]),
        |cell| {
            assert_eq!(cell, choose);
            None
        },
        &foreign,
        50,
    );
    assert_eq!(evaluation.result, blob("selected"));
}

#[test]
fn rust_functions_can_expose_the_calling_environment_as_graph_data() {
    let inspect = new_cell_id();
    let parameter = new_cell_id();
    let foreign = ForeignFunctions::default().register(
        inspect,
        ForeignFunction::from_value(|_, _, environment| Ok(Value::from(environment))),
    );
    let inspect_from_body = lambda([parameter], call(Value::from(inspect), []));
    let evaluation = evaluate(
        &call(inspect_from_body, [(parameter, blob("bound"))]),
        |_| None,
        &foreign,
        40,
    );
    assert_eq!(
        evaluation
            .result
            .as_record()
            .and_then(|environment| environment.get(&parameter)),
        Some(&blob("bound"))
    );
}

#[test]
fn rust_functions_evaluate_raw_operands_in_extended_environments() {
    const BINDING: CellId = CellId::from_u128(0xedcd2b19cf89faf94a4a72ab1e02ec31);
    const VALUE: CellId = CellId::from_u128(0x3f7a1c90d2e84b65a0c19e4d7b5826f3);
    const BODY: CellId = CellId::from_u128(0x70d4e8a1c5b2936f4a1e07c8d5b64920);
    let bind = new_cell_id();
    let value = VALUE;
    let body = BODY;
    let foreign = ForeignFunctions::default().register(
        bind,
        ForeignFunction::from_value(|context, call, environment| {
            let Some(value) = context.field(call, VALUE) else {
                return Ok(context.missing_argument(call, VALUE));
            };
            let Some(body) = context.field(call, BODY) else {
                return Ok(context.missing_argument(call, BODY));
            };
            let value = context.eval_to_value(value, environment)?;
            context.eval_to_value(body, &environment.extended([(BINDING, value)]))
        }),
    );
    let evaluation = evaluate(
        &call(
            Value::from(bind),
            [(value, blob("locally bound")), (body, Value::from(BINDING))],
        ),
        |_| None,
        &foreign,
        30,
    );
    assert_eq!(evaluation.result, blob("locally bound"));
}

#[test]
fn returned_closures_are_graph_values_with_graph_environments() {
    let captured = new_cell_id();
    let make_constant = lambda(
        [captured],
        lambda(Vec::<CellId>::new(), Value::from(captured)),
    );
    let closure = evaluate(
        &call(make_constant, [(captured, blob("remembered"))]),
        |_| None,
        &ForeignFunctions::default(),
        40,
    )
    .result;
    assert_eq!(
        evaluate(
            &call(closure.clone(), []),
            |_| None,
            &ForeignFunctions::default(),
            40,
        )
        .result,
        blob("remembered")
    );
    assert_eq!(
        closure
            .as_record()
            .and_then(|closure| closure.get(&vocabulary::ENVIRONMENT))
            .and_then(Value::as_record)
            .and_then(|environment| environment.get(&captured)),
        Some(&blob("remembered"))
    );
}

#[test]
fn absents_carry_stable_reasons_and_occurrence_details() {
    let missing = new_cell_id();
    let evaluation = evaluate(
        &Value::from(missing),
        |_| None,
        &ForeignFunctions::default(),
        10,
    );
    assert_eq!(
        evaluation.result,
        absent::with_detail(absent::MISSING_CELL, absent::CELL, missing.into())
    );

    let malformed = isa::record(
        vocabulary::kind::LAMBDA,
        [
            (vocabulary::PARAMS, blob("not a list")),
            (vocabulary::BODY, blob("body")),
        ],
    );
    let evaluation = evaluate(&malformed, |_| None, &ForeignFunctions::default(), 10);
    assert_eq!(
        evaluation.result,
        absent::with_detail(absent::MALFORMED_LAMBDA, absent::VALUE, malformed)
    );

    let invalid = blob("not a cell");
    let lambda = isa::record(
        vocabulary::kind::LAMBDA,
        [
            (vocabulary::PARAMS, Value::list([invalid.clone()])),
            (vocabulary::BODY, blob("body")),
        ],
    );
    let evaluation = evaluate(&lambda, |_| None, &ForeignFunctions::default(), 10);
    assert_eq!(
        evaluation.result,
        absent::with_detail(absent::INVALID_PARAMETER, absent::VALUE, invalid)
    );
}

#[test]
fn incomplete_lambda_shapes_are_inert_data() {
    for incomplete in [
        Value::record([(vocabulary::PARAMS, Value::list(Vec::<Value>::new()))]),
        Value::record([(vocabulary::BODY, blob("body"))]),
    ] {
        let evaluation = evaluate(&incomplete, |_| None, &ForeignFunctions::default(), 10);
        assert_eq!(evaluation.result, incomplete);
    }
}

#[test]
fn recursion_and_cell_cycles_are_values() {
    let recurse = new_cell_id();
    let parameter = new_cell_id();
    let definition = lambda(
        [parameter],
        call(Value::from(recurse), [(parameter, Value::from(parameter))]),
    );
    let evaluation = evaluate(
        &call(Value::from(recurse), [(parameter, blob("again"))]),
        |cell| (cell == recurse).then(|| definition.clone()),
        &ForeignFunctions::default(),
        30,
    );
    assert_eq!(evaluation.result, absent::value(absent::FUEL_EXHAUSTED));

    let a = new_cell_id();
    let b = new_cell_id();
    let evaluation = evaluate(
        &Value::from(a),
        |cell| match cell {
            cell if cell == a => Some(Value::from(b)),
            cell if cell == b => Some(Value::from(a)),
            _ => None,
        },
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(
        evaluation.result,
        absent::with_detail(
            absent::CELL_CYCLE,
            absent::CYCLE,
            Value::list([a, b, a].map(Value::from))
        )
    );
}

#[test]
fn extra_call_fields_do_not_prevent_calling() {
    let parameter = new_cell_id();
    let extra = new_cell_id();
    let expression = call(
        lambda([parameter], Value::from(parameter)),
        [(parameter, blob("result")), (extra, blob("still GID data"))],
    );
    let evaluation = evaluate(&expression, |_| None, &ForeignFunctions::default(), 10);
    assert_eq!(evaluation.result, blob("result"));
}

#[test]
fn grap_parameters_may_repeat_and_name_the_function_field() {
    let parameter = new_cell_id();
    let definition = lambda(
        [vocabulary::FUNCTION, parameter, parameter],
        Value::from(vocabulary::FUNCTION),
    );
    let evaluation = evaluate(
        &call(definition, [(parameter, blob("argument"))]),
        |_| None,
        &ForeignFunctions::default(),
        30,
    );
    assert!(
        evaluation
            .result
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::LAMBDA))
            .is_some()
    );
}

#[test]
fn foreign_functions_may_consume_the_function_field() {
    const PARAMETER: CellId = CellId::from_u128(0x4b8e0d27c1a9563f80e2c4a7d6b1359e);
    let function = new_cell_id();
    let parameter = PARAMETER;
    let foreign = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(|context, call, _| {
            let Some(function) = context.field(call, vocabulary::FUNCTION) else {
                return Ok(context.missing_argument(call, vocabulary::FUNCTION));
            };
            let Some(value) = context.field(call, PARAMETER) else {
                return Ok(context.missing_argument(call, PARAMETER));
            };
            assert!(context.value(&function).as_cell().is_some());
            Ok(context.value(&value).clone())
        }),
    );
    assert_eq!(
        evaluate(
            &call(Value::from(function), [(parameter, blob("argument"))]),
            |_| None,
            &foreign,
            20,
        )
        .result,
        blob("argument")
    );
}

#[test]
fn a_later_table_overrides_a_shared_cell() {
    let function = new_cell_id();
    let left = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(|_, _, _| Ok(blob("left"))),
    );
    let right = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(|_, _, _| Ok(blob("right"))),
    );
    assert_eq!(
        evaluate(
            &call(Value::from(function), []),
            |_| None,
            &left.merge(right),
            10,
        )
        .result,
        blob("right")
    );
}

#[test]
fn apply_binds_code_shaped_arguments_as_data() {
    let parameter = new_cell_id();
    let identity = lambda([parameter], Value::from(parameter));
    let code_shaped = lambda([new_cell_id()], blob("body"));
    // `call` + `evaluate` closes over the argument; `apply`
    // hands it through untouched.
    assert!(
        evaluate(
            &call(identity.clone(), [(parameter, code_shaped.clone())]),
            |_| None,
            &ForeignFunctions::default(),
            20,
        )
        .result
            != code_shaped
    );
    let applied = apply(
        &identity,
        [(parameter, code_shaped.clone())],
        |_| None,
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(applied.result, code_shaped);
}

#[test]
fn apply_resolves_a_cell_to_its_stored_function() {
    let function = new_cell_id();
    let parameter = new_cell_id();
    let stored = lambda([parameter], Value::from(parameter));
    let applied = apply(
        &Value::from(function),
        [(parameter, blob("argument"))],
        |cell| (cell == function).then(|| stored.clone()),
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(applied.result, blob("argument"));
}

#[test]
fn apply_reaches_foreign_targets_with_the_arguments_as_fields() {
    let function = new_cell_id();
    let foreign = ForeignFunctions::default().register(
        function,
        ForeignFunction::from_value(|context, call, environment| {
            let argument = context.field(call, CellId::from_u128(7)).unwrap();
            context.eval_to_value(argument, environment)
        }),
    );
    let applied = apply(
        &Value::from(function),
        [(CellId::from_u128(7), blob("passed"))],
        |_| None,
        &foreign,
        20,
    );
    assert_eq!(applied.result, blob("passed"));
}

#[test]
fn prepared_calls_pass_a_foreign_function_as_its_value() {
    let control = new_cell_id();
    let foreign = new_cell_id();
    let callable = new_cell_id();
    let argument = new_cell_id();
    let functions = ForeignFunctions::default()
        .register(
            foreign,
            ForeignFunction::from_value(move |context, call, environment| {
                let function = context.field(call, vocabulary::FUNCTION).unwrap();
                let function = context.value(&function).as_record().unwrap();
                assert_eq!(isa::of(function), Some(vocabulary::kind::FOREIGN_FUNCTION));
                assert_eq!(
                    function
                        .get(&vocabulary::IMPLEMENTATION)
                        .and_then(Value::as_cell),
                    Some(foreign)
                );
                let argument = context.field(call, argument).unwrap();
                context.eval_to_value(argument, environment)
            }),
        )
        .register(
            control,
            ForeignFunction::from_value(move |context, call, environment| {
                let callable = context.field(call, callable).unwrap();
                let callable = context.prepare_callable(callable, environment)?;
                context
                    .call_prepared(&callable, [(argument, blob("passed").into())])
                    .map(RuntimeValue::into_value)
            }),
        );
    let evaluation = evaluate(
        &call(Value::from(control), [(callable, Value::from(foreign))]),
        |_| None,
        &functions,
        20,
    );
    assert_eq!(evaluation.result, blob("passed"));
}

#[test]
fn values_evaluated_from_rust_keep_syntax_children_for_native_functions() {
    let outer = new_cell_id();
    let inner = new_cell_id();
    let argument = new_cell_id();
    let functions = [outer, inner];
    let scoped = |function,
                  context: &mut Context<'_>,
                  call_expression: &Expression,
                  environment: &Environment| {
        if function == outer {
            context
                .eval_value_runtime(
                    &call(Value::from(inner), [(argument, Value::list([blob("a")]))]),
                    environment,
                )
                .map(RuntimeValue::into_value)
        } else {
            let walked = context
                .field(call_expression, argument)
                .is_some_and(|list| context.elements(&list).is_some());
            Ok(blob(if walked { "walked" } else { "opaque" }))
        }
    };
    let overlay = ForeignOverlay::from_value(&functions, &scoped);
    let foreign = ForeignFunctions::default();
    let applied = super::apply_value(
        &Value::from(outer),
        [],
        &definitions_from_parts(|_| None, &foreign),
        Some(&overlay),
        20,
    );
    assert_eq!(applied.result, blob("walked"));
}

#[test]
fn a_scoped_foreign_layer_borrows_state_and_remains_reentrant() {
    let outer = new_cell_id();
    let inner = new_cell_id();
    let calls = std::cell::Cell::new(0);
    let functions = [outer, inner];
    let scoped =
        |function, context: &mut Context<'_>, _: &Expression, environment: &Environment| {
            calls.set(calls.get() + 1);
            if function == outer {
                context
                    .eval_value_runtime(&call(Value::from(inner), []), environment)
                    .map(RuntimeValue::into_value)
            } else {
                Ok(blob("scoped"))
            }
        };
    let overlay = ForeignOverlay::from_value(&functions, &scoped);
    let foreign = ForeignFunctions::default();
    let applied = super::apply_value(
        &Value::from(outer),
        [],
        &definitions_from_parts(|_| None, &foreign),
        Some(&overlay),
        20,
    );
    assert_eq!(applied.result, blob("scoped"));
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_scoped_foreign_layer_can_evaluate_a_source_call() {
    let function = new_cell_id();
    let calls = std::cell::Cell::new(0);
    let functions = [function];
    let scoped = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
        calls.set(calls.get() + 1);
        Ok(blob("drawn"))
    };
    let overlay = ForeignOverlay::from_value(&functions, &scoped);
    let foreign = ForeignFunctions::default();
    let evaluation = super::evaluate_runtime_scoped(
        &RuntimeValue::from(call(Value::from(function), [])),
        &definitions_from_parts(|_| None, &foreign),
        &overlay,
        10,
    )
    .into_value();
    assert_eq!(evaluation.result, blob("drawn"));
    assert_eq!(calls.get(), 1);
}

#[test]
fn captured_calls_share_ancestry_and_source_locations() {
    let repeat = new_cell_id();
    let body = new_cell_id();
    let leaf = new_cell_id();
    let captured = Rc::new(RefCell::new(Vec::new()));
    let output = captured.clone();
    let tracer = Rc::new(Tracer::default());
    let observer = tracer.clone();
    let functions = ForeignFunctions::default()
        .register(
            repeat,
            ForeignFunction::from_value(move |context, call, environment| {
                let body = context.field(call, body).unwrap();
                context.eval_to_value(body.clone(), environment)?;
                context.eval_to_value(body.clone(), environment)
            }),
        )
        .register(
            leaf,
            ForeignFunction::from_value(move |_, _, _| {
                let trace = observer.trace().unwrap();
                let again = observer.trace().unwrap();
                assert!(Rc::ptr_eq(&trace.0, &again.0));
                output.borrow_mut().push(trace);
                Ok(blob("done"))
            }),
        );
    let expression = call(repeat.into(), [(body, call(leaf.into(), []))]);
    let result = super::evaluate_value(
        &expression,
        &Observed::new(&definitions_from_parts(|_| None, &functions), &*tracer),
        100,
    );
    assert!(result.completed);
    let captured = captured.borrow();
    assert_eq!(captured.len(), 2);
    assert_eq!(
        captured[0].origins().cloned().collect::<Vec<_>>(),
        vec![
            SourceOrigin::Input(vec![gid::Step::Key(body)]),
            SourceOrigin::Input(vec![]),
        ]
    );
    assert!(Rc::ptr_eq(&captured[0].0.origin, &captured[1].0.origin));
    assert!(Rc::ptr_eq(
        &captured[0].0.caller.as_ref().unwrap().0,
        &captured[1].0.caller.as_ref().unwrap().0,
    ));
}

#[test]
fn generated_prepared_calls_keep_their_source_callers() {
    let invoke = new_cell_id();
    let leaf = new_cell_id();
    let function = new_cell_id();
    let captured = Rc::new(RefCell::new(None));
    let output = captured.clone();
    let tracer = Rc::new(Tracer::default());
    let observer = tracer.clone();
    let functions = ForeignFunctions::default()
        .register(
            invoke,
            ForeignFunction::from_value(move |context, call, environment| {
                let function = context.field(call, function).unwrap();
                let function = context.prepare_callable(function, environment)?;
                context
                    .call_prepared(&function, [])
                    .map(RuntimeValue::into_value)
            }),
        )
        .register(
            leaf,
            ForeignFunction::staged(move |_, _| {
                let (output, observer) = (output.clone(), observer.clone());
                Rc::new(move |_, _| {
                    *output.borrow_mut() = observer.trace();
                    Ok(RuntimeValue::from(blob("done")))
                })
            }),
        );
    let expression = call(
        invoke.into(),
        [(function, lambda([], call(leaf.into(), [])))],
    );
    let result = super::evaluate_value(
        &expression,
        &Observed::new(&definitions_from_parts(|_| None, &functions), &*tracer),
        100,
    );
    assert!(result.completed);
    assert_eq!(
        captured
            .borrow()
            .as_ref()
            .unwrap()
            .origins()
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            SourceOrigin::Input(vec![
                gid::Step::Key(function),
                gid::Step::Key(vocabulary::BODY)
            ]),
            SourceOrigin::Input(vec![]),
        ]
    );
}

#[test]
fn a_foreign_call_retains_its_cell_relative_source_origin() {
    let function = new_cell_id();
    let foreign = new_cell_id();
    let source = RefCell::new(None);
    let functions = [foreign];
    let scoped = |_, context: &mut Context<'_>, call: &Expression, _: &Environment| {
        *source.borrow_mut() = context.source_origin(call);
        Ok(blob("drawn"))
    };
    let overlay = ForeignOverlay::from_value(&functions, &scoped);
    let stored = lambda([], call(Value::from(foreign), []));
    let foreign_functions = ForeignFunctions::default();

    let evaluation = super::evaluate_runtime_scoped(
        &RuntimeValue::from(call(Value::from(function), [])),
        &definitions_from_parts(
            |cell| (cell == function).then(|| stored.clone()),
            &foreign_functions,
        ),
        &overlay,
        20,
    )
    .into_value();

    assert_eq!(evaluation.result, blob("drawn"));
    assert_eq!(
        *source.borrow(),
        Some(SourceOrigin::Cell {
            cell: function,
            path: vec![gid::Step::Key(vocabulary::BODY)],
        }),
    );
}

#[test]
fn a_value_definitions_origin_is_its_cell() {
    let cell = new_cell_id();
    let foreign = new_cell_id();
    let functions = [foreign];
    let origin = RefCell::new(None);
    let scoped = |_, context: &mut Context<'_>, call: &Expression, _: &Environment| {
        *origin.borrow_mut() = context.source_origin(call);
        Ok(blob("drawn"))
    };
    let overlay = ForeignOverlay::from_value(&functions, &scoped);
    let evaluation = super::evaluate_runtime_scoped(
        &RuntimeValue::from(Value::from(cell)),
        &TestHost(|queried| {
            assert_eq!(queried, cell);
            vec![Definition::Value(call(Value::from(foreign), []))]
        }),
        &overlay,
        20,
    )
    .into_value();
    assert_eq!(evaluation.result, blob("drawn"));
    assert_eq!(
        origin.into_inner(),
        Some(SourceOrigin::Cell { cell, path: vec![] })
    );
}

#[test]
fn a_missing_argument_answers_only_where_the_body_uses_it() {
    let (used, unused) = (new_cell_id(), new_cell_id());
    let given = blob("given");
    let foreign = ForeignFunctions::default();
    let call_with_used =
        |body: CellId| call(lambda([used, unused], body.into()), [(used, given.clone())]);
    assert_eq!(
        evaluate(&call_with_used(used), |_| None, &foreign, 20).result,
        given
    );
    assert_eq!(
        apply(
            &lambda([used, unused], used.into()),
            [(used, given.clone())],
            |_| None,
            &foreign,
            20
        )
        .result,
        given
    );
    // It names the call it's missing from, here the evaluation's input.
    assert_eq!(
        evaluate(&call_with_used(unused), |_| None, &foreign, 20).result,
        absent::missing_argument(unused, Some(&crate::SourceOrigin::Input(vec![])))
    );
}

#[test]
fn apply_classifies_missing_arguments_and_uncallable_targets() {
    let parameter = new_cell_id();
    let missing = apply(
        &lambda([parameter], Value::from(parameter)),
        [],
        |_| None,
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(
        missing.result,
        absent::with_detail(absent::MISSING_ARGUMENT, absent::CELL, parameter.into())
    );

    let uncallable = apply(
        &blob("not a function"),
        [],
        |_| None,
        &ForeignFunctions::default(),
        20,
    );
    assert_eq!(
        uncallable.result,
        absent::with_detail(absent::NOT_CALLABLE, absent::VALUE, blob("not a function"))
    );
}

#[test]
fn a_native_boolean_answers_as_its_record() {
    for value in [true, false] {
        let native = RuntimeValue::bool(value);
        assert_eq!(native.to_value(), logic::value(value));
        assert_eq!(native.record_len(), Some(1));
        assert_eq!(
            native.field(logic::BOOL).and_then(|field| field.as_cell()),
            Some(if value { logic::TRUE } else { logic::FALSE })
        );
        assert_eq!(logic::read(&native.into_value()), Some(value));
    }
}
