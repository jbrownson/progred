//! A sequence is a zero-argument callable returning a step, `{item, next}`,
//! where `next` is its successor callable. A step without `next` is the last,
//! and one without `item` yields nothing, so `{}` ends a sequence. Calling the
//! same pure sequence again does not advance it.

use crate::display::Host;
use crate::libraries::{Library, f64, name};
use ::grap::{
    Context, Early, Environment, Expression, ForeignFunction, ForeignFunctions, PreparedCallable,
    RuntimeValue,
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

/// The sequence a call's `items` evaluates to, ready to step; or, early,
/// why there's none.
fn items(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<PreparedCallable, Early> {
    let sequence = context.eval_argument(call, ITEMS, environment)?.present()?;
    Ok(context.prepare_runtime_callable(sequence, environment))
}

/// Steps through `sequence` to its end, handing each step's item to `item`;
/// an absent step, or one that isn't a record, is the early answer.
fn each(
    context: &mut Context,
    mut sequence: PreparedCallable,
    environment: &Environment,
    mut item: impl FnMut(&mut Context, RuntimeValue) -> Result<(), Early>,
) -> Result<(), Early> {
    loop {
        let step = context.call_prepared(&sequence, [])?.present()?;
        if step.record_len().is_none() {
            break Err(invalid().into());
        }
        if let Some(value) = step.field(ITEM) {
            item(context, value)?;
        }
        match step.field(NEXT) {
            Some(next) => sequence = context.prepare_runtime_callable(next, environment),
            None => break Ok(()),
        }
    }
}

/// `for each {items, action}`: the action applied to each item, stopping at
/// the first absent result, which is the answer.
fn for_each(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Early> {
    let sequence = items(context, call, environment)?;
    let action = context.prepare_callable(call.argument(ACTION)?, environment)?;
    each(context, sequence, environment, |context, item| {
        context.call_prepared(&action, [(ITEM, item)])?.present()?;
        Ok(())
    })?;
    Ok(RuntimeValue::record([]))
}

/// `collect {items}`: the sequence's items as a list.
fn collect(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Early> {
    let sequence = items(context, call, environment)?;
    let mut collected = Vec::new();
    each(context, sequence, environment, |_, item| {
        collected.push(item);
        Ok(())
    })?;
    Ok(RuntimeValue::list(collected))
}

fn functions() -> ForeignFunctions<'static> {
    ForeignFunctions::default()
        .register(
            RANGE,
            ForeignFunction::with_arguments([COUNT], range).cacheable(),
        )
        .register(
            RANGE_STEP,
            ForeignFunction::with_arguments([COUNT, INDEX], range_step).cacheable(),
        )
        .register(
            FOR_EACH,
            ForeignFunction::early(for_each)
                .parameters([ITEMS, ACTION])
                .cacheable(),
        )
        .register(
            COLLECT,
            ForeignFunction::early(collect)
                .parameters([ITEMS])
                .cacheable(),
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
#[path = "tests/sequence.rs"]
mod tests;
