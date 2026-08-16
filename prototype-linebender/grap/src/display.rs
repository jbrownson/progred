//! A live projection for a record with a `grap` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::vocabulary::GRAP;
use progred_display::{Click, Env, Layout, col, dim, group, nest, on_click, row, transient};
use progred_graph::{Step, Value};

pub fn display(env: &dyn Env, value: &Value) -> Option<Layout> {
    let expression = value.as_record()?.get(&GRAP)?;
    let (result, fuel) = env.evaluate(expression);
    let expression = nest(Step::Key(GRAP), expression);
    let shaft = on_click(dim("→"), Click::Select);
    let result = transient(&result, fuel);
    Some(group(
        row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
        col(0, 2.0, [expression, row(6.0, [shaft, result])]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    struct TestEnv {
        result: Value,
    }

    impl Env for TestEnv {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.result.clone(), 7)
        }
    }

    fn wrapper(expression: Value, extra: impl IntoIterator<Item = (progred_graph::CellId, Value)>) -> Value {
        Value::record(std::iter::once((GRAP, expression)).chain(extra))
    }

    fn env() -> TestEnv {
        TestEnv {
            result: Value::from(vec![1]),
        }
    }

    fn arms(layout: &Layout) -> (&Layout, &Layout) {
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
        let layout = display(&env(), &wrapper(expression.clone(), [])).unwrap();
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
            display(
                &env(),
                &wrapper(Value::from(vec![0]), [(new_cell_id(), Value::from(vec![2]))]),
            ),
            Some(Layout::Group { .. })
        ));
    }

    #[test]
    fn a_grap_shaped_result_is_another_projection() {
        let inner = Value::from(vec![2]);
        let result = wrapper(inner.clone(), []);
        let layout = display(
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
        let layout = display(&env(), &result).unwrap();
        let (nested, _) = arms(&layout);
        assert!(matches!(
            nested,
            Layout::At { steps, value }
                if *steps == [Step::Key(GRAP)] && *value == inner
        ));
    }
}
