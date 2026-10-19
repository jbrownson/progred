use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::projection::{Layout, ProjectionInput};

use crate::display::Env;
use crate::display::test_support::{EvaluatesTo, NoEval, ProjectionCall, inspect};
use crate::libraries::test_widgets::{project_runtime, projection_input};
use ::grap::RuntimeValue;
use gid::{CellId, Value};

const LEFT_VALUE: CellId = CellId::from_u128(1);

fn projected_display(input: &ProjectionInput<'_>) -> Option<Layout> {
    project_runtime(super::projected_display, input)
}

fn display(input: &ProjectionInput<'_>) -> Option<Layout> {
    project_runtime(super::display, input)
}

#[test]
fn viewport_passes_logical_dimensions_and_preserves_its_value_as_data() {
    struct CheckArguments;
    impl Env for CheckArguments {
        fn apply_expression_runtime(
            &self,
            function: &RuntimeValue,
            arguments: &[(CellId, RuntimeValue)],
        ) -> RuntimeValue {
            assert_eq!(function.as_value(), &Value::from(LEFT_VALUE));
            assert_eq!(
                values(arguments),
                [
                    (vocabulary::VALUE, Value::from(LEFT_VALUE)),
                    (layout::vocabulary::WIDTH, f64::value(420.5)),
                    (layout::vocabulary::HEIGHT, f64::value(160.25)),
                ]
            );
            layout::drawing(420.5, 0.0, 160.25, []).into()
        }
        fn evaluate_runtime(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            panic!("viewport sources are passed as data")
        }
        fn evaluate_runtime_memo(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            panic!("viewport sources are passed as data")
        }
        fn apply_runtime_scoped(
            &self,
            _: &RuntimeValue,
            _: &[(CellId, RuntimeValue)],
            _: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("the viewport applies its function as an expression")
        }
        fn apply_runtime_memo(&self, _: &Value, _: &[(CellId, Value)], _: usize) -> RuntimeValue {
            panic!("the declaration has nothing to prepare")
        }
    }
    let declaration = Value::record([
        (vocabulary::VALUE, LEFT_VALUE.into()),
        (vocabulary::VIEWPORT, LEFT_VALUE.into()),
    ]);
    assert!(projected(&declaration, &CheckArguments, display).is_none());
    assert!(matches!(
        (projected(&declaration, &CheckArguments, |input| viewport_display(
            input, 420.5, 160.25
        )))
        .map(|layout| inspect(&layout)),
        Some(ProjectionCall::At { steps, .. }) if steps == [Step::Key(vocabulary::RESULT)]
    ));
    assert!(matches!(
        projected(&declaration, &CheckArguments, |input| viewport_display(
            input, 0.0, 160.25
        ))
        .map(|layout| layout.record()),
        Some(Recorded::Row { .. })
    ));
}

#[test]
fn viewport_preparation_failure_does_not_call_the_viewport() {
    struct Failed(std::cell::Cell<usize>);
    impl Env for Failed {
        fn apply_runtime_memo(
            &self,
            _: &Value,
            _: &[(CellId, Value)],
            fuel: usize,
        ) -> RuntimeValue {
            assert_eq!(fuel, ::grap::DEFAULT_FUEL);
            self.0.set(self.0.get() + 1);
            absent::with_reason(::grap::absent::MISSING_CELL).into()
        }
        fn apply_expression_runtime(
            &self,
            _: &RuntimeValue,
            _: &[(CellId, RuntimeValue)],
        ) -> RuntimeValue {
            panic!("the viewport must not run after preparation failed")
        }
        fn apply_runtime_scoped(
            &self,
            _: &RuntimeValue,
            _: &[(CellId, RuntimeValue)],
            _: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            unreachable!()
        }
        fn evaluate_runtime(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            unreachable!()
        }
        fn evaluate_runtime_memo(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            unreachable!()
        }
    }
    let mut fields = Value::record([
        (vocabulary::VALUE, LEFT_VALUE.into()),
        (vocabulary::PREPARE, vocabulary::PREPARE.into()),
        (vocabulary::VIEWPORT, vocabulary::VIEWPORT.into()),
    ])
    .as_record()
    .unwrap()
    .clone();
    let env = Failed(Default::default());
    let declaration = Value::Record(fields.clone());
    assert_eq!(
        viewport_output(&declaration, &env, 400.0, 300.0),
        Some(absent::with_reason(::grap::absent::MISSING_CELL))
    );
    assert_eq!(env.0.get(), 1);
    for fuel in [f64::value(-1.0), f64::value(0.5), Value::record([])] {
        fields.insert(layout::vocabulary::FUEL, fuel);
        assert!(viewport_output(&Value::Record(fields.clone()), &env, 400.0, 300.0).is_none());
    }
    assert_eq!(env.0.get(), 1, "malformed fuel declines before preparation");
}

#[test]
fn viewport_preparation_receives_only_data_before_dimensions_are_supplied() {
    struct Prepared(std::cell::Cell<usize>);
    impl Env for Prepared {
        fn apply_runtime_memo(
            &self,
            function: &Value,
            arguments: &[(CellId, Value)],
            fuel: usize,
        ) -> RuntimeValue {
            assert_eq!(function, &Value::from(vocabulary::PREPARE));
            assert_eq!(arguments, &[(vocabulary::VALUE, LEFT_VALUE.into())]);
            assert_eq!(fuel, 1234);
            self.0.set(self.0.get() + 1);
            Value::list([LEFT_VALUE.into()]).into()
        }
        fn apply_expression_runtime(
            &self,
            _: &RuntimeValue,
            arguments: &[(CellId, RuntimeValue)],
        ) -> RuntimeValue {
            assert!(self.0.get() > 0);
            assert_eq!(
                values(arguments),
                [
                    (vocabulary::VALUE, Value::list([LEFT_VALUE.into()])),
                    (layout::vocabulary::WIDTH, f64::value(400.0)),
                    (layout::vocabulary::HEIGHT, f64::value(300.0)),
                ]
            );
            Value::record([]).into()
        }
        fn apply_runtime_scoped(
            &self,
            _: &RuntimeValue,
            _: &[(CellId, RuntimeValue)],
            _: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            unreachable!()
        }
        fn evaluate_runtime(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            unreachable!()
        }
        fn evaluate_runtime_memo(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            unreachable!()
        }
    }
    let declaration = Value::record([
        (vocabulary::VALUE, LEFT_VALUE.into()),
        (vocabulary::PREPARE, vocabulary::PREPARE.into()),
        (layout::vocabulary::FUEL, f64::value(1234.0)),
        (vocabulary::VIEWPORT, vocabulary::VIEWPORT.into()),
    ]);
    let env = Prepared(Default::default());
    assert_eq!(
        viewport_output(&declaration, &env, 400.0, 300.0),
        Some(Value::record([]))
    );
    assert_eq!(env.0.get(), 1);
    projected(&declaration, &env, |input| {
        viewport_display(input, 0.0, 300.0)
    })
    .unwrap();
    assert_eq!(env.0.get(), 1, "a zero-sized viewport doesn't prepare");
}

/// `arguments` as GID data, to compare.
fn values(arguments: &[(CellId, RuntimeValue)]) -> Vec<(CellId, Value)> {
    arguments
        .iter()
        .map(|(key, value)| (*key, value.to_value()))
        .collect()
}

fn projected<V>(
    value: &V,
    env: &dyn Env,
    projection: impl FnOnce(&ProjectionInput<'_, V>) -> Option<Layout>,
) -> Option<Layout> {
    projection(&projection_input(env, value))
}

#[test]
fn declaration_passes_runtime_callable_and_data_without_reification() {
    struct CheckRuntime(RuntimeValue);
    impl Env for CheckRuntime {
        fn apply_expression_runtime(
            &self,
            function: &RuntimeValue,
            arguments: &[(CellId, RuntimeValue)],
        ) -> RuntimeValue {
            assert!(function.same_result(&self.0));
            assert_eq!(arguments.len(), 1);
            assert_eq!(arguments[0].0, vocabulary::VALUE);
            assert!(arguments[0].1.same_result(&self.0));
            RuntimeValue::f64(42.0)
        }
        fn evaluate_runtime(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            panic!("unexpected evaluation")
        }
        fn evaluate_runtime_memo(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
            panic!("unexpected evaluation")
        }
        fn apply_runtime_scoped(
            &self,
            _: &RuntimeValue,
            _: &[(CellId, RuntimeValue)],
            _: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("unexpected application")
        }
        fn apply_runtime_memo(&self, _: &Value, _: &[(CellId, Value)], _: usize) -> RuntimeValue {
            panic!("unexpected application")
        }
    }
    let host = crate::libraries::TestHost(|_| vec![]);
    let callback = ::grap::evaluate_runtime_at(
        &::grap::RuntimeValue::from(::grap::lambda([], f64::value(7.0))),
        Some(::grap::SourceOrigin::Stored(vec![Step::Key(LEFT_VALUE)])),
        &host,
        100,
    )
    .result;
    let declaration = ::grap::RuntimeValue::record([
        (vocabulary::PROJECTION, callback.clone()),
        (vocabulary::VALUE, callback.clone()),
    ]);
    let layout = projected(
        &declaration,
        &CheckRuntime(callback),
        super::projected_display,
    )
    .unwrap();
    assert!(
        matches!(inspect(&layout), ProjectionCall::At { value, .. } if f64::read(&value) == Some(42.0))
    );
}

#[test]
fn declarations_are_inert_until_the_view_opts_in_and_absent_reveals_the_source() {
    let source = Value::from(b"source".to_vec());
    let value = Value::record([
        (vocabulary::VALUE, source),
        (vocabulary::PROJECTION, Value::from(LEFT_VALUE)),
    ]);
    let result = Value::from(b"projected".to_vec());
    assert!(projected(&value, &NoEval, display).is_none());
    assert!(
        matches!((projected(&value, &EvaluatesTo(result.clone()), projected_display)).map(|layout| inspect(&layout)),
        Some(ProjectionCall::At { value, .. }) if value == result)
    );
    assert!(matches!(
        (projected(
            &value,
            &EvaluatesTo(absent::with_reason(LEFT_VALUE)),
            projected_display
        ))
        .map(|layout| inspect(&layout)),
        Some(ProjectionCall::Descend {
            step: Step::Key(vocabulary::VALUE),
            ..
        })
    ));
    assert!(
        projected(
            &Value::record([(vocabulary::VALUE, value)]),
            &NoEval,
            projected_display,
        )
        .is_none()
    );
}

#[test]
fn render_projects_only_the_evaluation_result() {
    let result = Value::from(b"picture".to_vec());
    let value = Value::record([(vocabulary::RENDER, Value::from(LEFT_VALUE))]);
    assert!(
        matches!((projected(&value, &EvaluatesTo(result.clone()), display)).map(|layout| inspect(&layout)),
            Some(ProjectionCall::At { value, .. }) if value == result
        )
    );
}

mod rendered;
