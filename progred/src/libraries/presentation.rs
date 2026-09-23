//! Projections for computed artifacts. A document may place one in a
//! pane, whose view opts into applying its declared projection. The
//! document view leaves that declaration editable. A projection function
//! receives one argument under [`vocabulary::VALUE`] and returns a value;
//! an absent result exposes its source. An explicit viewport function also
//! receives the pane's assigned logical width and height.

#[cfg(test)]
use crate::libraries::absent;
use crate::libraries::{Library, f64, layout, name};
use gid::{Cells, Step};

mod outline;

pub const ID: gid::CellId = gid::CellId::from_u128(0xd22b834154d60b1df228f9bb4d3c13de);
use crate::display::{Layout, ProjectionInput, at};

pub mod vocabulary {
    use gid::CellId;

    pub const RENDER: CellId = CellId::from_u128(0x37cda4bdea0091349e305951564fbdf1);
    pub const PROJECTION: CellId = CellId::from_u128(0x873503e2e37a1722a0dd21399be9ee7f);
    pub const VIEWPORT: CellId = CellId::from_u128(0x709c987e4c2a110931d8597c4da68f69);
    /// Optional size-independent preparation of a viewport's data input.
    pub const PREPARE: CellId = CellId::from_u128(0x8b782978097343efd90f6fb61f649419);
    /// The single argument of a projection function.
    pub const VALUE: CellId = CellId::from_u128(0x84d3ba81fd2a52ea37478f4a868106f4);
    /// Ordered field references for an opt-in record outline.
    pub const OUTLINE: CellId = CellId::from_u128(0xe6c0b2058b8468428d64576d953e2d62);
    /// Occurrence step for a computed result, not a field in the source record.
    pub const RESULT: CellId = CellId::from_u128(0x11098129f74918af8a0924f94c37df4f);
}

pub fn viewport(value: &gid::Value) -> Option<(&gid::Value, &gid::Value)> {
    let fields = value.as_record()?;
    Some((
        fields.get(&vocabulary::VALUE)?,
        fields.get(&vocabulary::VIEWPORT)?,
    ))
}

/// A viewport function receives its assigned logical size, not a size
/// inferred from its output. Its result uses the ordinary layout and handlers.
pub fn viewport_display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    width: f64,
    height: f64,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    viewport(input.value?)?;
    if width <= 0.0 || height <= 0.0 {
        return Some(crate::display::row(0.0, []));
    }
    let result = viewport_runtime_output(input.value?, input.env, width, height)?;
    Some(if result.is_absent() {
        crate::display::descend(Step::Key(vocabulary::VALUE), None, None)
    } else {
        at([Step::Key(vocabulary::RESULT)], &result)
    })
}

#[cfg(test)]
pub(crate) fn viewport_output(
    declaration: &gid::Value,
    env: &dyn crate::display::Env,
    width: f64,
    height: f64,
) -> Option<gid::Value> {
    viewport_runtime_output(declaration, env, width, height).map(::grap::RuntimeValue::into_value)
}

pub(crate) fn viewport_runtime_output(
    declaration: &gid::Value,
    env: &dyn crate::display::Env,
    width: f64,
    height: f64,
) -> Option<::grap::RuntimeValue> {
    let (value, function) = viewport(declaration)?;
    let fields = declaration.as_record()?;
    let value = match fields.get(&vocabulary::PREPARE) {
        Some(prepare) => {
            let fuel = match fields.get(&layout::vocabulary::FUEL) {
                Some(value) => {
                    let fuel = f64::read(value)?;
                    (fuel >= 0.0 && fuel.fract() == 0.0 && fuel <= usize::MAX as f64)
                        .then_some(fuel as usize)?
                }
                None => ::grap::DEFAULT_FUEL,
            };
            let prepared =
                env.apply_runtime_memo(prepare, &[(vocabulary::VALUE, value.clone())], fuel);
            if prepared.is_absent() {
                return Some(prepared);
            }
            prepared
        }
        None => value.into(),
    };
    Some(env.apply_expression_runtime(
        function,
        &[
            (vocabulary::VALUE, value),
            (layout::vocabulary::WIDTH, ::grap::RuntimeValue::f64(width)),
            (
                layout::vocabulary::HEIGHT,
                ::grap::RuntimeValue::f64(height),
            ),
        ],
    ))
}

pub fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?;
    let expression = fields.get(&vocabulary::RENDER)?;
    let evaluated = expression.as_record().and_then(|fields| {
        let expression = fields.get(&::grap::vocabulary::EXPRESSION)?;
        let fuel = f64::read(fields.get(&layout::vocabulary::FUEL)?)?;
        (fuel >= 0.0 && fuel.fract() == 0.0 && fuel <= usize::MAX as f64).then(|| {
            input.env.evaluate_runtime_memo(
                expression,
                fuel as usize,
                &[
                    Step::Key(vocabulary::RENDER),
                    Step::Key(::grap::vocabulary::EXPRESSION),
                ],
            )
        })
    });
    let result = evaluated.unwrap_or_else(|| {
        input.env.evaluate_runtime_memo(
            expression,
            ::grap::DEFAULT_FUEL,
            &[Step::Key(vocabulary::RENDER)],
        )
    });
    Some(at([Step::Key(vocabulary::RESULT)], &result))
}

/// Opt-in presentation of a declaration; not part of the library's
/// ordinary authoring projection.
pub fn projected_display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?;
    let value = fields.get(&vocabulary::VALUE)?;
    let function = fields.get(&vocabulary::PROJECTION)?;
    let result = input
        .env
        .apply_expression_runtime(function, &[(vocabulary::VALUE, value.into())]);
    Some(if result.is_absent() {
        crate::display::descend(Step::Key(vocabulary::VALUE), None, None)
    } else {
        at([Step::Key(vocabulary::RESULT)], &result)
    })
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::RENDER, "render"),
        (vocabulary::PROJECTION, "projection"),
        (vocabulary::VIEWPORT, "viewport"),
        (vocabulary::PREPARE, "prepare"),
        (vocabulary::VALUE, "value"),
        (vocabulary::OUTLINE, "outline"),
        (vocabulary::RESULT, "result"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "presentation",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
        crate::display::compose_partials([
            crate::display::partial(display),
            crate::display::partial(outline::display),
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recordable, Recorded};

    use crate::display::test_support::{ProjectionCall, inspect};
    use crate::display::{Env, ProjectionTargets};
    use gid::{CellId, Value};
    use std::rc::Rc;

    const LEFT_VALUE: CellId = CellId::from_u128(1);

    #[test]
    fn viewport_passes_logical_dimensions_and_preserves_its_value_as_data() {
        struct CheckArguments;
        impl Env for CheckArguments {
            fn apply_scoped(
                &self,
                function: &Value,
                arguments: &[(CellId, Value)],
                _scope: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation<gid::Value> {
                assert_eq!(function, &Value::from(LEFT_VALUE));
                assert_eq!(
                    arguments,
                    &[
                        (vocabulary::VALUE, Value::from(LEFT_VALUE)),
                        (layout::vocabulary::WIDTH, f64::value(420.5)),
                        (layout::vocabulary::HEIGHT, f64::value(160.25)),
                    ]
                );
                ::grap::Evaluation {
                    result: layout::drawing(420.5, 0.0, 160.25, []),
                    remaining_fuel: 17,
                    completed: true,
                }
            }
            fn evaluate(&self, _: &Value) -> Value {
                panic!("viewport sources are passed as data")
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

    struct EvaluateTo(Value);

    #[test]
    fn viewport_preparation_failure_does_not_call_the_viewport() {
        struct Failed(std::cell::Cell<usize>);
        impl Env for Failed {
            fn apply_memo(&self, _: &Value, _: &[(CellId, Value)], fuel: usize) -> Value {
                assert_eq!(fuel, ::grap::DEFAULT_FUEL);
                self.0.set(self.0.get() + 1);
                absent::with_reason(::grap::absent::MISSING_CELL)
            }
            fn apply_scoped(
                &self,
                _: &Value,
                _: &[(CellId, Value)],
                _: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation<gid::Value> {
                panic!("the viewport must not run after preparation failed")
            }
            fn evaluate(&self, _: &Value) -> Value {
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
            fn apply_memo(
                &self,
                function: &Value,
                arguments: &[(CellId, Value)],
                fuel: usize,
            ) -> Value {
                assert_eq!(function, &Value::from(vocabulary::PREPARE));
                assert_eq!(arguments, &[(vocabulary::VALUE, LEFT_VALUE.into())]);
                assert_eq!(fuel, 1234);
                self.0.set(self.0.get() + 1);
                Value::list([LEFT_VALUE.into()])
            }
            fn apply_scoped(
                &self,
                _: &Value,
                arguments: &[(CellId, Value)],
                _: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation<gid::Value> {
                assert!(self.0.get() > 0);
                assert_eq!(
                    arguments[0],
                    (vocabulary::VALUE, Value::list([LEFT_VALUE.into()]))
                );
                assert_eq!(arguments[1], (layout::vocabulary::WIDTH, f64::value(400.0)));
                assert_eq!(
                    arguments[2],
                    (layout::vocabulary::HEIGHT, f64::value(300.0))
                );
                ::grap::Evaluation {
                    result: Value::record([]),
                    remaining_fuel: 0,
                    completed: true,
                }
            }
            fn evaluate(&self, _: &Value) -> Value {
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

    impl Env for EvaluateTo {
        fn apply_scoped(
            &self,
            _: &gid::Value,
            _: &[(gid::CellId, gid::Value)],
            _scope: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation<gid::Value> {
            ::grap::Evaluation {
                result: self.0.clone(),
                remaining_fuel: 17,
                completed: true,
            }
        }

        fn evaluate(&self, _: &Value) -> Value {
            self.0.clone()
        }
    }

    fn projected(
        value: &Value,
        env: &dyn Env,
        projection: impl FnOnce(
            &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
        ) -> Option<Layout<crate::Editor, crate::frame::Hovered>>,
    ) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
        let target = |_| crate::display::ProjectionTarget {
            select: Rc::new(|_| false),
            select_with: Rc::new(|_, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        };
        projection(&ProjectionInput {
            default_projection: crate::display::runtime_partial(|_| None),
            env,
            value: Some(value),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: ProjectionTargets::new(&target),
        })
    }

    #[test]
    fn declarations_are_inert_until_the_view_opts_in_and_absent_reveals_the_source() {
        struct NoEvaluation;
        impl Env for NoEvaluation {
            fn apply_scoped(
                &self,
                _: &Value,
                _: &[(CellId, Value)],
                _scope: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation<gid::Value> {
                panic!("authoring a declaration must not apply its projection")
            }
            fn evaluate(&self, _: &Value) -> Value {
                panic!("authoring a declaration must not evaluate it")
            }
        }
        let source = Value::from(b"source".to_vec());
        let value = Value::record([
            (vocabulary::VALUE, source),
            (vocabulary::PROJECTION, Value::from(LEFT_VALUE)),
        ]);
        let result = Value::from(b"projected".to_vec());
        assert!(projected(&value, &NoEvaluation, display).is_none());
        assert!(
            matches!((projected(&value, &EvaluateTo(result.clone()), projected_display)).map(|layout| inspect(&layout)),
            Some(ProjectionCall::At { value, .. }) if value == result)
        );
        assert!(matches!(
            (projected(
                &value,
                &EvaluateTo(absent::with_reason(LEFT_VALUE)),
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
                &NoEvaluation,
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
            matches!((projected(&value, &EvaluateTo(result.clone()), display)).map(|layout| inspect(&layout)),
                Some(ProjectionCall::At { value, .. }) if value == result
            )
        );
    }
}
