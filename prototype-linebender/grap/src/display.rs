//! A live projection for a record with a `grap` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::vocabulary::GRAP;
use gid::Step;
#[cfg(test)]
use gid::Value;
use progred_display::{
    Layout, ProjectionInput, col, dim, group, nest, on_click, on_hover, row, transient,
};

pub fn display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let expression = input.value.as_record()?.get(&GRAP)?;
    let (result, fuel) = input.env.evaluate(expression);
    let expression = nest(Step::Key(GRAP), expression);
    let shaft = on_hover(on_click(dim("→"), input.select), input.hover);
    let result = transient(&result, fuel);
    Some(group(
        row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
        col(0, 2.0, [expression, row(6.0, [shaft, result])]),
    ))
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
            select: std::rc::Rc::new(|_, _| false),
            hover: (),
        })
    }

    fn arms(layout: &Layout<(), ()>) -> (&Layout<(), ()>, &Layout<(), ()>) {
        let Layout::Group { flat, .. } = layout else {
            panic!("expected a group");
        };
        let Layout::Row { children, .. } = flat.as_ref() else {
            panic!("expected a row");
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
            Some(Layout::Group { .. })
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
}
