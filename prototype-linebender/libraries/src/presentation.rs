//! Document-level composition for showing a computed artifact beside
//! the GID that produces it. Both forms are ordinary projections;
//! the shell has no alternate demo mode.

use crate::{Library, f64, layout, name};
use gid::{Cells, Step};
use progred_display::{Layout, ProjectionInput, row, transient};

pub mod vocabulary {
    use gid::CellId;

    pub const SPLIT: CellId = CellId::from_u128(0x018328f7bb516a8f50847232068fba92);
    pub const RENDER: CellId = CellId::from_u128(0x37cda4bdea0091349e305951564fbdf1);
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    if let Some(split) = fields.get(&vocabulary::SPLIT) {
        let split = split.as_record()?;
        split.get(&layout::vocabulary::LEFT)?;
        split.get(&layout::vocabulary::RIGHT)?;
        Some(row(
            28.0,
            [
                Layout::At {
                    steps: vec![
                        Step::Key(vocabulary::SPLIT),
                        Step::Key(layout::vocabulary::LEFT),
                    ],
                    value: split.get(&layout::vocabulary::LEFT)?.clone(),
                    projection: None,
                },
                Layout::At {
                    steps: vec![
                        Step::Key(vocabulary::SPLIT),
                        Step::Key(layout::vocabulary::RIGHT),
                    ],
                    value: split.get(&layout::vocabulary::RIGHT)?.clone(),
                    projection: None,
                },
            ],
        ))
    } else if let Some(expression) = fields.get(&vocabulary::RENDER) {
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
    for (cell, spelling) in [(vocabulary::SPLIT, "split"), (vocabulary::RENDER, "render")] {
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
    use progred_display::{ActionHandler, Env, ProjectionTargets};
    use std::rc::Rc;

    const LEFT_VALUE: CellId = CellId::from_u128(1);
    const RIGHT_VALUE: CellId = CellId::from_u128(2);

    struct EvaluateTo(Value);

    impl Env for EvaluateTo {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.0.clone(), 17)
        }
    }

    fn projected(value: &Value, env: &dyn Env) -> Option<Layout<(), ()>> {
        let select: ActionHandler<()> = Rc::new(|_| false);
        display(&ProjectionInput {
            env,
            value,
            selection: None,
            state: None,
            select: select.clone(),
            hover: (),
            targets: ProjectionTargets::fixed(select, ()),
        })
    }

    #[test]
    fn split_projects_both_stored_sides_at_their_real_paths() {
        let value = Value::record([(
            vocabulary::SPLIT,
            Value::record([
                (layout::vocabulary::LEFT, Value::from(LEFT_VALUE)),
                (layout::vocabulary::RIGHT, Value::from(RIGHT_VALUE)),
            ]),
        )]);
        let Some(Layout::Row { children, .. }) = projected(&value, &EvaluateTo(value.clone()))
        else {
            panic!("split row");
        };
        assert!(matches!(
            children.as_slice(),
            [
                Layout::At { steps: left, .. },
                Layout::At { steps: right, .. }
            ] if left == &[
                Step::Key(vocabulary::SPLIT),
                Step::Key(layout::vocabulary::LEFT)
            ] && right == &[
                Step::Key(vocabulary::SPLIT),
                Step::Key(layout::vocabulary::RIGHT)
            ]
        ));
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
