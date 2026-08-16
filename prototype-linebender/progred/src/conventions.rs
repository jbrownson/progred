//! Name lookup and the `grap` value projection.

use crate::sources::Sources;
use progred_display::{Env, Layout, arrow, nest, transient};
use progred_graph::{CellId, Step, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    /// A record with this field is a projection request: show the
    /// stored expression, then `→`, then the evaluated result. This is
    /// not a Grap evaluator form.
    pub const GRAP: CellId = CellId::from_u128(0xac807d20d964e141d44c1b2eb98e5ca9);
}

pub fn name<'a>(sources: &'a Sources, cell: CellId) -> Option<&'a str> {
    sources.value(cell).and_then(progred_name::read)
}

/// Raw shows the uninterpreted value and therefore uses the short id.
pub fn display_name<'a>(sources: &'a Sources, raw: bool, cell: CellId) -> Option<&'a str> {
    (!raw).then(|| name(sources, cell)).flatten()
}

/// A record with a `grap` field. Other fields do not block recognition.
pub fn display(env: &dyn Env, value: &Value) -> Option<Layout> {
    let expression = value.as_record()?.get(&vocabulary::GRAP)?;
    (!env.transient()).then(|| {
        arrow(
            nest(Step::Key(vocabulary::GRAP), expression),
            transient(&env.evaluate(expression)),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEnv {
        computed: bool,
    }

    impl Env for TestEnv {
        fn evaluate(&self, expression: &Value) -> Value {
            grap::evaluate(
                expression,
                |_| None,
                &crate::stack::foreign_functions(),
                grap::DEFAULT_FUEL,
            )
            .result
        }

        fn transient(&self) -> bool {
            self.computed
        }
    }

    fn add_expression() -> Value {
        grap::call(
            Value::from(grap_f64::vocabulary::ADD),
            [
                (grap_f64::vocabulary::LEFT, grap_f64::value(2.0)),
                (grap_f64::vocabulary::RIGHT, grap_f64::value(3.0)),
            ],
        )
    }

    #[test]
    fn grap_projects_a_record_with_that_field() {
        let expression = add_expression();
        let wrapper = Value::record([(vocabulary::GRAP, expression.clone())]);
        let Layout::Arrow {
            expression: shown,
            result,
        } = display(&TestEnv { computed: false }, &wrapper).unwrap()
        else {
            panic!("expected an arrow");
        };
        assert!(matches!(
            shown.as_ref(),
            Layout::Nest { step, value }
                if *step == Step::Key(vocabulary::GRAP) && *value == expression
        ));
        assert!(matches!(
            result.as_ref(),
            Layout::Transient(value) if *value == grap_f64::value(5.0)
        ));
    }

    #[test]
    fn grap_still_matches_when_other_fields_are_present() {
        let extra = crate::test_values::label("note");
        let expression = add_expression();
        let wrapper = Value::record([
            (vocabulary::GRAP, expression.clone()),
            (extra, crate::test_values::text("kept")),
        ]);
        let picture = display(&TestEnv { computed: false }, &wrapper);
        assert!(matches!(picture, Some(Layout::Arrow { .. })));
    }

    #[test]
    fn grap_fails_closed_on_a_transient_result() {
        let wrapper = Value::record([(vocabulary::GRAP, add_expression())]);
        assert!(display(&TestEnv { computed: true }, &wrapper).is_none());
    }
}
