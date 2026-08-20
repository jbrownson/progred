//! A live projection for a record with a `grap` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::{Library, absent, name};
use gid::{Cells, Step, Value};
use grap_runtime::vocabulary::GRAP;
use grap_runtime::{Context, Environment, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{
    Layout, ProjectionInput, alternatives, col, dim, nest, on_click, on_hover, row, transient,
};

pub fn display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let expression = input.value.as_record()?.get(&GRAP)?;
    let (result, fuel) = input.env.evaluate(expression);
    let expression = nest(Step::Key(GRAP), expression);
    let shaft = on_hover(on_click(dim("→"), input.select), input.hover);
    let result = transient(&result, fuel);
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
        projections: vec![display::<World, Hover>],
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

    fn projected(env: &dyn Env, value: &Value) -> Option<Layout<(), ()>> {
        display(ProjectionInput {
            env,
            value,
            selection: None,
            state: None,
            select: std::rc::Rc::new(|_| false),
            hover: (),
        })
    }

    fn arms(layout: &Layout<(), ()>) -> (&Layout<(), ()>, &Layout<(), ()>) {
        let Layout::Alternatives(options) = layout else {
            panic!("expected alternatives");
        };
        let Some(Layout::Row { children, .. }) = options.first() else {
            panic!("expected a row first");
        };
        assert_eq!(children.len(), 3);
        (&children[0], &children[2])
    }

    #[test]
    fn a_record_with_the_field_is_expression_then_result() {
        let expression = Value::from(vec![0]);
        let layout = projected(&env(), &wrapper(expression.clone(), [])).unwrap();
        let (shown, result) = arms(&layout);
        assert!(matches!(
            shown,
            Layout::At { steps, value }
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
            Layout::At { steps, value }
                if *steps == [Step::Key(GRAP)] && *value == inner
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
        assert_eq!(library.projections.len(), 1);

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
