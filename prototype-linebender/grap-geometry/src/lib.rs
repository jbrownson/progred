//! The first geometry Grap library: a circle value and a Rust-backed
//! constructor consuming the f64 library's representation.

use grap::{ForeignFunctions, RegistrationError};
use progred_graph::{Cells, Label, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    pub const CIRCLE: CellId = CellId::from_u128(0xf546a977c8531e0edffa7e524b393b9f);
    pub const RADIUS: CellId = CellId::from_u128(0x21025a64620233d9b9c28abe95f06224);
    pub const INVALID_RADIUS: CellId = CellId::from_u128(0xe0c00fc5967ea1cc78f45988486793a7);
}

pub fn value(radius: f64) -> Value {
    Value::record([(
        Label::from(vocabulary::CIRCLE),
        Value::record([(Label::from(vocabulary::RADIUS), grap_f64::value(radius))]),
    )])
}

pub fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    let radius = fields
        .get(&Label::from(vocabulary::CIRCLE))
        .and_then(Value::as_record)
        .and_then(|circle| circle.get(&Label::from(vocabulary::RADIUS)))
        .and_then(grap_f64::read)?;
    (radius.is_finite() && radius >= 0.0).then_some(radius)
}

pub fn install(foreign: &mut ForeignFunctions) -> Result<(), RegistrationError> {
    foreign.register(
        vocabulary::CIRCLE,
        [vocabulary::RADIUS],
        |args| match args {
            [radius] => match grap_f64::read(radius) {
                Some(radius) if radius.is_finite() && radius >= 0.0 => value(radius),
                _ => Value::from(vocabulary::INVALID_RADIUS),
            },
            _ => unreachable!("Grap checks foreign arity before calling"),
        },
    )
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::CIRCLE, progred_name::value("circle"));
    cells.set_value(vocabulary::RADIUS, progred_name::value("radius"));
    cells.set_value(
        vocabulary::INVALID_RADIUS,
        grap_error::named("invalid radius"),
    );
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    #[test]
    fn circle_is_a_library_call_over_an_f64() {
        let mut foreign = ForeignFunctions::new();
        install(&mut foreign).unwrap();
        let expression = Value::record([
            (
                Label::from(grap::vocabulary::FUNCTION),
                Value::from(vocabulary::CIRCLE),
            ),
            (Label::from(vocabulary::RADIUS), grap_f64::value(20.0)),
        ]);
        assert_eq!(
            grap::evaluate(&expression, |_| None, &foreign, 10).result,
            Ok(value(20.0))
        );
        assert_eq!(read(&value(20.0)), Some(20.0));

        let enriched = Value::record(
            value(20.0)
                .as_record()
                .unwrap()
                .clone()
                .update(Label::from(new_cell_id()), Value::from(b"now".to_vec())),
        );
        assert_eq!(read(&enriched), Some(20.0));
        let with_extra = Value::record(enriched.as_record().unwrap().clone().update(
            Label::from(vocabulary::CIRCLE),
            Value::record([
                (Label::from(vocabulary::RADIUS), grap_f64::value(20.0)),
                (Label::from(new_cell_id()), Value::from(b"survey".to_vec())),
            ]),
        ));
        assert_eq!(read(&with_extra), Some(20.0));
    }

    #[test]
    fn invalid_radius_is_a_library_sentinel() {
        let mut foreign = ForeignFunctions::new();
        install(&mut foreign).unwrap();
        let expression = Value::record([
            (
                Label::from(grap::vocabulary::FUNCTION),
                Value::from(vocabulary::CIRCLE),
            ),
            (Label::from(vocabulary::RADIUS), grap_f64::value(-1.0)),
        ]);
        let evaluation = grap::evaluate(&expression, |_| None, &foreign, 10);
        assert_eq!(
            evaluation.result,
            Ok(Value::from(vocabulary::INVALID_RADIUS))
        );
        assert!(grap_error::is_error(
            library().value(vocabulary::INVALID_RADIUS).unwrap()
        ));
    }
}
