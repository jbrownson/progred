//! Projections for computed artifacts. A document may place one in a
//! root-declared pane, while evaluation and projection remain ordinary
//! local value behavior. A projection function receives one argument
//! under [`vocabulary::VALUE`] and returns a value; an absent result
//! declines the projection.

use crate::{Library, f64, layout, name};
use gid::Cells;
use progred_display::{Layout, ProjectionInput, transient};

pub mod vocabulary {
    use gid::CellId;

    pub const RENDER: CellId = CellId::from_u128(0x37cda4bdea0091349e305951564fbdf1);
    /// The single argument of a projection function.
    pub const VALUE: CellId = CellId::from_u128(0x84d3ba81fd2a52ea37478f4a868106f4);
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    if let Some(expression) = fields.get(&vocabulary::RENDER) {
        let evaluated = expression.as_record().and_then(|fields| {
            let expression = fields.get(&grap_runtime::vocabulary::EXPRESSION)?;
            let fuel = f64::read(fields.get(&layout::vocabulary::FUEL)?)?;
            (fuel >= 0.0 && fuel.fract() == 0.0 && fuel <= usize::MAX as f64)
                .then(|| input.env.evaluate_with_fuel(expression, fuel as usize))
        });
        let (result, fuel) = evaluated.unwrap_or_else(|| input.env.evaluate(expression));
        Some(transient(&result, fuel))
    } else {
        None
    }
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [(vocabulary::RENDER, "render"), (vocabulary::VALUE, "value")] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library {
        cells,
        projections: vec![display::<World, Hover>],
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{CellId, Value};
    use progred_display::{Env, ProjectionTargets};
    use std::rc::Rc;

    const LEFT_VALUE: CellId = CellId::from_u128(1);

    struct EvaluateTo(Value);

    impl Env for EvaluateTo {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.0.clone(), 17)
        }
    }

    fn projected(value: &Value, env: &dyn Env) -> Option<Layout<(), ()>> {
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_| false),
            select_with: Rc::new(|_, _| false),
            hover: (),
        };
        display(&ProjectionInput {
            env,
            value,
            writable: true,
            selection: None,
            state: None,
            targets: ProjectionTargets::new(&target),
        })
    }

    #[test]
    fn render_projects_only_the_transient_evaluation_result() {
        let result = Value::from(b"picture".to_vec());
        let value = Value::record([(vocabulary::RENDER, Value::from(LEFT_VALUE))]);
        assert!(matches!(
            projected(&value, &EvaluateTo(result.clone())),
            Some(Layout::Transient { value, fuel: 17 }) if value == result
        ));
    }
}
