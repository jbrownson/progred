//! Small Grap operations over GID lists. Lists stay ordinary values;
//! these functions only provide the operations awkward to express by
//! structural matching alone.

use crate::display::Host;
use crate::libraries::{Library, f64, name};
use ::grap::arguments::Parse;
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

/// An argument that's a list.
struct List(RuntimeValue);

impl List {
    fn len(&self) -> usize {
        self.0.list_len().expect("parsed as a list")
    }

    fn values(&self) -> ::grap::RuntimeListValues<'_> {
        self.0.list_values().expect("parsed as a list")
    }

    fn positions(&self) -> Vec<gid::Position> {
        self.0.list_positions().expect("parsed as a list")
    }
}

impl Parse for List {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        if value.list_len().is_some() {
            Ok(Self(value))
        } else {
            Err(::grap::arguments::not(vocabulary::NOT_LIST, value))
        }
    }
}

fn out_of_bounds() -> RuntimeValue {
    ::grap::absent::value(vocabulary::OUT_OF_BOUNDS).into()
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::PREPEND,
            ForeignFunction::with_arguments(
                [vocabulary::LIST, vocabulary::ITEM],
                |_, (list, item): (List, RuntimeValue)| {
                    let positions = list.positions();
                    let first = gid::position::between(None, positions.first())
                        .expect("there's room before any position");
                    RuntimeValue::positioned_list(
                        std::iter::once((first, item))
                            .chain(positions.into_iter().zip(list.values())),
                    )
                },
            )
            .tracked(),
        )
        .register(
            vocabulary::CONCAT,
            ForeignFunction::with_arguments(
                [
                    crate::libraries::grap::numbers::vocabulary::LEFT,
                    crate::libraries::grap::numbers::vocabulary::RIGHT,
                ],
                |_, (left, right): (List, List)| {
                    RuntimeValue::list(left.values().chain(right.values()))
                },
            )
            .tracked(),
        )
        .register(
            vocabulary::LENGTH,
            ForeignFunction::with_arguments([vocabulary::LIST], |_, (list,): (List,)| {
                RuntimeValue::f64(list.len() as f64)
            })
            .tracked(),
        )
        .register(
            vocabulary::AT,
            ForeignFunction::with_arguments(
                [vocabulary::LIST, vocabulary::INDEX],
                |_, (list, index): (List, ::core::primitive::f64)| {
                    ::grap::f64::to_usize(index)
                        .and_then(|index| list.0.list_get(index))
                        .unwrap_or_else(out_of_bounds)
                },
            )
            .tracked(),
        )
        .register(
            vocabulary::TAIL,
            ForeignFunction::with_arguments([vocabulary::LIST], |_, (list,): (List,)| {
                match list.len() {
                    0 => out_of_bounds(),
                    _ => RuntimeValue::positioned_list(
                        list.positions().into_iter().zip(list.values()).skip(1),
                    ),
                }
            })
            .tracked(),
        )
        .register(
            vocabulary::UNFOLD,
            ForeignFunction::new(|context, call, environment| {
                let Some(mut state) = evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_runtime_argument(call, vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(call, vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                let mut items = Vec::new();
                // A step's item, if it has one, joins the list; a step
                // without a next state is the last.
                loop {
                    let result =
                        context.call_prepared(&step, [(vocabulary::STATE, state.clone())])?;
                    if result.absent_reason().is_some() {
                        break Ok(result);
                    }
                    if result.record_len().is_none() {
                        break Ok(::grap::absent::value(vocabulary::INVALID_STEP).into());
                    }
                    items.extend(result.field(vocabulary::ITEM));
                    match result.field(vocabulary::STATE) {
                        Some(next) => state = next,
                        None => {
                            break Ok(RuntimeValue::record([
                                (vocabulary::LIST, RuntimeValue::list(items)),
                                (vocabulary::STATE, state),
                            ]));
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
                    return Ok(context.missing_runtime_argument(call, vocabulary::LIST));
                };
                let Some(accumulator) = evaluated(context, call, environment, vocabulary::INITIAL)?
                else {
                    return Ok(context.missing_runtime_argument(call, vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(call, vocabulary::STEP));
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
                    return Ok(context.missing_runtime_argument(call, vocabulary::INITIAL));
                };
                let Some(step) = context.field(call, vocabulary::STEP) else {
                    return Ok(context.missing_runtime_argument(call, vocabulary::STEP));
                };
                let step = context.prepare_callable(step, environment)?;
                loop {
                    let next =
                        context.call_prepared(&step, [(vocabulary::STATE, state.clone())])?;
                    match next.absent_reason() {
                        Some(vocabulary::FINISHED) => break Ok(state),
                        Some(_) => break Ok(next),
                        None => state = next,
                    }
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
#[path = "tests/list.rs"]
mod tests;
