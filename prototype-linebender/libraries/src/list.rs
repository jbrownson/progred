//! Small Grap operations over GID lists. Lists stay ordinary values;
//! these functions only provide the operations awkward to express by
//! structural matching alone.

use crate::{Library, absent, f64, name};
use gid::{Cells, Value};
use grap_runtime::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};

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

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::PREPEND,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_argument(vocabulary::LIST));
                };
                let Some(item) = evaluated(context, call, environment, vocabulary::ITEM)? else {
                    return Ok(context.missing_argument(vocabulary::ITEM));
                };
                Ok(list
                    .as_list()
                    .map(|list| Value::list(std::iter::once(item).chain(list.values().cloned())))
                    .unwrap_or_else(|| Value::from(vocabulary::NOT_LIST)))
            }),
        )
        .register(
            vocabulary::CONCAT,
            ForeignFunction::new(|context, call, environment| {
                let Some(left) = evaluated(context, call, environment, f64::vocabulary::LEFT)?
                else {
                    return Ok(context.missing_argument(f64::vocabulary::LEFT));
                };
                let Some(right) = evaluated(context, call, environment, f64::vocabulary::RIGHT)?
                else {
                    return Ok(context.missing_argument(f64::vocabulary::RIGHT));
                };
                Ok(match (left.as_list(), right.as_list()) {
                    (Some(left), Some(right)) => {
                        Value::list(left.values().chain(right.values()).cloned())
                    }
                    _ => Value::from(vocabulary::NOT_LIST),
                })
            }),
        )
        .register(
            vocabulary::LENGTH,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_argument(vocabulary::LIST));
                };
                Ok(list
                    .as_list()
                    .map(|list| f64::value(list.len() as f64))
                    .unwrap_or_else(|| Value::from(vocabulary::NOT_LIST)))
            }),
        )
        .register(
            vocabulary::AT,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_argument(vocabulary::LIST));
                };
                let Some(index) = evaluated(context, call, environment, vocabulary::INDEX)? else {
                    return Ok(context.missing_argument(vocabulary::INDEX));
                };
                let Some(list) = list.as_list() else {
                    return Ok(Value::from(vocabulary::NOT_LIST));
                };
                let Some(index) = f64::read(&index)
                    .filter(|index| *index >= 0.0 && index.fract() == 0.0)
                    .map(|index| index as usize)
                else {
                    return Ok(Value::from(vocabulary::OUT_OF_BOUNDS));
                };
                Ok(list
                    .values()
                    .nth(index)
                    .cloned()
                    .unwrap_or_else(|| Value::from(vocabulary::OUT_OF_BOUNDS)))
            }),
        )
        .register(
            vocabulary::TAIL,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_argument(vocabulary::LIST));
                };
                Ok(list
                    .as_list()
                    .and_then(|list| {
                        list.values().next()?;
                        Some(Value::list(list.values().skip(1).cloned()))
                    })
                    .unwrap_or_else(|| Value::from(vocabulary::OUT_OF_BOUNDS)))
            }),
        )
        .register(
            vocabulary::UNFOLD,
            ForeignFunction::new(|context, call, environment| {
                let Some(mut state) = evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                let mut items = Vec::new();
                loop {
                    let result =
                        context.call_prepared(&step, [(vocabulary::STATE, state.clone())])?;
                    if absent::is_absent(&result) {
                        break Ok(Value::record([
                            (vocabulary::LIST, Value::list(items)),
                            (vocabulary::STATE, state),
                        ]));
                    }
                    let Some(fields) = result.as_record() else {
                        break Ok(Value::from(vocabulary::INVALID_STEP));
                    };
                    let (Some(item), Some(next)) = (
                        fields.get(&vocabulary::ITEM),
                        fields.get(&vocabulary::STATE),
                    ) else {
                        break Ok(Value::from(vocabulary::INVALID_STEP));
                    };
                    items.push(item.clone());
                    state = next.clone();
                }
            }),
        )
        .register(
            vocabulary::FOLD,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_argument(vocabulary::LIST));
                };
                let Some(mut accumulator) =
                    evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                let Some(list) = list.as_list() else {
                    return Ok(Value::from(vocabulary::NOT_LIST));
                };
                for item in list.values() {
                    accumulator = context.call_prepared(
                        &step,
                        [
                            (vocabulary::ACCUMULATOR, accumulator),
                            (vocabulary::ITEM, item.clone()),
                        ],
                    )?;
                }
                Ok(accumulator)
            }),
        )
        .register(
            vocabulary::ITERATE,
            ForeignFunction::new(|context, call, environment| {
                let Some(mut state) =
                    evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                loop {
                    let next = context.call_prepared(
                        &step,
                        [(vocabulary::STATE, state.clone())],
                    )?;
                    if absent::is_absent(&next) {
                        break Ok(state);
                    }
                    state = next;
                }
            }),
        )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
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
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, spelling) in [
        (vocabulary::NOT_LIST, "not a list"),
        (vocabulary::OUT_OF_BOUNDS, "list index out of bounds"),
        (vocabulary::INVALID_STEP, "invalid unfold step"),
    ] {
        cells.set_value(cell, absent::named(spelling));
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
    use grap_runtime as grap;

    fn call(
        function: gid::CellId,
        fields: impl IntoIterator<Item = (gid::CellId, Value)>,
    ) -> Value {
        grap::call(Value::from(function), fields)
    }

    #[test]
    fn list_operations_are_value_operations() {
        let functions = functions();
        let concatenated = call(
            vocabulary::CONCAT,
            [
                (
                    f64::vocabulary::LEFT,
                    Value::list([Value::from(b"a".to_vec())]),
                ),
                (
                    f64::vocabulary::RIGHT,
                    Value::list([Value::from(b"b".to_vec())]),
                ),
            ],
        );
        let at = call(
            vocabulary::AT,
            [
                (vocabulary::LIST, concatenated),
                (vocabulary::INDEX, f64::value(1.0)),
            ],
        );
        assert_eq!(
            grap::evaluate(&at, |_| None, &functions, 30).result,
            Value::from(b"b".to_vec())
        );
    }

    #[test]
    fn iterate_drives_a_step_without_collecting_intermediate_values() {
        use std::cell::Cell;
        use std::rc::Rc;

        let step = gid::new_cell_id();
        let calls = Rc::new(Cell::new(0));
        let step_calls = calls.clone();
        let functions = functions().register(
            step,
            ForeignFunction::new(move |_, _, _| {
                let call = step_calls.get();
                step_calls.set(call + 1);
                Ok(if call == 3 {
                    absent::value()
                } else {
                    Value::from(vec![call as u8])
                })
            }),
        );
        let iterated = call(
            vocabulary::ITERATE,
            [
                (vocabulary::INITIAL, Value::from(vec![99])),
                (vocabulary::STEP, Value::from(step)),
            ],
        );
        assert_eq!(
            grap::evaluate(&iterated, |_| None, &functions, 30).result,
            Value::from(vec![2]),
        );
        assert_eq!(calls.get(), 4);
    }
}
