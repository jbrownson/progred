//! An open f32 convention used by libraries whose host boundary is
//! single precision. It remains ordinary GID data and ordinary Grap
//! library behavior.

use crate::display::Host;
use crate::libraries::{Library, number};
use gid::{CellId, Value};

pub const ID: CellId = CellId::from_u128(0xf8daecede6e48de724408cfb0e3090f8);

pub mod vocabulary {
    use gid::CellId;

    pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
    pub const SUM: CellId = CellId::from_u128(0x257e5967d826e69d28929cf365667ae1);
    pub const SUBTRACT: CellId = CellId::from_u128(0x5cd7408dab067d8b92c7a1bd9cda7b05);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd4df1fe63a95cda53c19ad7220054c24);
    pub const DIVIDE: CellId = CellId::from_u128(0xfbf1ebc4eeeea21f86af583f6ad85e71);
    pub const LESS: CellId = CellId::from_u128(0x3dde12d1f97ccca65c579ae2703bb07a);
    pub const EQUAL: CellId = CellId::from_u128(0xe0637a60944f8dd9afd2fb4c28214dc3);
    pub const LEFT_NOT_F32: CellId = CellId::from_u128(0x2f7a7dfd96df51008a563a36e075e50b);
    pub const RIGHT_NOT_F32: CellId = CellId::from_u128(0xd4a7b035dc57953dca5c1d20cb591d0d);
    pub const OPERAND_NOT_F32: CellId = CellId::from_u128(0xcc210598ba95d6d2a4fb8baa13e92864);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0x722482f3e369634464ae68add98480e6);
}

pub fn value(number: f32) -> Value {
    Value::record([(vocabulary::F32, Value::from(number.to_le_bytes().to_vec()))])
}

pub fn read(value: &Value) -> Option<f32> {
    value
        .as_record()?
        .get(&vocabulary::F32)?
        .as_blob()?
        .try_into()
        .ok()
        .map(f32::from_le_bytes)
}

impl number::Scrubbable for f32 {
    fn magnitude(self) -> f64 {
        self.into()
    }

    fn minimum_precision() -> f64 {
        0.0
    }

    fn scrubbable(self) -> bool {
        self.is_finite()
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        number::rounded(f64::from(start) + offset, precision) as f32
    }

    fn spelling(self, precision: f64) -> String {
        if precision >= 1.0 {
            self.round().to_string()
        } else {
            let decimal_places = (-precision.log10()).round().clamp(0.0, 8.0) as usize;
            format!("{self:.decimal_places$}")
        }
    }
}

pub(crate) fn convention() -> number::Convention<f32> {
    number::Convention {
        name: "f32",
        tag: vocabulary::F32,
        left_not: vocabulary::LEFT_NOT_F32,
        right_not: vocabulary::RIGHT_NOT_F32,
        operand_not: vocabulary::OPERAND_NOT_F32,
        invalid_input: vocabulary::INVALID_INPUT,
        encode: value,
        runtime: |number| value(number).into(),
        read: |value| {
            Some(f32::from_le_bytes(
                value.field(vocabulary::F32)?.as_blob()?.try_into().ok()?,
            ))
        },
        eval: |context, expression, environment| {
            Ok(read(&context.eval_to_value(expression, environment)?))
        },
    }
}

fn parts() -> number::Parts {
    use number::Operation::{Arithmetic, Comparison};
    convention().parts([
        (vocabulary::SUM, "+", Arithmetic(|left, right| left + right)),
        (
            vocabulary::SUBTRACT,
            "-",
            Arithmetic(|left, right| left - right),
        ),
        (
            vocabulary::MULTIPLY,
            "*",
            Arithmetic(|left, right| left * right),
        ),
        (
            vocabulary::DIVIDE,
            "/",
            Arithmetic(|left, right| left / right),
        ),
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
    ])
}

#[cfg(test)]
pub fn functions() -> ::grap::ForeignFunctions {
    parts().functions
}

pub fn library<W: Host>() -> Library<W> {
    convention().library(ID, parts(), std::iter::empty())
}

#[cfg(test)]
mod tests;
