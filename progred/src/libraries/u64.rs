//! An open u64 record convention with the stock editable projection.

use crate::display::Host;
use crate::libraries::{Library, number};
use ::grap::RuntimeValue;
use gid::{CellId, Value};

pub const ID: CellId = CellId::from_u128(0xb7212cd0aed055a7a2fbe4036b7f3e51);

pub mod vocabulary {
    use gid::CellId;

    pub const U64: CellId = CellId::from_u128(0xfc77282178c2fa31b80aed2b8b6f7888);
    pub const SUM: CellId = CellId::from_u128(0x2b63a57fb5065d7ae992241b3018fcb5);
    pub const SUBTRACT: CellId = CellId::from_u128(0x74203a2808e4216a6eff871dd39ab313);
    pub const MULTIPLY: CellId = CellId::from_u128(0xeb9073084d92e0ecb2253ff70a27c03e);
    pub const DIVIDE: CellId = CellId::from_u128(0x610171b11dd2d2844119b7f9bc7da17c);
    pub const LESS: CellId = CellId::from_u128(0x425af3c983c925e2eb8becf4389aac58);
    pub const EQUAL: CellId = CellId::from_u128(0xc3931b2321d6c783fee133acaa929738);
    pub const LEFT_NOT_U64: CellId = CellId::from_u128(0x64dd17fd81404f413122ffc00b452e2e);
    pub const RIGHT_NOT_U64: CellId = CellId::from_u128(0x1a13161cd3a0e5fa71455a277839bd32);
    pub const OPERAND_NOT_U64: CellId = CellId::from_u128(0xe516273ed0eb1e3a6acd87dabb878383);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0x64082046ad013b14b42773624d8453f1);
    pub const OVERFLOW: CellId = CellId::from_u128(0x184f82a04d9cb7609238b2dd05ad5cd0);
    pub const DIVISION_BY_ZERO: CellId = CellId::from_u128(0x7eef118f55e12f09c8f7324c0fdd765d);
}

pub fn value(value: u64) -> Value {
    number::le_value(vocabulary::U64, value.to_le_bytes())
}

pub fn read(value: &Value) -> Option<u64> {
    number::le_bytes(value, vocabulary::U64).map(u64::from_le_bytes)
}

fn read_runtime(value: &RuntimeValue) -> Option<u64> {
    number::le_runtime_bytes(value, vocabulary::U64).map(u64::from_le_bytes)
}

impl number::Scrubbable for u64 {
    fn magnitude(self) -> f64 {
        self as f64
    }

    fn minimum_precision() -> f64 {
        1.0
    }

    fn scrubbable(self) -> bool {
        true
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        let step = (precision.round() as u64).max(1);
        let lower = start / step * step;
        let steps = ((start % step) as f64 + offset) / step as f64;
        (lower as i128)
            .saturating_add((steps.round() as i128).saturating_mul(step as i128))
            .clamp(0, u64::MAX as i128) as u64
    }

    fn spelling(self, _: f64) -> String {
        self.to_string()
    }
}

pub(crate) fn convention() -> number::Convention<u64> {
    number::Convention {
        name: "u64",
        tag: vocabulary::U64,
        left_not: vocabulary::LEFT_NOT_U64,
        right_not: vocabulary::RIGHT_NOT_U64,
        operand_not: vocabulary::OPERAND_NOT_U64,
        invalid_input: vocabulary::INVALID_INPUT,
        encode: value,
        runtime: |number| value(number).into(),
        read: read_runtime,
        eval: |context, expression, environment| {
            Ok(read_runtime(&context.eval(expression, environment)?))
        },
    }
}

/// Unsigned arithmetic has no representable overflow or division by
/// zero, so those outcomes are absents rather than wrapped bits.
fn parts() -> number::Parts {
    use number::Operation::{Checked, Comparison};
    let mut parts = convention().parts([
        (
            vocabulary::SUM,
            "+",
            Checked(u64::checked_add, vocabulary::OVERFLOW),
        ),
        (
            vocabulary::SUBTRACT,
            "-",
            Checked(u64::checked_sub, vocabulary::OVERFLOW),
        ),
        (
            vocabulary::MULTIPLY,
            "*",
            Checked(u64::checked_mul, vocabulary::OVERFLOW),
        ),
        (
            vocabulary::DIVIDE,
            "/",
            Checked(u64::checked_div, vocabulary::DIVISION_BY_ZERO),
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
    ]);
    parts.names.extend([
        (vocabulary::OVERFLOW, "u64 overflow"),
        (vocabulary::DIVISION_BY_ZERO, "division by zero"),
    ]);
    parts
}

#[cfg(test)]
fn functions() -> ::grap::ForeignFunctions {
    parts().functions
}

pub fn library<W: Host>() -> Library<W> {
    convention().library(ID, parts(), std::iter::empty())
}

#[cfg(test)]
mod tests;
