//! f32 numbers, for libraries whose host boundary is single precision: four
//! little-endian bytes under the f32 tag, drawn as their spelling. Their
//! arithmetic is Grap's numbers library's.

use crate::display::Host;
use crate::libraries::{Library, number};
use ::grap::RuntimeValue;
use gid::{CellId, Value};

pub const ID: CellId = CellId::from_u128(0xf8daecede6e48de724408cfb0e3090f8);

pub mod vocabulary {
    use gid::CellId;

    pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
}

pub fn value(number: f32) -> Value {
    number::le_value(vocabulary::F32, number.to_le_bytes())
}

pub fn read(value: &Value) -> Option<f32> {
    number::le_bytes(value, vocabulary::F32).map(f32::from_le_bytes)
}

pub(crate) fn read_runtime(value: &RuntimeValue) -> Option<f32> {
    number::le_runtime_bytes(value, vocabulary::F32).map(f32::from_le_bytes)
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

pub(crate) fn representation() -> number::Representation<f32> {
    number::Representation {
        name: "f32",
        tag: vocabulary::F32,
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
