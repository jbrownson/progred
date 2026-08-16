//! A live projection for a record with a `grap` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::vocabulary::GRAP;
use progred_display::{Env, Layout, arrow, nest, transient};
use progred_graph::{Step, Value};

pub fn display(env: &dyn Env, value: &Value) -> Option<Layout> {
    let expression = value.as_record()?.get(&GRAP)?;
    (!env.transient()).then(|| {
        arrow(
            nest(Step::Key(GRAP), expression),
            transient(&env.evaluate(expression)),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    struct TestEnv {
        computed: bool,
        result: Value,
    }

    impl Env for TestEnv {
        fn evaluate(&self, _: &Value) -> Value {
            self.result.clone()
        }

        fn transient(&self) -> bool {
            self.computed
        }
    }

    fn wrapper(expression: Value, extra: impl IntoIterator<Item = (progred_graph::CellId, Value)>) -> Value {
        Value::record(std::iter::once((GRAP, expression)).chain(extra))
    }

    fn stored() -> TestEnv {
        TestEnv {
            computed: false,
            result: Value::from(vec![1]),
        }
    }

    #[test]
    fn a_record_with_the_field_is_an_arrow() {
        let expression = Value::from(vec![0]);
        let Layout::Arrow {
            expression: shown,
            result,
        } = display(&stored(), &wrapper(expression.clone(), [])).unwrap()
        else {
            panic!("expected an arrow");
        };
        assert!(matches!(
            shown.as_ref(),
            Layout::Nest { step, value }
                if *step == Step::Key(GRAP) && *value == expression
        ));
        assert!(matches!(
            result.as_ref(),
            Layout::Transient(value) if *value == Value::from(vec![1])
        ));
    }

    #[test]
    fn other_fields_do_not_block_recognition() {
        assert!(matches!(
            display(
                &stored(),
                &wrapper(Value::from(vec![0]), [(new_cell_id(), Value::from(vec![2]))]),
            ),
            Some(Layout::Arrow { .. })
        ));
    }

    #[test]
    fn a_transient_result_fails_closed() {
        assert!(
            display(
                &TestEnv {
                    computed: true,
                    result: Value::from(vec![1]),
                },
                &wrapper(Value::from(vec![0]), []),
            )
            .is_none()
        );
    }
}
