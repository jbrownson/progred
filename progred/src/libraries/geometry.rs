//! The first geometry Grap library: a circle value and a Rust-backed
//! constructor consuming the f64 library's representation.

use crate::display::Host;
use crate::libraries::{Library, f64, name};
use gid::Value;

pub const ID: gid::CellId = gid::CellId::from_u128(0xac27c33e44d2df4f3d1753bcebd2cfe5);
#[cfg(test)]
use ::grap;
use ::grap::{ForeignFunction, ForeignFunctions};

pub mod vocabulary {
    use gid::CellId;

    pub const CIRCLE: CellId = CellId::from_u128(0xeba2ca3d6a0fba957bd96bfb138c7a5b);
    pub const RADIUS: CellId = CellId::from_u128(0xe2321b78d65f87918c64b7875408051a);
    pub const INVALID_RADIUS: CellId = CellId::from_u128(0x415a1c171b38ed09254c7ce7a11bcaf8);
}

pub fn value(radius: f64) -> Value {
    Value::record([(
        vocabulary::CIRCLE,
        Value::record([(vocabulary::RADIUS, f64::value(radius))]),
    )])
}

#[cfg(test)]
pub fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    let radius = fields
        .get(&vocabulary::CIRCLE)
        .and_then(Value::as_record)
        .and_then(|circle| circle.get(&vocabulary::RADIUS))
        .and_then(f64::read)?;
    (radius.is_finite() && radius >= 0.0).then_some(radius)
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::CIRCLE,
        ForeignFunction::from_value(|context, call, environment| {
            let Some(radius) = context.field(call, vocabulary::RADIUS) else {
                return Ok(context.missing_argument(vocabulary::RADIUS));
            };
            let radius = context.eval_to_value(radius, environment)?;
            Ok(f64::read(&radius)
                .filter(|radius| radius.is_finite() && *radius >= 0.0)
                .map(value)
                .unwrap_or_else(|| ::grap::absent::value(vocabulary::INVALID_RADIUS)))
        })
        .parameters([vocabulary::RADIUS])
        .tracked(),
    )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "geometry",
        [
            (vocabulary::CIRCLE, "circle"),
            (vocabulary::RADIUS, "radius"),
            (vocabulary::INVALID_RADIUS, "invalid radius"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
