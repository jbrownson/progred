//! A sequence is a zero-argument callable returning a step, `{item, next}`,
//! where `next` is its successor callable. A step without `next` is the last,
//! and one without `item` yields nothing, so `{}` ends a sequence. Calling the
//! same pure sequence again does not advance it.

use crate::display::Host;
use crate::libraries::{Library, f64, name};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use gid::Value;

pub const ID: gid::CellId = gid::CellId::from_u128(0x0ad8124ba821acd5fbf2c868371e1492);

pub mod vocabulary {
    pub use crate::libraries::list::vocabulary::{INDEX, ITEM};
    use gid::CellId;
    pub const RANGE: CellId = CellId::from_u128(0x846f3beb349626d4078ab09c988884e3);
    pub const RANGE_STEP: CellId = CellId::from_u128(0x7284caac9215cb0b1186d70347bf6f67);
    pub const FOR_EACH: CellId = CellId::from_u128(0x58a4ac72abe43dd18117ff17f66e157a);
    pub const COLLECT: CellId = CellId::from_u128(0xcf28a3cb9d968c355d284ee0c01fb689);
    pub const ITEMS: CellId = CellId::from_u128(0xf4979d6fbf8752063608f7046c075004);
    pub const ACTION: CellId = CellId::from_u128(0x2669abe847620523793fe4629f5870b8);
    pub const COUNT: CellId = CellId::from_u128(0xce24de3241e9827a6eb9bc8bbdcdefce);
    pub const NEXT: CellId = CellId::from_u128(0x58ccdbbff1ef6cd7967095f11af2648d);
    pub const INVALID: CellId = CellId::from_u128(0xd8c0f1238f83fa01a271eda2c1545fd4);
}
use vocabulary::*;

fn invalid() -> RuntimeValue {
    ::grap::absent::value(INVALID).into()
}

fn evaluated(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: gid::CellId,
) -> Result<RuntimeValue, Halt> {
    match context.field(call, field) {
        Some(expression) => context.eval(expression, environment),
        None => Ok(context.missing_runtime_argument(field)),
    }
}

/// A count or an index a range can step through exactly: whole, not
/// negative, and at most 2^53, past which f64 integers have gaps.
fn exact(number: f64) -> bool {
    number >= 0.0 && number.fract() == 0.0 && number <= 9_007_199_254_740_992.0
}

fn range(context: &mut Context, (count,): (::core::primitive::f64,)) -> RuntimeValue {
    if exact(count) {
        range_tail(context, count, 0.0)
    } else {
        invalid()
    }
}

fn range_tail(context: &mut Context, count: f64, index: f64) -> RuntimeValue {
    let body = ::grap::call(
        Value::from(RANGE_STEP),
        [(COUNT, f64::value(count)), (INDEX, f64::value(index))],
    );
    context.closure_value([], body, &context.empty_environment())
}

fn range_step(
    context: &mut Context,
    (count, index): (::core::primitive::f64, ::core::primitive::f64),
) -> RuntimeValue {
    if !(exact(count) && exact(index) && index <= count) {
        invalid()
    } else if index == count {
        RuntimeValue::record([])
    } else {
        RuntimeValue::record([
            (ITEM, RuntimeValue::f64(index)),
            (NEXT, range_tail(context, count, index + 1.0)),
        ])
    }
}

fn consume(
    context: &mut Context,
    call: &Expression,
    env: &Environment,
    collect: bool,
) -> Result<RuntimeValue, Halt> {
    let sequence = evaluated(context, call, env, ITEMS)?;
    if sequence.absent_reason().is_some() {
        Ok(sequence)
    } else {
        let mut sequence = context.prepare_runtime_callable(sequence, env);
        let action = if collect {
            None
        } else {
            let Some(expression) = context.field(call, ACTION) else {
                return Ok(context.missing_runtime_argument(ACTION));
            };
            Some(context.prepare_callable(expression, env)?)
        };
        let mut collected = Vec::new();
        loop {
            let step = context.call_prepared(&sequence, [])?;
            if step.absent_reason().is_some() {
                break Ok(step);
            }
            if step.record_len().is_none() {
                break Ok(invalid());
            }
            if let Some(item) = step.field(ITEM) {
                if let Some(action) = &action {
                    let result = context.call_prepared(action, [(ITEM, item)])?;
                    if result.absent_reason().is_some() {
                        break Ok(result);
                    }
                } else {
                    collected.push(item);
                }
            }
            match step.field(NEXT) {
                Some(next) => sequence = context.prepare_runtime_callable(next, env),
                None => {
                    break Ok(if collect {
                        RuntimeValue::list(collected)
                    } else {
                        RuntimeValue::record([])
                    });
                }
            }
        }
    }
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            RANGE,
            ForeignFunction::with_arguments([COUNT], range).tracked(),
        )
        .register(
            RANGE_STEP,
            ForeignFunction::with_arguments([COUNT, INDEX], range_step).tracked(),
        )
        .register(
            FOR_EACH,
            ForeignFunction::new(|cx, call, env| consume(cx, call, env, false))
                .parameters([ITEMS, ACTION])
                .tracked(),
        )
        .register(
            COLLECT,
            ForeignFunction::new(|cx, call, env| consume(cx, call, env, true))
                .parameters([ITEMS])
                .tracked(),
        )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "sequence",
        [
            (RANGE, "range"),
            (RANGE_STEP, "range step"),
            (FOR_EACH, "for each"),
            (COLLECT, "collect"),
            (ITEMS, "items"),
            (ACTION, "action"),
            (COUNT, "count"),
            (INDEX, "index"),
            (ITEM, "item"),
            (NEXT, "next"),
            (INVALID, "invalid sequence"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
