//! f64 numbers: eight little-endian bytes under the f64 tag, drawn as their
//! spelling. Grap holds them natively, with their encoding, and their arithmetic
//! is Grap's numbers library's.

use crate::display::Host;
use crate::libraries::{Library, name, number};
use ::grap::RuntimeValue;
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x1fdb573a2c56a7063546c195318214bc);

pub mod vocabulary {
    use gid::CellId;

    pub use ::grap::f64::F64;
    pub const PI: CellId = CellId::from_u128(0x9cd591f37312e563f52b7374a6cef5c0);
}

// The encoding itself lives in the evaluator, which privileges it as an
// accelerator; this library remains its owner in vocabulary and projection.
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

pub(crate) fn representation() -> number::Representation<f64> {
    number::Representation {
        name: "f64",
        tag: vocabulary::F64,
        encode: value,
        runtime: RuntimeValue::f64,
        read: RuntimeValue::as_f64,
    }
}

pub fn library<W: Host>() -> Library<W> {
    representation().library(ID).with_values([(
        vocabulary::PI,
        name::record(
            "π",
            value(std::f64::consts::PI).as_record().unwrap().clone(),
        ),
    )])
}

#[cfg(test)]
#[path = "tests/f64.rs"]
mod tests;
