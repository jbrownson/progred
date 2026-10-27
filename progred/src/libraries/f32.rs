//! f32 numbers, for libraries whose host boundary is single precision: four
//! little-endian bytes under the f32 tag, drawn as their spelling. Grap holds
//! them natively, with their encoding, and their arithmetic is Grap's numbers
//! library's.

use crate::display::Host;
use crate::libraries::{Library, number};
use ::grap::RuntimeValue;
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0xf8daecede6e48de724408cfb0e3090f8);

pub mod vocabulary {
    pub use ::grap::f32::F32;
}

pub use ::grap::f32::{read, value};

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

pub(crate) fn representation() -> number::Representation<f32> {
    number::Representation {
        name: "f32",
        tag: vocabulary::F32,
        encode: value,
        runtime: RuntimeValue::f32,
        read: RuntimeValue::as_f32,
    }
}

pub fn library<W: Host>() -> Library<W> {
    representation().library(ID)
}

#[cfg(test)]
#[path = "tests/f32.rs"]
mod tests;
