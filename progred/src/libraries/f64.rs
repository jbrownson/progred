//! An f64 Grap library. The number representation is ordinary Grap
//! data; arithmetic is supplied to the evaluator as Rust foreign
//! functions.

use crate::display::Host;
use crate::libraries::{Library, absent, name, number};
use gid::CellId;
#[cfg(test)]
use gid::{Step, Value};

pub const ID: CellId = CellId::from_u128(0x1fdb573a2c56a7063546c195318214bc);
#[cfg(test)]
use ::grap;
#[cfg(test)]
use ::grap::vocabulary::FUNCTION;
use ::grap::{Context, Environment, Expression, ForeignFunction, Halt, RuntimeValue};

pub mod vocabulary {
    use gid::CellId;

    #[cfg(test)]
    pub use crate::libraries::number::vocabulary::{LEFT, OPERAND, RIGHT};
    pub use ::grap::f64::F64;
    pub const SUM: CellId = CellId::from_u128(0x201af445eb7e2c270bb5ead10b781fc1);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd6f384c439d9d69996d545df422efd79);
    pub const SUBTRACT: CellId = CellId::from_u128(0x08d1ebc7fd4ce62efec9671f73e9b645);
    pub const DIVIDE: CellId = CellId::from_u128(0xb08dd4c44eeea43ab3c7593ddeedd742);
    pub const SIN: CellId = CellId::from_u128(0x9f62e0f56d92be74f6ae36378d889262);
    pub const COS: CellId = CellId::from_u128(0xfb05b6c9b1565ca4add3732e8ddbe2b4);
    pub const LESS: CellId = CellId::from_u128(0xed44dbf5b4cdf5c952e1ef00f219b655);
    pub const EQUAL: CellId = CellId::from_u128(0x22ab9aa3e7ce4f4f79a7039e1cc23773);
    pub const FLOOR: CellId = CellId::from_u128(0xd007814c5f6a6c38b025605b399473d4);
    pub const MIN: CellId = CellId::from_u128(0x6daed3f67cff3da30767201e5a895f1a);
    pub const MAX: CellId = CellId::from_u128(0x9dfc13c9c7e059da42ea74ef63897ce6);
    pub const CEIL: CellId = CellId::from_u128(0xe1ddb3a620b61117d3873c163f603488);
    pub const HYPOT: CellId = CellId::from_u128(0xfe79fe9c7be4542249fdf604ddf5f5ea);
    pub const IS_FINITE: CellId = CellId::from_u128(0x1ff1fcec05b669eba2314ad52fe8e8f1);
    pub const LERP: CellId = CellId::from_u128(0x432ad7a31ef129e353e251419e690ca1);
    pub const PI: CellId = CellId::from_u128(0x9cd591f37312e563f52b7374a6cef5c0);
    pub const START: CellId = CellId::from_u128(0x4b4fb6349d2fd798e7aafca85a2deca8);
    pub const END: CellId = CellId::from_u128(0x3a816b0af0160bc77ba1948b122b3f29);
    pub const AMOUNT: CellId = CellId::from_u128(0x55e66fc5eb91699cf12833fb0a15d4b6);
    pub const LEFT_NOT_F64: CellId = CellId::from_u128(0x50c0d2fd8fe0325a8e0e41f79ce86eff);
    pub const RIGHT_NOT_F64: CellId = CellId::from_u128(0xcab77cffe8c38745dd8e748ece331409);
    pub const OPERAND_NOT_F64: CellId = CellId::from_u128(0x9c2a4845e1c67df6dd43ac1116e76441);
    pub const START_NOT_F64: CellId = CellId::from_u128(0x809d7bba33afaf8a673846f0c44cd2e6);
    pub const END_NOT_F64: CellId = CellId::from_u128(0x17081dd43c4af46c54408cd125eaf3e2);
    pub const AMOUNT_NOT_F64: CellId = CellId::from_u128(0xe1977104f3574cd37a99ef9083dde01f);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0xa344d4784dae693385d91140b3c030a2);
}

// The convention itself lives in the evaluator, which privileges it
// as an accelerator; this library remains its owner in vocabulary,
// functions, and projections.
pub use ::grap::f64::{read, value};

impl number::Scrubbable for f64 {
    fn magnitude(self) -> f64 {
        self
    }

    fn minimum_precision() -> f64 {
        0.0
    }

    fn scrubbable(self) -> bool {
        self.is_finite()
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        number::rounded(start + offset, precision)
    }

    fn spelling(self, precision: f64) -> String {
        if precision >= 1.0 {
            self.round().to_string()
        } else {
            let decimal_places = (-precision.log10()).round().clamp(0.0, 16.0) as usize;
            format!("{self:.decimal_places$}")
        }
    }
}

pub(crate) fn convention() -> number::Convention<f64> {
    number::Convention {
        name: "f64",
        tag: vocabulary::F64,
        left_not: vocabulary::LEFT_NOT_F64,
        right_not: vocabulary::RIGHT_NOT_F64,
        operand_not: vocabulary::OPERAND_NOT_F64,
        invalid_input: vocabulary::INVALID_INPUT,
        encode: value,
        runtime: RuntimeValue::f64,
        read: RuntimeValue::as_f64,
        eval: |context, expression, environment| context.eval_f64(expression, environment),
    }
}

fn parts() -> number::Parts {
    use number::Operation::{Arithmetic, Comparison, Predicate, Unary};
    let mut parts = convention().parts([
        (vocabulary::SUM, "+", Arithmetic(|left, right| left + right)),
        (
            vocabulary::MULTIPLY,
            "*",
            Arithmetic(|left, right| left * right),
        ),
        (
            vocabulary::SUBTRACT,
            "-",
            Arithmetic(|left, right| left - right),
        ),
        (
            vocabulary::DIVIDE,
            "/",
            Arithmetic(|left, right| left / right),
        ),
        (vocabulary::MIN, "min", Arithmetic(f64::min)),
        (vocabulary::MAX, "max", Arithmetic(f64::max)),
        (vocabulary::HYPOT, "hypot", Arithmetic(f64::hypot)),
        (
            vocabulary::LESS,
            "<",
            Comparison(|left, right| left < right),
        ),
        (
            vocabulary::EQUAL,
            "==",
            Comparison(|left, right| left == right),
        ),
        (vocabulary::SIN, "sin", Unary(f64::sin)),
        (vocabulary::COS, "cos", Unary(f64::cos)),
        (vocabulary::FLOOR, "floor", Unary(f64::floor)),
        (vocabulary::CEIL, "ceil", Unary(f64::ceil)),
        (
            vocabulary::IS_FINITE,
            "is finite",
            Predicate(f64::is_finite),
        ),
    ]);
    parts.functions = parts.functions.register(
        vocabulary::LERP,
        ForeignFunction::new(lerp)
            .parameters([vocabulary::START, vocabulary::END, vocabulary::AMOUNT])
            .tracked(),
    );
    for (cell, name) in [
        (vocabulary::LERP, "lerp"),
        (vocabulary::START, "start"),
        (vocabulary::END, "end"),
        (vocabulary::AMOUNT, "amount"),
    ] {
        parts.cells.set_value(cell, name::record(name, []));
    }
    for (cell, name) in [
        (vocabulary::START_NOT_F64, "start is not f64"),
        (vocabulary::END_NOT_F64, "end is not f64"),
        (vocabulary::AMOUNT_NOT_F64, "amount is not f64"),
    ] {
        parts.cells.set_value(cell, absent::named_reason(name));
    }
    parts.cells.set_value(
        vocabulary::PI,
        value(std::f64::consts::PI).overlaid(name::record("π", [])),
    );
    parts.calls.push(vocabulary::LERP);
    parts
}

#[cfg(test)]
pub fn functions() -> ::grap::ForeignFunctions {
    parts().functions
}

fn lerp(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(start) = context.field(call, vocabulary::START) else {
        return Ok(context.missing_runtime_argument(vocabulary::START));
    };
    let Some(end) = context.field(call, vocabulary::END) else {
        return Ok(context.missing_runtime_argument(vocabulary::END));
    };
    let Some(amount) = context.field(call, vocabulary::AMOUNT) else {
        return Ok(context.missing_runtime_argument(vocabulary::AMOUNT));
    };
    let start = context.eval_f64(start, environment)?;
    let end = context.eval_f64(end, environment)?;
    let amount = context.eval_f64(amount, environment)?;
    Ok(match (start, end, amount) {
        (Some(start), Some(end), Some(amount)) => RuntimeValue::f64(start + (end - start) * amount),
        (None, _, _) => absent::with_reason(vocabulary::START_NOT_F64).into(),
        (_, None, _) => absent::with_reason(vocabulary::END_NOT_F64).into(),
        (_, _, None) => absent::with_reason(vocabulary::AMOUNT_NOT_F64).into(),
    })
}

pub fn library<W: Host>() -> Library<W> {
    convention().library(ID, parts(), std::iter::empty())
}

#[cfg(test)]
mod tests;
