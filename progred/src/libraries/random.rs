//! Deterministic random sampling inside an explicit Grap evaluation scope.

use crate::display::Host;
use crate::libraries::{Library, f64, name, u64};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use gid::Value;
use std::cell::Cell;
use std::rc::Rc;

pub const ID: gid::CellId = gid::CellId::from_u128(0x953e2838d5985718dd4e91f0af673fe3);

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
    call: &Expression,
    environment: &Environment,
    field: gid::CellId,
) -> Result<Option<Value>, Halt> {
    context
        .field(call, field)
        .map(|value| context.eval_to_value(value, environment))
        .transpose()
}

fn stream(state: Rc<Cell<u64>>) -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::BETWEEN,
        ForeignFunction::new(move |context, call, environment| {
            let Some(min) = context.field(call, vocabulary::MIN) else {
                return Ok(context.missing_runtime_argument(vocabulary::MIN));
            };
            let Some(max) = context.field(call, vocabulary::MAX) else {
                return Ok(context.missing_runtime_argument(vocabulary::MAX));
            };
            let min = context.eval_f64(min, environment)?;
            let max = context.eval_f64(max, environment)?;
            let (Some(min), Some(max)) = (min, max) else {
                return Ok(::grap::absent::value(vocabulary::INVALID_BOUNDS).into());
            };
            let next = state
                .get()
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            Ok(context.effect(|| {
                state.set(next);
                let unit = ((next >> 11) as f64) / ((1_u64 << 53) as f64);
                RuntimeValue::f64(min + (max - min) * unit)
            }))
        }),
    )
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::BETWEEN,
            ForeignFunction::from_value(|_, _, _| {
                Ok(::grap::absent::value(vocabulary::OUTSIDE_SCOPE))
            })
            .parameters([vocabulary::MIN, vocabulary::MAX]),
        )
        .register(
            vocabulary::WITH_RANDOM,
            ForeignFunction::new(|context, call, environment| {
                // An omitted seed is zero; one that isn't a u64 is invalid.
                match evaluated(context, call, environment, vocabulary::SEED)?
                    .map_or(Some(0), |seed| u64::read(&seed))
                {
                    Some(seed) => match context.field(call, ::grap::vocabulary::EXPRESSION) {
                        Some(expression) => context
                            .with_foreign_functions(stream(Rc::new(Cell::new(seed))), |context| {
                                context.eval(expression, environment)
                            }),
                        None => {
                            Ok(context.missing_runtime_argument(::grap::vocabulary::EXPRESSION))
                        }
                    },
                    None => Ok(::grap::absent::value(vocabulary::INVALID_SEED).into()),
                }
            })
            .parameters([::grap::vocabulary::EXPRESSION]),
        )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "random",
        [
            (vocabulary::BETWEEN, "random between"),
            (vocabulary::MIN, "minimum"),
            (vocabulary::MAX, "maximum"),
            (vocabulary::WITH_RANDOM, "with random"),
            (vocabulary::SEED, "seed"),
            (vocabulary::OUTSIDE_SCOPE, "random outside scope"),
            (vocabulary::INVALID_SEED, "invalid random seed"),
            (vocabulary::INVALID_BOUNDS, "invalid random bounds"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
