//! The first geometry Grap library: a circle value and a Rust-backed
//! constructor consuming the f64 library's representation.

use grap::ForeignFunctions;
use progred_graph::{Cells, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    pub const CIRCLE: CellId = CellId::from_u128(0xeba2ca3d6a0fba957bd96bfb138c7a5b);
    pub const RADIUS: CellId = CellId::from_u128(0xe2321b78d65f87918c64b7875408051a);
    pub const INVALID_RADIUS: CellId = CellId::from_u128(0x415a1c171b38ed09254c7ce7a11bcaf8);
}

pub fn value(radius: f64) -> Value {
    Value::record([(
        vocabulary::CIRCLE,
        Value::record([(vocabulary::RADIUS, grap_f64::value(radius))]),
    )])
}

pub fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    let radius = fields
        .get(&vocabulary::CIRCLE)
        .and_then(Value::as_record)
        .and_then(|circle| circle.get(&vocabulary::RADIUS))
        .and_then(grap_f64::read)?;
    (radius.is_finite() && radius >= 0.0).then_some(radius)
}

pub fn functions() -> ForeignFunctions {
    let mut foreign = ForeignFunctions::new();
    foreign
        .register(
            vocabulary::CIRCLE,
            [vocabulary::RADIUS],
            |evaluate, arguments, environment| {
                let radius = match arguments {
                    [radius] => evaluate(radius, environment)?,
                    _ => unreachable!("Grap checks foreign arity before calling"),
                };
                Ok(grap_f64::read(&radius)
                    .filter(|radius| radius.is_finite() && *radius >= 0.0)
                    .map(value)
                    .unwrap_or_else(|| Value::from(vocabulary::INVALID_RADIUS)))
            },
        )
        .expect("geometry function cells are distinct");
    foreign
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::CIRCLE, progred_name::record("circle", []));
    cells.set_value(vocabulary::RADIUS, progred_name::record("radius", []));
    cells.set_value(
        vocabulary::INVALID_RADIUS,
        grap_absent::named("invalid radius"),
    );
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    #[test]
    fn circle_is_a_library_call_over_an_f64() {
        let foreign = functions();
        let expression = grap::call(
            Value::from(vocabulary::CIRCLE),
            [(vocabulary::RADIUS, grap_f64::value(20.0))],
        );
        assert_eq!(
            grap::evaluate(&expression, |_| None, &foreign, 10).result,
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
                (vocabulary::RADIUS, grap_f64::value(20.0)),
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
            [(vocabulary::RADIUS, grap_f64::value(-1.0))],
        );
        let evaluation = grap::evaluate(&expression, |_| None, &foreign, 10);
        assert_eq!(
            evaluation.result,
            Value::from(vocabulary::INVALID_RADIUS)
        );
        assert!(grap_absent::is_absent(
            library().value(vocabulary::INVALID_RADIUS).unwrap()
        ));
    }
}
