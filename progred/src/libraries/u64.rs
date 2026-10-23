//! u64 numbers: eight little-endian bytes under the u64 tag, drawn as their
//! spelling. Grap holds them natively, with their encoding, and their arithmetic
//! is Grap's numbers library's.

use crate::display::Host;
use crate::libraries::{Library, number};
use ::grap::RuntimeValue;
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0xb7212cd0aed055a7a2fbe4036b7f3e51);

pub mod vocabulary {
    pub use ::grap::u64::U64;
}

pub use ::grap::u64::{read, value};

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
        runtime: RuntimeValue::u64,
        read: RuntimeValue::as_u64,
    }
}

pub fn library<W: Host>() -> Library<W> {
    representation().library(ID)
}

#[cfg(test)]
mod tests;
