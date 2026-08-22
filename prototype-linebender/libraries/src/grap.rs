//! A live projection for a record with a `grap` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::{Library, absent, name};
use gid::{CellId, Cells, Step, Value};
use grap_runtime::vocabulary::{BODY, FFI, FUNCTION, GRAP, PARAMS};
use grap_runtime::{Context, Environment, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{
    Face, Layout, ProjectionInput, RecordField, activatable, alternatives, at_with_projection, col,
    dim, faced, hug, record, row, shared, transient,
};

fn short_id(cell: CellId) -> String {
    let hex = cell.simple().to_string();
    format!("…{}", &hex[hex.len() - 5..])
}

fn spelling(env: &dyn progred_display::Env, cell: CellId) -> (String, Face) {
    match env.name(cell) {
        Some(name) => (name, Face::Name),
        None => (short_id(cell), Face::Id),
    }
}

/// A cell as a reference, not as an invitation to inspect its value.
/// Contextual projections use this for binders and callable names.
fn shallow_cell<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let cell = input.value.as_cell()?;
    let (spelling, face) = spelling(input.env, cell);
    Some(activatable(
        faced(spelling, face),
        input.hover,
        input.select,
    ))
}

pub(crate) fn shallow_at<World, Hover: Clone>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
) -> Layout<World, Hover> {
    at_with_projection(
        steps,
        value,
        [shallow_cell::<World, Hover> as progred_display::Partial<World, Hover>],
    )
}

pub(crate) fn at<World, Hover: Clone>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
) -> Layout<World, Hover> {
    at_with_projection(
        steps,
        value,
        [shallow_cell::<World, Hover> as progred_display::Partial<World, Hover>],
    )
}

fn field_spelling(env: &dyn progred_display::Env, field: CellId) -> (String, Face) {
    match env.name(field) {
        Some(name) => (name, Face::Label),
        None => (short_id(field), Face::Id),
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
fn function_parameters(
    env: &dyn progred_display::Env,
    function: &Value,
) -> Option<Vec<CellId>> {
    let mut function = function;
    let mut followed = std::collections::BTreeSet::new();
    while let Some(cell) = function.as_cell() {
        if !followed.insert(cell) {
            return None;
        }
        function = env.cell_value(cell)?;
    }
    parameters(function)
}

fn standard_field_order(
    env: &dyn progred_display::Env,
    left: &CellId,
    right: &CellId,
) -> std::cmp::Ordering {
    match (env.name(*left), env.name(*right)) {
        (Some(left_name), Some(right_name)) => left_name.cmp(&right_name).then(left.cmp(right)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

/// Calls read as calls. Their function position is a shallow
/// reference when it is a cell; arguments retain Grap's contextual
/// projection.
pub fn call_display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&FUNCTION)?;
    let parameters = function_parameters(input.env, function);
    let mut parameter_positions = std::collections::BTreeMap::new();
    for (position, parameter) in parameters.iter().flatten().enumerate() {
        parameter_positions.entry(*parameter).or_insert(position);
    }
    let function = match function {
        Value::Cell(_) => shallow_at([Step::Key(FUNCTION)], function),
        _ => at([Step::Key(FUNCTION)], function),
    };
    let arguments = record(
        fields
            .iter()
            .filter(|(field, _)| **field != FUNCTION)
            .map(|(field, value)| (*field, value)),
        |left, right| match (parameter_positions.get(left), parameter_positions.get(right)) {
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
                value: at([Step::Key(field)], value),
            }
        },
    );
    Some(hug(function, arguments, 0.0, 20.0))
}

/// A stored lambda exposes its parameter references shallowly and
/// continues Grap's contextual projection through its body.
pub fn lambda_display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let params = fields.get(&PARAMS)?;
    params
        .as_list()?
        .values()
        .all(|param| param.as_cell().is_some())
        .then_some(())?;
    let body = fields.get(&BODY)?;
    let params = at_with_projection(
        [Step::Key(PARAMS)],
        params,
        [shallow_cell::<World, Hover> as progred_display::Partial<World, Hover>],
    );
    let body_target = input.targets.at([Step::Key(BODY)]);
    let lambda = activatable(dim("λ"), input.hover, input.select);
    let arrow = activatable(
        dim("→"),
        body_target.hover,
        body_target.select,
    );
    let head = row(3.0, [lambda, params, arrow]);
    Some(hug(head, at([Step::Key(BODY)], body), 6.0, 20.0))
}

/// Foreignness is an evaluator implementation detail. In source, an
/// FFI callable projects exactly like the cell it names.
pub fn ffi_display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let ffi = input.value.as_record()?.get(&FFI)?;
    ffi.as_cell()?;
    Some(shallow_at([Step::Key(FFI)], ffi))
}

pub fn display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let expression = input.value.as_record()?.get(&GRAP)?;
    let (result, fuel) = input.env.evaluate(expression);
    let expression = shared(at([Step::Key(GRAP)], expression));
    let shaft = shared(activatable(dim("→"), input.hover, input.select));
    let result = shared(transient(&result, fuel));
    Some(alternatives([
        row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
        col(0, 2.0, [expression, row(6.0, [shaft, result])]),
    ]))
}

fn evaluate_foreign(
    context: &mut Context,
    call: &Value,
    calling_environment: &Environment,
) -> Result<Value, Halt> {
    let Some(expression) = context.field(call, grap_runtime::vocabulary::EXPRESSION) else {
        return Ok(context.missing_argument(grap_runtime::vocabulary::EXPRESSION));
    };
    let Some(environment) = context.field(call, grap_runtime::vocabulary::ENVIRONMENT) else {
        return Ok(context.missing_argument(grap_runtime::vocabulary::ENVIRONMENT));
    };
    let environment = context.eval(environment, calling_environment)?;
    match Environment::try_from(environment) {
        Ok(environment) => context.eval(expression, &environment),
        Err(()) => Ok(Value::from(grap_runtime::absent::INVALID_ENVIRONMENT)),
    }
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        grap_runtime::vocabulary::EVALUATE,
        ForeignFunction::new(evaluate_foreign),
    )
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
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
        (grap_runtime::vocabulary::GRAP, "grap"),
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
        cells.set_value(cell, absent::named(value));
    }
    Library {
        cells,
        functions: functions(),
        // Projection order mirrors evaluator precedence: the explicit
        // Grap-result wrapper, calls, lambdas, then FFI values.
        projections: vec![
            display::<World, Hover>,
            call_display::<World, Hover>,
            lambda_display::<World, Hover>,
            ffi_display::<World, Hover>,
        ],
    }
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
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.result.clone(), 7)
        }
    }

    fn wrapper(expression: Value, extra: impl IntoIterator<Item = (gid::CellId, Value)>) -> Value {
        Value::record(std::iter::once((GRAP, expression)).chain(extra))
    }

    fn env() -> TestEnv {
        TestEnv {
            result: Value::from(vec![1]),
        }
    }

    fn input<'a>(env: &'a dyn Env, value: &'a Value) -> ProjectionInput<'a, (), ()> {
        let select = std::rc::Rc::new(|_: &mut ()| false);
        ProjectionInput {
            env,
            value,
            selection: None,
            state: None,
            select: select.clone(),
            hover: (),
            targets: progred_display::ProjectionTargets::fixed(select, ()),
        }
    }

    fn relative_input<'a>(
        env: &'a dyn Env,
        value: &'a Value,
    ) -> ProjectionInput<'a, (), Vec<Step>> {
        let select = std::rc::Rc::new(|_: &mut ()| false);
        let target_select = select.clone();
        ProjectionInput {
            env,
            value,
            selection: None,
            state: None,
            select,
            hover: Vec::new(),
            targets: progred_display::ProjectionTargets::new(move |steps| {
                progred_display::ProjectionTarget {
                    select: target_select.clone(),
                    hover: steps,
                }
            }),
        }
    }

    fn projected(env: &dyn Env, value: &Value) -> Option<Layout<(), ()>> {
        display(input(env, value))
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
                if *steps == [Step::Key(GRAP)] && *value == expression
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
    fn a_grap_shaped_result_is_another_projection() {
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
                if *steps == [Step::Key(GRAP)] && *value == inner
        ));
    }

    #[test]
    fn a_call_projects_its_function_cell_shallowly() {
        let function = new_cell_id();
        let argument = new_cell_id();
        let layout = call_display(input(
            &env(),
            &grap_runtime::call(
                Value::from(function),
                [(argument, Value::from(vec![1]))],
            ),
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
        let call = grap_runtime::call(
            Value::from(function),
            [(argument, Value::from(vec![1]))],
        );
        let layout = call_display(relative_input(&env(), &call)).unwrap();
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
            fn evaluate(&self, _: &Value) -> (Value, usize) {
                (Value::record([]), 0)
            }

            fn cell_value(&self, cell: CellId) -> Option<&Value> {
                (cell == FUNCTION_CELL).then_some(&self.definition)
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
        let layout = call_display(input(&env, &call))
        .unwrap();

        assert_eq!(
            argument_order(&layout),
            [
                FIRST_PARAMETER,
                SECOND_PARAMETER,
                FIRST_EXTRA,
                SECOND_EXTRA,
            ]
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
        let layout = call_display(input(&env(), &call)).unwrap();

        assert_eq!(
            argument_order(&layout),
            [FIRST_PARAMETER, SECOND_PARAMETER]
        );
    }

    #[test]
    fn a_lambda_targets_its_syntax_and_projects_its_body_as_grap() {
        let parameter = new_cell_id();
        let definition = grap_runtime::lambda([parameter], Value::from(parameter));
        let layout = lambda_display(relative_input(&env(), &definition)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("lambda has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Layout::Row {
            children: head, ..
        } = unshared(&children[0])
        else {
            panic!("lambda has a syntax head");
        };
        let Layout::OnHover { child, hover } = &head[0] else {
            panic!("lambda marker targets the whole function");
        };
        assert_eq!(hover.as_deref(), Some(&[][..]));
        assert!(matches!(child.as_ref(), Layout::OnActivate { .. }));
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
    fn the_library_describes_grap_and_owns_evaluate() {
        let library = library::<(), ()>();
        assert_eq!(
            library
                .cells
                .value(grap_runtime::vocabulary::FUNCTION)
                .and_then(name::read),
            Some("function")
        );
        assert!(absent::is_absent(
            library
                .cells
                .value(grap_runtime::absent::MISSING_CELL)
                .unwrap()
        ));
        assert_eq!(library.projections.len(), 4);

        let input = gid::new_cell_id();
        let expression = grap_runtime::call(
            grap_runtime::lambda([input], Value::from(input)),
            [(input, Value::from(b"evaluated".to_vec()))],
        );
        let evaluation = grap_runtime::evaluate(
            &grap_runtime::call(
                Value::from(grap_runtime::vocabulary::EVALUATE),
                [
                    (grap_runtime::vocabulary::EXPRESSION, expression),
                    (grap_runtime::vocabulary::ENVIRONMENT, Value::record([])),
                ],
            ),
            |_| None,
            &library.functions,
            40,
        );
        assert_eq!(evaluation.result, Value::from(b"evaluated".to_vec()));
        assert!(evaluation.diagnostics.is_empty());
    }
}
