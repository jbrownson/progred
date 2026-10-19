//! Conversions between number types, belonging to neither type.

use crate::display::Host;
use crate::libraries::{Library, f32, name};
use ::grap::ForeignFunctions;
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x20540168af7321cb256f4e275e9389f9);

pub mod vocabulary {
    use gid::CellId;

    pub const F32_FROM_F64: CellId = CellId::from_u128(0x73bd2225d2091ba893a5570e8e9bacff);
    /// Why a conversion to f32 declines.
    pub const INVALID_F32_INPUT: CellId = CellId::from_u128(0x722482f3e369634464ae68add98480e6);
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::F32_FROM_F64,
        f32::convention().conversion(vocabulary::INVALID_F32_INPUT, |operand| {
            operand.as_f64().map(|number| number as f32)
        }),
    )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "conversion",
        [
            (vocabulary::F32_FROM_F64, "f32 from f64"),
            (vocabulary::INVALID_F32_INPUT, "invalid f32 input"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
