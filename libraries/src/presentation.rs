//! Projections for computed artifacts. A document may place one in a
//! pane, whose view opts into applying its declared projection. The
//! document view leaves that declaration editable. A projection function
//! receives one argument under [`vocabulary::VALUE`] and returns a value;
//! an absent result exposes its source. An explicit viewport function also
//! receives the pane's assigned logical width and height.

use crate::{Library, absent, f64, layout, name};
use gid::{Cells, Step};

pub const ID: gid::CellId = gid::CellId::from_u128(0xd22b834154d60b1df228f9bb4d3c13de);
use progred_display::{Layout, ProjectionInput, transient};

pub mod vocabulary {
    use gid::CellId;

    pub const RENDER: CellId = CellId::from_u128(0x37cda4bdea0091349e305951564fbdf1);
    pub const PROJECTION: CellId = CellId::from_u128(0x873503e2e37a1722a0dd21399be9ee7f);
    pub const VIEWPORT: CellId = CellId::from_u128(0x709c987e4c2a110931d8597c4da68f69);
    /// The single argument of a projection function.
    pub const VALUE: CellId = CellId::from_u128(0x84d3ba81fd2a52ea37478f4a868106f4);
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
pub fn viewport_display<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
    width: f64,
    height: f64,
) -> Option<Layout<World, Hover>> {
    let (value, function) = viewport(input.value)?;
    if width <= 0.0 || height <= 0.0 {
        return Some(progred_display::row(0.0, []));
    }
    let (result, fuel) = input.env.apply(
        function,
        &[
            (vocabulary::VALUE, value.clone()),
            (layout::vocabulary::WIDTH, f64::value(width)),
            (layout::vocabulary::HEIGHT, f64::value(height)),
        ],
    );
    Some(if absent::is_absent(&result) {
        progred_display::descend(Step::Key(vocabulary::VALUE), None, None)
    } else {
        transient(&result, fuel)
    })
}

pub fn display<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let expression = fields.get(&vocabulary::RENDER)?;
    let evaluated = expression.as_record().and_then(|fields| {
        let expression = fields.get(&grap_runtime::vocabulary::EXPRESSION)?;
        let fuel = f64::read(fields.get(&layout::vocabulary::FUEL)?)?;
        (fuel >= 0.0 && fuel.fract() == 0.0 && fuel <= usize::MAX as f64)
            .then(|| input.env.evaluate_with_fuel(expression, fuel as usize))
    });
    let (result, fuel) = evaluated.unwrap_or_else(|| input.env.evaluate(expression));
    Some(transient(&result, fuel))
}

/// Opt-in presentation of a declaration; not part of the library's
/// ordinary authoring projection.
pub fn projected_display<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let value = fields.get(&vocabulary::VALUE)?;
    let function = fields.get(&vocabulary::PROJECTION)?;
    let (result, fuel) = input
        .env
        .apply(function, &[(vocabulary::VALUE, value.clone())]);
    Some(if absent::is_absent(&result) {
        progred_display::descend(Step::Key(vocabulary::VALUE), None, None)
    } else {
        transient(&result, fuel)
    })
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::RENDER, "render"),
        (vocabulary::PROJECTION, "projection"),
        (vocabulary::VIEWPORT, "viewport"),
        (vocabulary::VALUE, "value"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "presentation",
        crate::Definitions::from_parts(cells, Default::default()),
        progred_display::partial(display::<World, Hover>),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{CellId, Value};
    use progred_display::{Env, ProjectionTargets};
    use std::rc::Rc;

    const LEFT_VALUE: CellId = CellId::from_u128(1);

    #[test]
    fn viewport_passes_logical_dimensions_and_preserves_its_value_as_data() {
        struct CheckArguments;
        impl Env for CheckArguments {
            fn apply(&self, function: &Value, arguments: &[(CellId, Value)]) -> (Value, usize) {
                assert_eq!(function, &Value::from(LEFT_VALUE));
                assert_eq!(
                    arguments,
                    &[
                        (vocabulary::VALUE, Value::from(LEFT_VALUE)),
                        (layout::vocabulary::WIDTH, f64::value(420.5)),
                        (layout::vocabulary::HEIGHT, f64::value(160.25)),
                    ]
                );
                (layout::drawing(420.5, 0.0, 160.25, []), 17)
            }
            fn evaluate(&self, _: &Value) -> (Value, usize) {
                panic!("viewport sources are passed as data")
            }
        }
        let declaration = Value::record([
            (vocabulary::VALUE, LEFT_VALUE.into()),
            (vocabulary::VIEWPORT, LEFT_VALUE.into()),
        ]);
        assert!(projected(&declaration, &CheckArguments, display).is_none());
        assert!(matches!(
            projected(&declaration, &CheckArguments, |input| viewport_display(
                input, 420.5, 160.25
            )),
            Some(Layout::Transient { fuel: 17, .. })
        ));
        assert!(matches!(
            projected(&declaration, &CheckArguments, |input| viewport_display(
                input, 0.0, 160.25
            )),
            Some(Layout::Row { .. })
        ));
    }

    struct EvaluateTo(Value);

    impl Env for EvaluateTo {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            (self.0.clone(), 17)
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.0.clone(), 17)
        }
    }

    fn projected(
        value: &Value,
        env: &dyn Env,
        projection: impl FnOnce(&ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>>,
    ) -> Option<Layout<(), ()>> {
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_| false),
            select_with: Rc::new(|_, _| false),
            hover: (),
        };
        projection(&ProjectionInput {
            env,
            value,
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
            fn apply(&self, _: &Value, _: &[(CellId, Value)]) -> (Value, usize) {
                panic!("authoring a declaration must not apply its projection")
            }
            fn evaluate(&self, _: &Value) -> (Value, usize) {
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
            matches!(projected(&value, &EvaluateTo(result.clone()), projected_display),
            Some(Layout::Transient { value, fuel: 17 }) if value == result)
        );
        assert!(matches!(
            projected(
                &value,
                &EvaluateTo(absent::with_reason(LEFT_VALUE)),
                projected_display
            ),
            Some(Layout::Descend {
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
    fn render_projects_only_the_transient_evaluation_result() {
        let result = Value::from(b"picture".to_vec());
        let value = Value::record([(vocabulary::RENDER, Value::from(LEFT_VALUE))]);
        assert!(matches!(
            projected(&value, &EvaluateTo(result.clone()), display),
            Some(Layout::Transient { value, fuel: 17 }) if value == result
        ));
    }
}
