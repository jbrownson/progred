//! u64 numbers: eight little-endian bytes under the u64 tag, drawn as their
//! spelling. Their arithmetic is Grap's numbers library's.

use crate::display::Host;
use crate::libraries::{Library, number};
use ::grap::RuntimeValue;
use gid::{CellId, Value};

pub const ID: CellId = CellId::from_u128(0xb7212cd0aed055a7a2fbe4036b7f3e51);

pub mod vocabulary {
    use gid::CellId;

    pub const U64: CellId = CellId::from_u128(0xfc77282178c2fa31b80aed2b8b6f7888);
}

pub fn value(value: u64) -> Value {
    number::le_value(vocabulary::U64, value.to_le_bytes())
}

pub fn read(value: &Value) -> Option<u64> {
    number::le_bytes(value, vocabulary::U64).map(u64::from_le_bytes)
}

pub(crate) fn read_runtime(value: &RuntimeValue) -> Option<u64> {
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

pub(crate) fn representation() -> number::Representation<u64> {
    number::Representation {
        name: "u64",
        tag: vocabulary::U64,
        encode: value,
        runtime: |number| value(number).into(),
        read: read_runtime,
    }
}

pub fn library<W: Host>() -> Library<W> {
    representation().library(ID)
}

#[cfg(test)]
mod tests;
