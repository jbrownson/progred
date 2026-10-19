//! Small Grap operations over GID lists. Lists stay ordinary values;
//! these functions only provide the operations awkward to express by
//! structural matching alone.

use crate::display::Host;
use crate::libraries::{Library, f64, name, number};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use gid::Value;

pub const ID: gid::CellId = gid::CellId::from_u128(0x7b0fa421250c1b5c8a78a3a95b172cb6);

/// Half-open index ranges are ordinary two-element lists of f64 integers.
pub(crate) fn index_range(value: &Value) -> Option<std::ops::Range<usize>> {
    let list = value.as_list()?;
    (list.len() == 2).then_some(())?;
    let mut values = list.values();
    let mut next = || ::grap::f64::to_usize(f64::read(values.next()?)?);
    let range = next()?..next()?;
    (range.start <= range.end).then_some(range)
}

pub mod vocabulary {
    use gid::CellId;

    pub const PREPEND: CellId = CellId::from_u128(0x06f34b889ba63141b4b62f02d8d49aa9);
    pub const CONCAT: CellId = CellId::from_u128(0x6f2613d22b4690ed5bac39776e51b775);
    pub const AT: CellId = CellId::from_u128(0xe56c968fe11bbf17b2033d12f5af5472);
    pub const LENGTH: CellId = CellId::from_u128(0x6882dc6ab06d393217abe6f888585499);
    pub const LIST: CellId = CellId::from_u128(0x216008ae157998d3838112b96e584d11);
    pub const ITEM: CellId = CellId::from_u128(0x67bdfe4ec7d3bafc14407235618840aa);
    pub const INDEX: CellId = CellId::from_u128(0x15db24ee6ff0354d94c0628fc412d64f);
    pub const NOT_LIST: CellId = CellId::from_u128(0xeceebe863edf24d51083bc40fa452635);
    pub const OUT_OF_BOUNDS: CellId = CellId::from_u128(0x9c9b7fd643b08a94a32783f6a6fe6f2e);
    pub const TAIL: CellId = CellId::from_u128(0xb5664e4ebf6aef871a4a02b0aa26d036);
    pub const UNFOLD: CellId = CellId::from_u128(0xb71c173bae05b26bfdca3b5bc79b2b09);
    pub const FOLD: CellId = CellId::from_u128(0x1df93e0aeb8cbeda81a7d513320dd4f0);
    pub const ITERATE: CellId = CellId::from_u128(0x9546343518fe44f380e6134f3f56cf37);
    pub const INITIAL: CellId = CellId::from_u128(0x24d55aeab9383d53899091958de88d21);
    pub const STEP: CellId = CellId::from_u128(0x569e33fee165d346791e05c0e9cfea8e);
    pub const ACCUMULATOR: CellId = CellId::from_u128(0xb6bd623b43e80b7f039873ab5f6afa13);
    pub const STATE: CellId = CellId::from_u128(0x3775735ae4c156144c110ba819cb1e39);
    pub const INVALID_STEP: CellId = CellId::from_u128(0xfd524cb1a241f60ebdb192db4d9ce9c5);
    pub const FINISHED: CellId = CellId::from_u128(0xb19fe726e3c2eab7a5d81cda12cf44b7);
}

fn evaluated(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: gid::CellId,
) -> Result<Option<RuntimeValue>, Halt> {
    context
        .field(call, field)
        .map(|value| context.eval(value, environment))
        .transpose()
}

fn not_list() -> RuntimeValue {
    ::grap::absent::value(vocabulary::NOT_LIST).into()
}

fn out_of_bounds() -> RuntimeValue {
    ::grap::absent::value(vocabulary::OUT_OF_BOUNDS).into()
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::PREPEND,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                let Some(item) = evaluated(context, call, environment, vocabulary::ITEM)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::ITEM));
                };
                Ok(match list.list_values() {
                    Some(values) => RuntimeValue::list(std::iter::once(item).chain(values)),
                    None => not_list(),
                })
            })
            .parameters([vocabulary::LIST, vocabulary::ITEM])
            .tracked(),
        )
        .register(
            vocabulary::CONCAT,
            ForeignFunction::new(|context, call, environment| {
                let Some(left) = evaluated(context, call, environment, number::vocabulary::LEFT)?
                else {
                    return Ok(context.missing_runtime_argument(number::vocabulary::LEFT));
                };
                let Some(right) = evaluated(context, call, environment, number::vocabulary::RIGHT)?
                else {
                    return Ok(context.missing_runtime_argument(number::vocabulary::RIGHT));
                };
                Ok(match (left.list_values(), right.list_values()) {
                    (Some(left), Some(right)) => RuntimeValue::list(left.chain(right)),
                    _ => not_list(),
                })
            })
            .parameters([number::vocabulary::LEFT, number::vocabulary::RIGHT])
            .tracked(),
        )
        .register(
            vocabulary::LENGTH,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                Ok(list
                    .list_len()
                    .map(|length| RuntimeValue::f64(length as f64))
                    .unwrap_or_else(not_list))
            })
            .parameters([vocabulary::LIST])
            .tracked(),
        )
        .register(
            vocabulary::AT,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                let Some(index) = context.field(call, vocabulary::INDEX) else {
                    return Ok(context.missing_runtime_argument(vocabulary::INDEX));
                };
                let index = context.eval(index, environment)?;
                Ok(match list.list_len() {
                    Some(_) => index
                        .as_f64()
                        .and_then(::grap::f64::to_usize)
                        .and_then(|index| list.list_get(index))
                        .unwrap_or_else(out_of_bounds),
                    None => not_list(),
                })
            })
            .parameters([vocabulary::LIST, vocabulary::INDEX])
            .tracked(),
        )
        .register(
            vocabulary::TAIL,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                Ok(match list.list_values() {
                    Some(mut values) => match values.next() {
                        Some(_) => RuntimeValue::list(values),
                        None => out_of_bounds(),
                    },
                    None => not_list(),
                })
            })
            .parameters([vocabulary::LIST])
            .tracked(),
        )
        .register(
            vocabulary::UNFOLD,
            ForeignFunction::new(|context, call, environment| {
                let Some(mut state) = evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_runtime_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                let mut items = Vec::new();
                loop {
                    let result =
                        context.call_prepared(&step, [(vocabulary::STATE, state.clone())])?;
                    match (
                        result.absent_reason(),
                        result.field(vocabulary::ITEM),
                        result.field(vocabulary::STATE),
                    ) {
                        (Some(vocabulary::FINISHED), _, _) => {
                            break Ok(RuntimeValue::record([
                                (vocabulary::LIST, RuntimeValue::list(items)),
                                (vocabulary::STATE, state),
                            ]));
                        }
                        (Some(_), _, _) => break Ok(result),
                        (None, Some(item), Some(next)) => {
                            items.push(item);
                            state = next;
                        }
                        (None, _, _) => {
                            break Ok(::grap::absent::value(vocabulary::INVALID_STEP).into());
                        }
                    }
                }
            })
            .parameters([vocabulary::INITIAL, vocabulary::STEP])
            .tracked(),
        )
        .register(
            vocabulary::FOLD,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                let Some(accumulator) = evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_runtime_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                match list.list_values() {
                    Some(mut values) => values.try_fold(accumulator, |accumulator, item| {
                        context.call_prepared(
                            &step,
                            [
                                (vocabulary::ACCUMULATOR, accumulator),
                                (vocabulary::ITEM, item),
                            ],
                        )
                    }),
                    None => Ok(not_list()),
                }
            })
            .parameters([vocabulary::LIST, vocabulary::INITIAL, vocabulary::STEP])
            .tracked(),
        )
        .register(
            vocabulary::ITERATE,
            ForeignFunction::new(|context, call, environment| {
                let Some(mut state) = evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_runtime_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                loop {
                    let next =
                        context.call_prepared(&step, [(vocabulary::STATE, state.clone())])?;
                    if next.is_absent() {
                        break Ok(if next.absent_reason() == Some(vocabulary::FINISHED) {
                            state
                        } else {
                            next
                        });
                    }
                    state = next;
                }
            })
            .parameters([vocabulary::INITIAL, vocabulary::STEP])
            .tracked(),
        )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "list",
        [
            (vocabulary::PREPEND, "prepend"),
            (vocabulary::CONCAT, "concat"),
            (vocabulary::AT, "list at"),
            (vocabulary::LENGTH, "list length"),
            (vocabulary::LIST, "list"),
            (vocabulary::ITEM, "item"),
            (vocabulary::INDEX, "index"),
            (vocabulary::TAIL, "list tail"),
            (vocabulary::UNFOLD, "unfold"),
            (vocabulary::FOLD, "fold"),
            (vocabulary::ITERATE, "iterate"),
            (vocabulary::INITIAL, "initial"),
            (vocabulary::STEP, "step"),
            (vocabulary::ACCUMULATOR, "accumulator"),
            (vocabulary::STATE, "state"),
            (vocabulary::NOT_LIST, "not a list"),
            (vocabulary::OUT_OF_BOUNDS, "list index out of bounds"),
            (vocabulary::INVALID_STEP, "invalid unfold step"),
            (vocabulary::FINISHED, "iteration finished"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
