//! Deterministic random sampling inside an explicit Grap evaluation scope.

use crate::{Library, absent, f64, name, u64};
use gid::{Cells, Value};
use grap_runtime::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use std::cell::Cell;
use std::rc::Rc;

pub mod vocabulary {
    use gid::CellId;

    pub const BETWEEN: CellId = CellId::from_u128(0xe8a99ada91800bca33709da0db80c29f);
    pub const MIN: CellId = CellId::from_u128(0x61213005b01abca2d4fb351cc4fe4d98);
    pub const MAX: CellId = CellId::from_u128(0x4be40301b427aeed939ef079ab63cf92);
    pub const WITH_RANDOM: CellId = CellId::from_u128(0xf47ef735130ecf7a72105ac58f30075a);
    pub const SEED: CellId = CellId::from_u128(0x208c3026e79e44fdbe405992c6cdea79);
    pub const OUTSIDE_SCOPE: CellId = CellId::from_u128(0x7b590515407fafa2d520ed1bd0fc6b18);
    pub const INVALID_SEED: CellId = CellId::from_u128(0xc188207adaefeee3f95d567476bd2d16);
    pub const INVALID_BOUNDS: CellId = CellId::from_u128(0xdd466492d252f6c5100386cb7d6d9f63);
}

fn evaluated(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    field: gid::CellId,
) -> Result<Option<Value>, Halt> {
    context
        .field(call, field)
        .map(|value| context.eval(value, environment))
        .transpose()
}

fn stream(state: Rc<Cell<u64>>) -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::BETWEEN,
        ForeignFunction::runtime(move |context, call, environment| {
            let Some(min) = context.field(call, vocabulary::MIN) else {
                return Ok(context.missing_runtime_argument(vocabulary::MIN));
            };
            let Some(max) = context.field(call, vocabulary::MAX) else {
                return Ok(context.missing_runtime_argument(vocabulary::MAX));
            };
            let min = context.eval_f64(min, environment, f64::read)?;
            let max = context.eval_f64(max, environment, f64::read)?;
            let (Some(min), Some(max)) = (min, max) else {
                return Ok(absent::with_reason(vocabulary::INVALID_BOUNDS).into());
            };
            let next = state
                .get()
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state.set(next);
            let unit = ((next >> 11) as f64) / ((1_u64 << 53) as f64);
            Ok(RuntimeValue::f64(min + (max - min) * unit, f64::value))
        }),
    )
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::BETWEEN,
            ForeignFunction::new(|_, _, _| {
                Ok(absent::with_reason(vocabulary::OUTSIDE_SCOPE))
            }),
        )
        .register(
            vocabulary::WITH_RANDOM,
            ForeignFunction::runtime(|context, call, environment| {
                let seed = match evaluated(context, call, environment, vocabulary::SEED)? {
                    Some(seed) => match u64::read(&seed) {
                        Some(seed) => seed,
                        None => {
                            return Ok(absent::with_reason(vocabulary::INVALID_SEED).into());
                        }
                    },
                    None => 0,
                };
                let Some(expression) = context.field(call, grap_runtime::vocabulary::EXPRESSION)
                else {
                    return Ok(context
                        .missing_runtime_argument(grap_runtime::vocabulary::EXPRESSION));
                };
                context.with_foreign_functions(stream(Rc::new(Cell::new(seed))), |context| {
                    context.eval_runtime(expression, environment)
                })
            }),
        )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::BETWEEN, "random between"),
        (vocabulary::MIN, "minimum"),
        (vocabulary::MAX, "maximum"),
        (vocabulary::WITH_RANDOM, "with random"),
        (vocabulary::SEED, "seed"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, spelling) in [
        (vocabulary::OUTSIDE_SCOPE, "random outside scope"),
        (vocabulary::INVALID_SEED, "invalid random seed"),
        (vocabulary::INVALID_BOUNDS, "invalid random bounds"),
    ] {
        cells.set_value(cell, absent::named_reason(spelling));
    }
    Library {
        cells,
        functions: functions(),
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use grap_runtime as grap;

    fn sample(seed: Option<Value>) -> Value {
        let arguments = [
            Some((
                grap::vocabulary::EXPRESSION,
                grap::call(
                    Value::from(vocabulary::BETWEEN),
                    [
                        (vocabulary::MIN, f64::value(-2.0)),
                        (vocabulary::MAX, f64::value(3.0)),
                    ],
                ),
            )),
            seed.map(|seed| (vocabulary::SEED, seed)),
        ];
        grap::evaluate(
            &grap::call(
                Value::from(vocabulary::WITH_RANDOM),
                arguments.into_iter().flatten(),
            ),
            |_| None,
            &functions(),
            30,
        )
        .result
    }

    #[test]
    fn a_seeded_scope_repeats_and_zero_is_the_default() {
        assert_eq!(sample(None), sample(Some(u64::value(0))));
        assert_eq!(sample(Some(u64::value(42))), sample(Some(u64::value(42))));
        assert_ne!(sample(Some(u64::value(0))), sample(Some(u64::value(42))));
    }

    #[test]
    fn successive_samples_advance_the_scoped_stream() {
        let pair = new_cell_id();
        let first_field = new_cell_id();
        let second_field = new_cell_id();
        let functions = functions().register(
            pair,
            ForeignFunction::new(move |context, call, environment| {
                let first = context
                    .field(call, first_field)
                    .map(|expression| context.eval(expression, environment))
                    .transpose()?
                    .unwrap_or_else(|| context.missing_argument(first_field));
                let second = context
                    .field(call, second_field)
                    .map(|expression| context.eval(expression, environment))
                    .transpose()?
                    .unwrap_or_else(|| context.missing_argument(second_field));
                Ok(Value::list([first, second]))
            }),
        );
        let between = || {
            grap::call(
                Value::from(vocabulary::BETWEEN),
                [
                    (vocabulary::MIN, f64::value(0.0)),
                    (vocabulary::MAX, f64::value(1.0)),
                ],
            )
        };
        let expression = grap::call(
            Value::from(vocabulary::WITH_RANDOM),
            [
                (vocabulary::SEED, u64::value(42)),
                (
                    grap::vocabulary::EXPRESSION,
                    grap::call(
                        Value::from(pair),
                        [(first_field, between()), (second_field, between())],
                    ),
                ),
            ],
        );
        let evaluate = || grap::evaluate(&expression, |_| None, &functions, 60).result;
        let result = evaluate();

        assert!(result.as_list().is_some_and(|values| {
            let mut values = values.values();
            matches!(
                (values.next(), values.next(), values.next()),
                (Some(first), Some(second), None) if first != second
            )
        }));
        assert_eq!(result, evaluate());
    }

    #[test]
    fn sampling_outside_a_scope_returns_a_stable_absent() {
        let result = grap::evaluate(
            &grap::call(
                Value::from(vocabulary::BETWEEN),
                [
                    (vocabulary::MIN, f64::value(0.0)),
                    (vocabulary::MAX, f64::value(1.0)),
                ],
            ),
            |_| None,
            &functions(),
            20,
        )
        .result;

        assert_eq!(result, absent::with_reason(vocabulary::OUTSIDE_SCOPE));
    }
}
