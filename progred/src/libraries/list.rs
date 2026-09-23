//! Small Grap operations over GID lists. Lists stay ordinary values;
//! these functions only provide the operations awkward to express by
//! structural matching alone.

use crate::libraries::{Library, absent, f64, name, number};
use gid::Cells;

pub const ID: gid::CellId = gid::CellId::from_u128(0x7b0fa421250c1b5c8a78a3a95b172cb6);
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use gid::Value;

/// Half-open index ranges are ordinary two-element lists of f64 integers.
pub(crate) fn index_range(value: &Value) -> Option<std::ops::Range<usize>> {
    let list = value.as_list()?;
    if list.len() != 2 {
        return None;
    }
    let mut values = list.values();
    let mut next = || {
        let n = f64::read(values.next()?)?;
        (n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n < usize::MAX as f64)
            .then_some(n as usize)
    };
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
        .field(&call, field)
        .map(|value| context.eval(value, environment))
        .transpose()
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
                let Some(values) = list.list_values() else {
                    return Ok(absent::with_reason(vocabulary::NOT_LIST).into());
                };
                Ok(RuntimeValue::list(std::iter::once(item).chain(values)))
            })
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
                let (Some(left), Some(right)) = (left.list_values(), right.list_values()) else {
                    return Ok(absent::with_reason(vocabulary::NOT_LIST).into());
                };
                Ok(RuntimeValue::list(left.chain(right)))
            })
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
                    .unwrap_or_else(|| absent::with_reason(vocabulary::NOT_LIST).into()))
            })
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
                if list.list_len().is_none() {
                    return Ok(absent::with_reason(vocabulary::NOT_LIST).into());
                }
                let Some(index) = index
                    .as_f64()
                    .filter(|index| *index >= 0.0 && index.fract() == 0.0)
                    .map(|index| index as usize)
                else {
                    return Ok(absent::with_reason(vocabulary::OUT_OF_BOUNDS).into());
                };
                Ok(list
                    .list_get(index)
                    .unwrap_or_else(|| absent::with_reason(vocabulary::OUT_OF_BOUNDS).into()))
            })
            .tracked(),
        )
        .register(
            vocabulary::TAIL,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                let Some(mut values) = list.list_values() else {
                    return Ok(absent::with_reason(vocabulary::NOT_LIST).into());
                };
                if values.next().is_none() {
                    return Ok(absent::with_reason(vocabulary::OUT_OF_BOUNDS).into());
                }
                Ok(RuntimeValue::list(values))
            })
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
                    if result.is_absent() {
                        if result
                            .field(absent::vocabulary::ABSENT)
                            .and_then(|reason| reason.as_cell())
                            != Some(vocabulary::FINISHED)
                        {
                            break Ok(result);
                        }
                        break Ok(RuntimeValue::record([
                            (vocabulary::LIST, RuntimeValue::list(items)),
                            (vocabulary::STATE, state),
                        ]));
                    }
                    let (Some(item), Some(next)) = (
                        result.field(vocabulary::ITEM),
                        result.field(vocabulary::STATE),
                    ) else {
                        break Ok(absent::with_reason(vocabulary::INVALID_STEP).into());
                    };
                    items.push(item);
                    state = next;
                }
            })
            .tracked(),
        )
        .register(
            vocabulary::FOLD,
            ForeignFunction::new(|context, call, environment| {
                let Some(list) = evaluated(context, call, environment, vocabulary::LIST)? else {
                    return Ok(context.missing_runtime_argument(vocabulary::LIST));
                };
                let Some(mut accumulator) =
                    evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_runtime_argument(vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                let Some(values) = list.list_values() else {
                    return Ok(absent::with_reason(vocabulary::NOT_LIST).into());
                };
                for item in values {
                    accumulator = context.call_prepared(
                        &step,
                        [
                            (vocabulary::ACCUMULATOR, accumulator),
                            (vocabulary::ITEM, item),
                        ],
                    )?;
                }
                Ok(accumulator)
            })
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
                        break Ok(
                            if next
                                .field(absent::vocabulary::ABSENT)
                                .and_then(|reason| reason.as_cell())
                                == Some(vocabulary::FINISHED)
                            {
                                state
                            } else {
                                next
                            },
                        );
                    }
                    state = next;
                }
            })
            .tracked(),
        )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
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
        (vocabulary::FINISHED, "iteration finished"),
    ] {
        cells.set_value(cell, absent::named_reason(spelling));
    }
    Library::named(
        ID,
        "list",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::grap;

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
                    number::vocabulary::LEFT,
                    Value::list([Value::from(b"a".to_vec())]),
                ),
                (
                    number::vocabulary::RIGHT,
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
            crate::libraries::test_evaluate(&at, |_| None, &functions, 30).result,
            Value::from(b"b".to_vec())
        );
    }

    #[test]
    fn iteration_and_unfold_only_consume_their_finished_reason() {
        let state = vocabulary::STATE;
        for operation in [vocabulary::ITERATE, vocabulary::UNFOLD] {
            let initial = Value::from(vec![7]);
            let failure = ::grap::absent::with_detail(
                gid::new_cell_id(),
                vocabulary::STATE,
                Value::from(vec![8]),
            );
            for result in [failure.clone(), absent::with_reason(vocabulary::FINISHED)] {
                let expression = call(
                    operation,
                    [
                        (vocabulary::INITIAL, initial.clone()),
                        (vocabulary::STEP, ::grap::lambda([state], result.clone())),
                    ],
                );
                let evaluation =
                    crate::libraries::test_evaluate(&expression, |_| None, &functions(), 100);
                assert!(evaluation.completed);
                assert_eq!(
                    evaluation.result,
                    if result == failure {
                        failure.clone()
                    } else if operation == vocabulary::ITERATE {
                        initial.clone()
                    } else {
                        Value::record([
                            (vocabulary::LIST, Value::list([])),
                            (vocabulary::STATE, initial.clone()),
                        ])
                    }
                );
                let exhausted =
                    crate::libraries::test_evaluate(&expression, |_| None, &functions(), 1);
                assert!(!exhausted.completed);
                assert_eq!(
                    absent::reason(&exhausted.result),
                    Some(::grap::absent::FUEL_EXHAUSTED)
                );
            }
        }
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
            ForeignFunction::from_value(move |_, _, _| {
                let call = step_calls.get();
                step_calls.set(call + 1);
                Ok(if call == 3 {
                    absent::with_reason(vocabulary::FINISHED)
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
            crate::libraries::test_evaluate(&iterated, |_| None, &functions, 30).result,
            Value::from(vec![2]),
        );
        assert_eq!(calls.get(), 4);
    }
}
