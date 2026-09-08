//! The first geometry Grap library: a circle value and a Rust-backed
//! constructor consuming the f64 library's representation.

use crate::libraries::{Library, absent, f64, name};
use gid::{Cells, Value};

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
        ForeignFunction::new(|context, call, environment| {
            let Some(radius) = context.field(call, vocabulary::RADIUS) else {
                return Ok(context.missing_argument(vocabulary::RADIUS));
            };
            let radius = context.eval(radius, environment)?;
            Ok(f64::read(&radius)
                .filter(|radius| radius.is_finite() && *radius >= 0.0)
                .map(value)
                .unwrap_or_else(|| absent::with_reason(vocabulary::INVALID_RADIUS)))
        }),
    )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::CIRCLE, name::record("circle", []));
    cells.set_value(vocabulary::RADIUS, name::record("radius", []));
    cells.set_value(
        vocabulary::INVALID_RADIUS,
        absent::named_reason("invalid radius"),
    );
    Library::named(
        ID,
        "geometry",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn circle_is_a_library_call_over_an_f64() {
        let foreign = functions();
        let expression = grap::call(
            Value::from(vocabulary::CIRCLE),
            [(vocabulary::RADIUS, f64::value(20.0))],
        );
        assert_eq!(
            crate::libraries::test_evaluate(&expression, |_| None, &foreign, 10).result,
            value(20.0)
        );
        assert_eq!(read(&value(20.0)), Some(20.0));

        let enriched = Value::record(
            value(20.0)
                .as_record()
                .unwrap()
                .clone()
                .update(new_cell_id(), Value::from(b"now".to_vec())),
        );
        assert_eq!(read(&enriched), Some(20.0));
        let with_extra = Value::record(enriched.as_record().unwrap().clone().update(
            vocabulary::CIRCLE,
            Value::record([
                (vocabulary::RADIUS, f64::value(20.0)),
                (new_cell_id(), Value::from(b"survey".to_vec())),
            ]),
        ));
        assert_eq!(read(&with_extra), Some(20.0));
    }

    #[test]
    fn invalid_radius_is_a_library_sentinel() {
        let foreign = functions();
        let expression = grap::call(
            Value::from(vocabulary::CIRCLE),
            [(vocabulary::RADIUS, f64::value(-1.0))],
        );
        let evaluation = crate::libraries::test_evaluate(&expression, |_| None, &foreign, 10);
        assert_eq!(
            evaluation.result,
            absent::with_reason(vocabulary::INVALID_RADIUS)
        );
    }
}
