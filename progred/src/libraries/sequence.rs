//! A sequence is a zero-argument callable returning `{item, next}`, where
//! `next` is its successor callable, or the list library's finished absent.
//! Calling the same pure sequence again does not advance it.

use crate::libraries::{Definitions, Library, absent, f64, name};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x0ad8124ba821acd5fbf2c868371e1492);

pub mod vocabulary {
    pub use crate::libraries::list::vocabulary::{FINISHED, INDEX, ITEM};
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
    absent::with_reason(INVALID).into()
}

fn evaluated(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    field: gid::CellId,
) -> Result<RuntimeValue, Halt> {
    match context.field(call, field) {
        Some(expression) => context.eval_runtime(expression, environment),
        None => Ok(context.missing_runtime_argument(field)),
    }
}

fn range(context: &mut Context, call: Expression, env: &Environment) -> Result<RuntimeValue, Halt> {
    let count = evaluated(context, call, env, COUNT)?;
    if count.is_absent() {
        return Ok(count);
    }
    let Some(count) = count.as_f64().filter(|n| {
        n.is_finite() && *n >= 0.0 && n.fract() == 0.0 && *n <= 9_007_199_254_740_992.0
    }) else {
        return Ok(invalid());
    };
    Ok(range_tail(context, count, 0.0))
}

fn range_tail(context: &mut Context, count: f64, index: f64) -> RuntimeValue {
    let body = ::grap::call(
        Value::from(RANGE_STEP),
        [(COUNT, f64::value(count)), (INDEX, f64::value(index))],
    );
    let empty = context.environment(&Value::record([])).unwrap();
    context.closure([], body, &empty)
}

fn range_step(
    context: &mut Context,
    call: Expression,
    env: &Environment,
) -> Result<RuntimeValue, Halt> {
    let count = evaluated(context, call, env, COUNT)?;
    if count.is_absent() {
        return Ok(count);
    }
    let index = evaluated(context, call, env, INDEX)?;
    if index.is_absent() {
        return Ok(index);
    }
    let (Some(count), Some(index)) = (count.as_f64(), index.as_f64()) else {
        return Ok(invalid());
    };
    if !count.is_finite()
        || count < 0.0
        || count.fract() != 0.0
        || count > 9_007_199_254_740_992.0
        || !index.is_finite()
        || index < 0.0
        || index.fract() != 0.0
        || index > count
    {
        return Ok(invalid());
    }
    Ok(if index == count {
        absent::with_reason(FINISHED).into()
    } else {
        RuntimeValue::record([
            (ITEM, RuntimeValue::f64(index)),
            (NEXT, range_tail(context, count, index + 1.0)),
        ])
    })
}

fn consume(
    context: &mut Context,
    call: Expression,
    env: &Environment,
    collect: bool,
) -> Result<RuntimeValue, Halt> {
    let sequence = evaluated(context, call, env, ITEMS)?;
    if sequence.is_absent() {
        return Ok(sequence);
    }
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
        let result = context.call_prepared_runtime(&sequence, [])?;
        if result.is_absent() {
            if result
                .field(absent::vocabulary::ABSENT)
                .and_then(|reason| reason.as_cell())
                != Some(FINISHED)
            {
                return Ok(result);
            }
            return Ok(if collect {
                RuntimeValue::list(collected)
            } else {
                RuntimeValue::record([])
            });
        }
        let (Some(item), Some(next)) = (result.field(ITEM), result.field(NEXT)) else {
            return Ok(invalid());
        };
        if let Some(action) = &action {
            let result = context.call_prepared_runtime(action, [(ITEM, item)])?;
            if result.is_absent() {
                return Ok(result);
            }
        } else {
            collected.push(item);
        }
        sequence = context.prepare_runtime_callable(next, env);
    }
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(RANGE, ForeignFunction::runtime(range).tracked())
        .register(RANGE_STEP, ForeignFunction::runtime(range_step).tracked())
        .register(
            FOR_EACH,
            ForeignFunction::runtime(|cx, call, env| consume(cx, call, env, false)).tracked(),
        )
        .register(
            COLLECT,
            ForeignFunction::runtime(|cx, call, env| consume(cx, call, env, true)).tracked(),
        )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
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
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    cells.set_value(INVALID, absent::named_reason("invalid sequence"));
    cells.set_value(FINISHED, absent::named_reason("iteration finished"));
    Library::named(
        ID,
        "sequence",
        Definitions::from_parts(cells, functions()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests;
