//! A live projection for a record with an `evaluate` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::{Library, absent, name};
use gid::{CellId, Cells, Step, Value};

pub const ID: CellId = CellId::from_u128(0xf7735b90f6826b25c350a8fd83af8c47);
use grap_runtime::vocabulary::{BODY, EVALUATE, FFI, FUNCTION, PARAMS};
use grap_runtime::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{
    Completion, CompletionKind, CompletionProvider, Delim, Face, Layout, Pending, ProjectionInput,
    RecordField, activatable, alternatives, at_with_projection, bracket, col, completion, descend,
    dim, faced, hug, record_with, row, shared, slot, transient,
};

pub mod vocabulary {
    use gid::CellId;

    /// Source whose contents belong to the Grap domain. This is an
    /// ordinary field and requests no evaluation.
    pub const GRAP: CellId = CellId::from_u128(0x315ca8459cfc64a210d518da1cad79b9);
}

fn short_id(cell: CellId) -> String {
    let hex = cell.simple().to_string();
    format!("…{}", &hex[hex.len() - 5..])
}

fn spelling(env: &dyn progred_display::Env, cell: CellId) -> (String, Face) {
    match env.names(cell) {
        names if !names.is_empty() => (names.join(" / "), Face::Name),
        _ => (short_id(cell), Face::Id),
    }
}

/// A cell as a reference, not as an invitation to inspect its value.
/// Contextual projections use this for expression and callable references.
fn shallow_cell<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let cell = input.value.as_cell()?;
    let (spelling, face) = spelling(input.env, cell);
    let target = input.targets.current();
    Some(activatable(
        faced(spelling, face),
        target.hover,
        target.select,
    ))
}

/// A cell as its definition. This is the structural cell form repeated
/// as a partial so a declaration can override an enclosing shallow
/// expression context.
fn deep_cell<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let cell = input.value.as_cell()?;
    let definitions = input.env.cell_definitions(cell);
    let [progred_display::CellDefinition::Value(resolution, _)] = definitions.as_slice() else {
        return None;
    };
    Some(bracket(
        Delim::Paren,
        descend(Step::Follow(*resolution), None, None),
    ))
}

fn lambda_name<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(crate::line_edit::layout(
        crate::text::read(input.value)?,
        grap_runtime::ffi(crate::text::vocabulary::UPDATE),
        "\"",
        "\"",
    ))
}

pub fn shallow_at<World: 'static, Hover: Clone + 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
) -> Layout<World, Hover> {
    at_with_projection(
        steps,
        value,
        [progred_display::partial(shallow_cell::<World, Hover>)],
    )
}

/// Project an expression subtree at a real location. Cells in that
/// subtree are references until a nested construct explicitly enters
/// a declaration or data subtree.
pub(crate) fn expression_at<World: 'static, Hover: Clone + 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
) -> Layout<World, Hover> {
    shallow_at(steps, value)
}

pub(crate) fn deep_at<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
) -> Layout<World, Hover> {
    at_with_projection(
        steps,
        value,
        [progred_display::partial(deep_cell::<World, Hover>)],
    )
}

pub(crate) fn shallow_descend<World: 'static, Hover: Clone + 'static>(
    step: Step,
) -> Layout<World, Hover> {
    descend(
        step,
        Some(vec![progred_display::partial(shallow_cell::<World, Hover>)]),
        None,
    )
}

pub(crate) fn expression_descend<World: 'static, Hover: Clone + 'static>(
    step: Step,
) -> Layout<World, Hover> {
    shallow_descend(step)
}

fn field_spelling(env: &dyn progred_display::Env, field: CellId) -> (String, Face) {
    match env.names(field) {
        names if !names.is_empty() => (names.join(" / "), Face::Label),
        _ => (short_id(field), Face::Id),
    }
}

fn parameters(value: &Value) -> Option<Vec<CellId>> {
    let fields = value.as_record()?;
    let fields = match fields.get(&grap_runtime::vocabulary::CLOSURE) {
        Some(closure) => closure.as_record()?,
        None => fields,
    };
    fields.get(&BODY)?;
    fields
        .get(&PARAMS)?
        .as_list()?
        .values()
        .map(Value::as_cell)
        .collect()
}

/// Parameter order is source metadata, not an evaluation. Follow
/// transparent cell references to a stored lambda or closure; a
/// computed callable has no order available to the projection.
fn function_parameters(env: &dyn progred_display::Env, function: &Value) -> Option<Vec<CellId>> {
    let mut function = function;
    let mut followed = std::collections::BTreeSet::new();
    while let Some(cell) = function.as_cell() {
        if !followed.insert(cell) {
            return None;
        }
        let definitions = env.cell_definitions(cell);
        let [progred_display::CellDefinition::Value(_, definition)] = definitions.as_slice() else {
            return None;
        };
        function = definition;
    }
    parameters(function)
}

fn standard_field_order(
    env: &dyn progred_display::Env,
    left: &CellId,
    right: &CellId,
) -> std::cmp::Ordering {
    match (env.names(*left), env.names(*right)) {
        (left_names, right_names) if !left_names.is_empty() && !right_names.is_empty() => {
            left_names.cmp(&right_names).then(left.cmp(right))
        }
        (left_names, _) if !left_names.is_empty() => std::cmp::Ordering::Less,
        (_, right_names) if !right_names.is_empty() => std::cmp::Ordering::Greater,
        _ => left.cmp(right),
    }
}

/// Calls read as calls. Their function position is a shallow
/// reference when it is a cell; arguments retain Grap's contextual
/// projection.
pub fn call_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&FUNCTION)?;
    let parameters = function_parameters(input.env, function);
    let mut parameter_positions = std::collections::BTreeMap::new();
    for (position, parameter) in parameters.iter().flatten().enumerate() {
        parameter_positions.entry(*parameter).or_insert(position);
    }
    let trailing = match &input.pending {
        Some(Pending::Field) => {
            let field_completions: Option<CompletionProvider> =
                parameters.as_ref().map(|parameters| {
                    let completions = parameters
                        .iter()
                        .filter(|parameter| !fields.contains_key(parameter))
                        .map(|parameter| {
                            let (display, _) = field_spelling(input.env, *parameter);
                            Completion::new(display, Value::from(*parameter))
                                .with_detail("parameter")
                        })
                        .collect::<Vec<_>>();
                    std::rc::Rc::new(move |_: &str| completions.clone()) as CompletionProvider
                });
            vec![RecordField {
                label: completion(CompletionKind::Field, field_completions),
                value: slot(),
            }]
        }
        Some(Pending::Child(Step::Key(field))) if !fields.contains_key(field) => {
            let (spelling, face) = field_spelling(input.env, *field);
            let target = input.targets.at([Step::Key(*field)]);
            vec![RecordField {
                label: activatable(faced(spelling, face), target.hover, target.select),
                value: descend(
                    Step::Key(*field),
                    Some(vec![progred_display::partial(shallow_cell::<World, Hover>)]),
                    Some(completion(CompletionKind::Value, None)),
                ),
            }]
        }
        _ => Vec::new(),
    };
    let function = expression_at([Step::Key(FUNCTION)], function);
    let arguments = record_with(
        fields
            .iter()
            .filter(|(field, _)| *field != FUNCTION)
            .map(|(field, value)| (*field, value)),
        |left, right| match (
            parameter_positions.get(left),
            parameter_positions.get(right),
        ) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => standard_field_order(input.env, left, right),
        },
        |field, value| {
            let (spelling, face) = field_spelling(input.env, field);
            let target = input.targets.at([Step::Key(field)]);
            RecordField {
                label: activatable(faced(spelling, face), target.hover, target.select),
                value: expression_at([Step::Key(field)], value),
            }
        },
        trailing,
    );
    Some(hug(function, arguments, 0.0, 20.0))
}

/// A stored lambda exposes its parameter declarations deeply and
/// projects its body as an expression.
pub fn lambda_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let params = fields.get(&PARAMS)?;
    params
        .as_list()?
        .values()
        .all(|param| param.as_cell().is_some())
        .then_some(())?;
    let body = fields.get(&BODY)?;
    let params = deep_at([Step::Key(PARAMS)], params);
    let body_target = input.targets.at([Step::Key(BODY)]);
    let lambda = descend(
        Step::Key(name::vocabulary::NAME),
        Some(vec![progred_display::partial(lambda_name::<World, Hover>)]),
        Some(crate::line_edit::layout_with_placeholder(
            "",
            Some("λ"),
            grap_runtime::ffi(crate::text::vocabulary::UPDATE),
            "",
            "",
        )),
    );
    let arrow = activatable(dim("→"), body_target.hover, body_target.select);
    let head = row(3.0, [lambda, params, arrow]);
    Some(hug(head, expression_at([Step::Key(BODY)], body), 6.0, 20.0))
}

/// Foreignness is an evaluator implementation detail. In source, an
/// FFI callable projects exactly like the cell it names.
pub fn ffi_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let ffi = input.value.as_record()?.get(&FFI)?;
    ffi.as_cell()?;
    Some(shallow_at([Step::Key(FFI)], ffi))
}

pub fn evaluate_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let expression = input.value.as_record()?.get(&EVALUATE)?;
    let (result, fuel) = input.env.evaluate(expression);
    let expression = shared(expression_at([Step::Key(EVALUATE)], expression));
    let shaft_target = input.targets.current();
    let shaft = shared(activatable(
        dim("→"),
        shaft_target.hover,
        shaft_target.select,
    ));
    let result = shared(transient(&result, fuel));
    Some(alternatives([
        row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
        col(0, 2.0, [expression, row(6.0, [shaft, result])]),
    ]))
}

fn evaluate_foreign(
    context: &mut Context,
    call: Expression,
    calling_environment: &Environment,
) -> Result<grap_runtime::RuntimeValue, Halt> {
    let Some(expression) = context.field(call, grap_runtime::vocabulary::EXPRESSION) else {
        return Ok(context.missing_runtime_argument(grap_runtime::vocabulary::EXPRESSION));
    };
    let Some(environment) = context.field(call, grap_runtime::vocabulary::ENVIRONMENT) else {
        return Ok(context.missing_runtime_argument(grap_runtime::vocabulary::ENVIRONMENT));
    };
    let environment = context.eval(environment, calling_environment)?;
    match context.environment(&environment) {
        Some(environment) => context.eval_runtime(expression, &environment),
        None => Ok(grap_runtime::absent::with_detail(
            grap_runtime::absent::INVALID_ENVIRONMENT,
            grap_runtime::absent::VALUE,
            environment,
        )
        .into()),
    }
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        grap_runtime::vocabulary::EVALUATE,
        ForeignFunction::runtime(evaluate_foreign),
    )
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, value) in [
        (grap_runtime::vocabulary::FUNCTION, "function"),
        (grap_runtime::vocabulary::PARAMS, "params"),
        (grap_runtime::vocabulary::BODY, "body"),
        (grap_runtime::vocabulary::CLOSURE, "closure"),
        (grap_runtime::vocabulary::ENVIRONMENT, "environment"),
        (grap_runtime::vocabulary::FFI, "ffi"),
        (grap_runtime::vocabulary::EVALUATE, "evaluate"),
        (grap_runtime::vocabulary::EXPRESSION, "expression"),
        (vocabulary::GRAP, "grap"),
    ] {
        cells.set_value(cell, name::record(value, []));
    }
    for (cell, value) in [
        (grap_runtime::absent::FUEL_EXHAUSTED, "fuel exhausted"),
        (grap_runtime::absent::MISSING_CELL, "missing cell"),
        (grap_runtime::absent::CELL_CYCLE, "cell cycle"),
        (grap_runtime::absent::MALFORMED_LAMBDA, "malformed lambda"),
        (grap_runtime::absent::INVALID_PARAMETER, "invalid parameter"),
        (grap_runtime::absent::NOT_CALLABLE, "not callable"),
        (grap_runtime::absent::MISSING_ARGUMENT, "missing argument"),
        (
            grap_runtime::absent::INVALID_ENVIRONMENT,
            "invalid environment",
        ),
    ] {
        cells.set_value(cell, absent::named_reason(value));
    }
    Library::named(
        "grap",
        crate::Definitions::from_parts(cells, functions()),
        // Projection order mirrors evaluator precedence: the explicit
        // Grap-result wrapper, calls, lambdas, then FFI values.
        vec![
            progred_display::partial(evaluate_display::<World, Hover>),
            progred_display::partial(call_display::<World, Hover>),
            progred_display::partial(lambda_display::<World, Hover>),
            progred_display::partial(ffi_display::<World, Hover>),
        ],
    )
    .with_root_completions([progred_display::Completion::new(
        "grap",
        Value::record([(vocabulary::GRAP, Value::list([]))]),
    )
    .with_detail("grap library")])
    .with_root_field_completions([progred_display::Completion::new(
        "grap",
        Value::from(vocabulary::GRAP),
    )
    .with_detail("grap library")])
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use progred_display::Env;

    struct TestEnv {
        result: Value,
    }

    impl Env for TestEnv {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.result.clone(), 7)
        }
    }

    fn wrapper(expression: Value, extra: impl IntoIterator<Item = (gid::CellId, Value)>) -> Value {
        Value::record(std::iter::once((EVALUATE, expression)).chain(extra))
    }

    fn env() -> TestEnv {
        TestEnv {
            result: Value::from(vec![1]),
        }
    }

    fn unit_target(_: Vec<Step>) -> progred_display::ProjectionTarget<(), ()> {
        progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: (),
        }
    }

    fn relative_target(steps: Vec<Step>) -> progred_display::ProjectionTarget<(), Vec<Step>> {
        progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: steps,
        }
    }

    fn input<'a>(env: &'a dyn Env, value: &'a Value) -> ProjectionInput<'a, (), ()> {
        ProjectionInput {
            env,
            value,
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&unit_target),
        }
    }

    fn relative_input<'a>(
        env: &'a dyn Env,
        value: &'a Value,
    ) -> ProjectionInput<'a, (), Vec<Step>> {
        ProjectionInput {
            env,
            value,
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&relative_target),
        }
    }

    fn projected(env: &dyn Env, value: &Value) -> Option<Layout<(), ()>> {
        evaluate_display(&input(env, value))
    }

    #[test]
    fn parameter_offers_are_prepared_only_during_field_insertion() {
        struct CountingEnv(std::cell::Cell<usize>);
        impl Env for CountingEnv {
            fn apply(
                &self,
                _: &gid::Value,
                _: &[(gid::CellId, gid::Value)],
            ) -> (gid::Value, usize) {
                panic!("unexpected projection application")
            }

            fn evaluate(&self, _: &Value) -> (Value, usize) {
                panic!("completion does not evaluate the function")
            }

            fn names(&self, _: CellId) -> Vec<&str> {
                self.0.set(self.0.get() + 1);
                vec!["parameter"]
            }
        }

        let parameter = new_cell_id();
        let value = Value::record([(
            FUNCTION,
            Value::record([
                (PARAMS, Value::list([Value::from(parameter)])),
                (BODY, Value::record([])),
            ]),
        )]);
        let env = CountingEnv(std::cell::Cell::new(0));
        assert!(call_display(&input(&env, &value)).is_some());
        assert_eq!(env.0.get(), 0);
        assert!(
            call_display(&ProjectionInput {
                pending: Some(Pending::Field),
                ..input(&env, &value)
            })
            .is_some()
        );
        assert_eq!(env.0.get(), 1);
    }

    fn unshared<World, Hover>(mut layout: &Layout<World, Hover>) -> &Layout<World, Hover> {
        while let Layout::Shared { child, .. } = layout {
            layout = child.as_ref();
        }
        layout
    }

    fn arms(layout: &Layout<(), ()>) -> (&Layout<(), ()>, &Layout<(), ()>) {
        let Layout::Alternatives(options) = layout else {
            panic!("expected alternatives");
        };
        let Some(Layout::Row { children, .. }) = options.first() else {
            panic!("expected a row first");
        };
        assert_eq!(children.len(), 3);
        (unshared(&children[0]), unshared(&children[2]))
    }

    fn argument_order(layout: &Layout<(), ()>) -> Vec<CellId> {
        let Layout::Alternatives(call_options) = layout else {
            panic!("call has responsive forms");
        };
        let Layout::Row { children, .. } = &call_options[0] else {
            panic!("flat call first");
        };
        let Layout::Surround { child, .. } = unshared(&children[1]) else {
            panic!("arguments are record-delimited");
        };
        let Layout::Alternatives(argument_options) = child.as_ref() else {
            panic!("arguments have responsive forms");
        };
        let Layout::Row { children, .. } = &argument_options[0] else {
            panic!("flat arguments first");
        };
        children
            .iter()
            .step_by(2)
            .map(|argument| {
                let Layout::Row { children, .. } = argument else {
                    panic!("argument has a label and value");
                };
                let Layout::At { steps, .. } = unshared(&children[2]) else {
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
        assert!(matches!(
            shown,
            Layout::At { steps, value, .. }
                if *steps == [Step::Key(EVALUATE)] && *value == expression
        ));
        assert!(matches!(
            result,
            Layout::Transient { value, fuel: 7 } if *value == Value::from(vec![1])
        ));
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
            Some(Layout::Alternatives(_))
        ));
    }

    #[test]
    fn an_evaluate_shaped_result_is_another_projection() {
        let inner = Value::from(vec![2]);
        let result = wrapper(inner.clone(), []);
        let layout = projected(
            &TestEnv {
                result: result.clone(),
            },
            &wrapper(Value::from(vec![0]), []),
        )
        .unwrap();
        let (_, shown) = arms(&layout);
        assert!(matches!(
            shown,
            Layout::Transient { value, fuel: 7 } if *value == result
        ));
        let layout = projected(&env(), &result).unwrap();
        let (nested, _) = arms(&layout);
        assert!(matches!(
            nested,
            Layout::At { steps, value, .. }
                if *steps == [Step::Key(EVALUATE)] && *value == inner
        ));
    }

    #[test]
    fn a_call_projects_its_function_cell_shallowly() {
        let function = new_cell_id();
        let argument = new_cell_id();
        let layout = call_display(&input(
            &env(),
            &grap_runtime::call(Value::from(function), [(argument, Value::from(vec![1]))]),
        ))
        .unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("call has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat call first");
        };
        assert!(matches!(
            unshared(&children[0]),
            Layout::At {
                steps,
                value,
                projection: Some(projection),
            } if *steps == [Step::Key(FUNCTION)]
                && *value == Value::from(function)
                && projection.len() == 1
        ));
    }

    #[test]
    fn a_call_argument_label_targets_its_value() {
        let function = new_cell_id();
        let argument = new_cell_id();
        let call = grap_runtime::call(Value::from(function), [(argument, Value::from(vec![1]))]);
        let layout = call_display(&relative_input(&env(), &call)).unwrap();
        let Layout::Alternatives(call_options) = layout else {
            panic!("call has responsive forms");
        };
        let Layout::Row { children, .. } = &call_options[0] else {
            panic!("flat call first");
        };
        let Layout::Surround { left, child, right } = unshared(&children[1]) else {
            panic!("arguments are record-delimited");
        };
        assert!(matches!(
            left,
            progred_display::Ink::Delim {
                delim: progred_display::Delim::Brace,
                side: progred_display::Side::Open,
            }
        ));
        assert!(matches!(
            right,
            progred_display::Ink::Delim {
                delim: progred_display::Delim::Brace,
                side: progred_display::Side::Close,
            }
        ));
        let Layout::Alternatives(argument_options) = child.as_ref() else {
            panic!("arguments have responsive forms");
        };
        let Layout::Row { children, .. } = &argument_options[0] else {
            panic!("flat arguments first");
        };
        let Layout::Row { children, .. } = &children[0] else {
            panic!("argument has a label and value");
        };
        let Layout::OnHover { child, hover } = unshared(&children[0]) else {
            panic!("the label targets its argument");
        };
        assert_eq!(hover.as_deref(), Some(&[Step::Key(argument)][..]));
        assert!(matches!(child.as_ref(), Layout::OnActivate { .. }));
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
        }

        impl Env for DefinitionEnv {
            fn apply(
                &self,
                _: &gid::Value,
                _: &[(gid::CellId, gid::Value)],
            ) -> (gid::Value, usize) {
                panic!("unexpected projection application")
            }

            fn evaluate(&self, _: &Value) -> (Value, usize) {
                (Value::record([]), 0)
            }

            fn cell_definitions(&self, cell: CellId) -> Vec<progred_display::CellDefinition<'_>> {
                (cell == FUNCTION_CELL)
                    .then_some(progred_display::CellDefinition::Value(
                        gid::Resolution::Document,
                        &self.definition,
                    ))
                    .into_iter()
                    .collect()
            }
        }

        let env = DefinitionEnv {
            definition: grap_runtime::lambda(
                [FIRST_PARAMETER, SECOND_PARAMETER],
                Value::from(FIRST_PARAMETER),
            ),
        };
        let call = grap_runtime::call(
            Value::from(FUNCTION_CELL),
            [
                (SECOND_EXTRA, Value::from(vec![5])),
                (SECOND_PARAMETER, Value::from(vec![2])),
                (FIRST_EXTRA, Value::from(vec![4])),
                (FIRST_PARAMETER, Value::from(vec![3])),
            ],
        );
        let layout = call_display(&input(&env, &call)).unwrap();

        assert_eq!(
            argument_order(&layout),
            [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA, SECOND_EXTRA,]
        );
    }

    #[test]
    fn a_call_uses_an_inline_lambdas_parameter_order() {
        const FIRST_PARAMETER: CellId = CellId::from_u128(2);
        const SECOND_PARAMETER: CellId = CellId::from_u128(1);
        let call = grap_runtime::call(
            grap_runtime::lambda(
                [FIRST_PARAMETER, SECOND_PARAMETER],
                Value::from(FIRST_PARAMETER),
            ),
            [
                (SECOND_PARAMETER, Value::from(vec![1])),
                (FIRST_PARAMETER, Value::from(vec![2])),
            ],
        );
        let layout = call_display(&input(&env(), &call)).unwrap();

        assert_eq!(argument_order(&layout), [FIRST_PARAMETER, SECOND_PARAMETER]);
    }

    #[test]
    fn a_lambda_targets_its_syntax_and_projects_its_body_as_grap() {
        let parameter = new_cell_id();
        let definition = grap_runtime::lambda([parameter], Value::from(parameter));
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("lambda has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Layout::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        assert!(matches!(
            &head[0],
            Layout::Descend {
                step: Step::Key(key),
                projection: Some(projection),
                missing: Some(_),
                ..
            } if *key == name::vocabulary::NAME && projection.len() == 1
        ));
        assert!(matches!(
            &head[1],
            Layout::At {
                steps,
                projection: Some(projection),
                ..
            } if *steps == [Step::Key(PARAMS)] && projection.len() == 1
        ));
        let Layout::OnHover { child, hover } = &head[2] else {
            panic!("lambda arrow targets its body");
        };
        assert_eq!(hover.as_deref(), Some(&[Step::Key(BODY)][..]));
        assert!(matches!(child.as_ref(), Layout::OnActivate { .. }));
        assert!(matches!(
            unshared(&children[1]),
            Layout::At {
                steps,
                value,
                projection: Some(projection),
            } if *steps == [Step::Key(BODY)]
                && *value == Value::from(parameter)
                && projection.len() == 1
        ));
    }

    #[test]
    fn a_named_lambda_projects_its_editable_name_in_place_of_the_marker() {
        let definition = name::record(
            "tree",
            [(PARAMS, Value::list([])), (BODY, Value::from(vec![1]))],
        );
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("lambda has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Layout::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        let Layout::Descend {
            step: Step::Key(key),
            projection: Some(projection),
            missing: Some(_),
            ..
        } = &head[0]
        else {
            panic!("lambda name is projected contextually");
        };
        assert_eq!(*key, name::vocabulary::NAME);
        let value = definition
            .as_record()
            .unwrap()
            .get(&name::vocabulary::NAME)
            .unwrap();
        let Layout::LineEdit(line) = projection[0](&relative_input(&env(), value)).unwrap() else {
            panic!("lambda name uses the stock line editor");
        };
        assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("\"", "\""));
    }

    #[test]
    fn an_anonymous_lambda_projects_an_empty_name_with_a_lambda_placeholder() {
        let definition = Value::record([(PARAMS, Value::list([])), (BODY, Value::from(vec![1]))]);
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("lambda has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Layout::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        let Layout::Descend {
            step: Step::Key(key),
            projection: Some(projection),
            missing: Some(missing),
            ..
        } = &head[0]
        else {
            panic!("lambda name is projected contextually");
        };
        assert_eq!(*key, name::vocabulary::NAME);
        assert_eq!(projection.len(), 1);
        let Layout::LineEdit(line) = missing.as_ref() else {
            panic!("an absent name uses the stock line editor");
        };
        assert_eq!(line.placeholder.as_deref(), Some("λ"));
    }

    #[test]
    fn an_explicit_empty_lambda_name_projects_as_an_empty_string() {
        let definition = name::record(
            "",
            [(PARAMS, Value::list([])), (BODY, Value::from(vec![1]))],
        );
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("lambda has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Layout::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        let Layout::Descend {
            projection: Some(projection),
            ..
        } = &head[0]
        else {
            panic!("lambda name is projected contextually");
        };
        let value = definition
            .as_record()
            .unwrap()
            .get(&name::vocabulary::NAME)
            .unwrap();
        let Layout::LineEdit(line) = projection[0](&relative_input(&env(), value)).unwrap() else {
            panic!("lambda name uses the stock line editor");
        };
        assert_eq!(line.text, "");
        assert_eq!(line.placeholder, None);
        assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("\"", "\""));
    }

    #[test]
    fn the_library_describes_grap_and_owns_evaluate() {
        let library = library::<(), ()>();
        assert_eq!(
            library
                .value(grap_runtime::vocabulary::FUNCTION)
                .and_then(name::read),
            Some("function")
        );
        assert_eq!(
            library
                .value(grap_runtime::absent::MISSING_CELL)
                .and_then(name::read),
            Some("missing cell")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(grap_runtime::absent::MISSING_CELL)),
            Some(grap_runtime::absent::MISSING_CELL)
        );
        assert_eq!(library.projections.len(), 4);

        let input = gid::new_cell_id();
        let expression = grap_runtime::call(
            grap_runtime::lambda([input], Value::from(input)),
            [(input, Value::from(b"evaluated".to_vec()))],
        );
        let evaluation = crate::test_evaluate(
            &grap_runtime::call(
                Value::from(grap_runtime::vocabulary::EVALUATE),
                [
                    (grap_runtime::vocabulary::EXPRESSION, expression),
                    (grap_runtime::vocabulary::ENVIRONMENT, Value::record([])),
                ],
            ),
            |_| None,
            &library.functions(),
            40,
        );
        assert_eq!(evaluation.result, Value::from(b"evaluated".to_vec()));
    }
}
