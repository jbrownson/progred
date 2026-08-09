//! An f64 Grap library. The number representation is ordinary Grap
//! data; arithmetic is supplied to the evaluator as Rust foreign
//! functions.

use grap::{ForeignFunctions, RegistrationError};
use progred_graph::{Cells, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);
    pub const ADD: CellId = CellId::from_u128(0x201af445eb7e2c270bb5ead10b781fc1);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd6f384c439d9d69996d545df422efd79);
    pub const LEFT: CellId = CellId::from_u128(0x764f6afe17ba14e81f5ab61204be0bec);
    pub const RIGHT: CellId = CellId::from_u128(0x4f53ff25390f58472d31a6142644dec2);
    pub const LEFT_NOT_F64: CellId = CellId::from_u128(0x50c0d2fd8fe0325a8e0e41f79ce86eff);
    pub const RIGHT_NOT_F64: CellId = CellId::from_u128(0xcab77cffe8c38745dd8e748ece331409);
}

pub fn value(value: f64) -> Value {
    Value::record([(
        vocabulary::F64,
        Value::from(value.to_le_bytes().to_vec()),
    )])
}

pub fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    fields
        .get(&vocabulary::F64)
        .and_then(Value::as_blob)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(f64::from_le_bytes)
}

pub fn install(foreign: &mut ForeignFunctions) -> Result<(), RegistrationError> {
    foreign.register(
        vocabulary::ADD,
        [vocabulary::LEFT, vocabulary::RIGHT],
        |args| binary(args, |left, right| left + right),
    )?;
    foreign.register(
        vocabulary::MULTIPLY,
        [vocabulary::LEFT, vocabulary::RIGHT],
        |args| binary(args, |left, right| left * right),
    )
}

fn binary(args: &[Value], operation: impl FnOnce(f64, f64) -> f64) -> Value {
    match args {
        [left, right] => match (read(left), read(right)) {
            (Some(left), Some(right)) => value(operation(left, right)),
            (None, _) => Value::from(vocabulary::LEFT_NOT_F64),
            (_, None) => Value::from(vocabulary::RIGHT_NOT_F64),
        },
        _ => unreachable!("Grap checks foreign arity before calling"),
    }
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::F64, "f64"),
        (vocabulary::ADD, "add"),
        (vocabulary::MULTIPLY, "multiply"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
    ] {
        cells.set_value(cell, progred_name::value(name));
    }
    for (cell, name) in [
        (vocabulary::LEFT_NOT_F64, "left is not f64"),
        (vocabulary::RIGHT_NOT_F64, "right is not f64"),
    ] {
        cells.set_value(cell, grap_error::named(name));
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::{CellId, new_cell_id};

    fn call(function: CellId, left: Value, right: Value) -> Value {
        Value::record([
            (
                grap::vocabulary::FUNCTION,
                Value::from(function),
            ),
            (vocabulary::LEFT, left),
            (vocabulary::RIGHT, right),
        ])
    }

    fn foreign() -> ForeignFunctions {
        let mut foreign = ForeignFunctions::new();
        install(&mut foreign).unwrap();
        foreign
    }

    #[test]
    fn representation_is_library_data() {
        assert_eq!(read(&value(2.5)), Some(2.5));
        assert_eq!(read(&Value::from(b"2.5".to_vec())), None);

        let with_extra = Value::record(
            value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(new_cell_id(), Value::from(b"degrees".to_vec())),
        );
        assert_eq!(read(&with_extra), Some(2.5));
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let add = call(vocabulary::ADD, value(2.0), value(3.0));
        let multiply = call(vocabulary::MULTIPLY, add, value(4.0));
        assert_eq!(
            grap::evaluate(&multiply, |_| None, &foreign(), 20).result,
            Ok(value(20.0))
        );
    }

    #[test]
    fn type_failures_are_library_values() {
        let left = call(vocabulary::ADD, Value::from(b"two".to_vec()), value(3.0));
        let right = call(vocabulary::ADD, value(2.0), Value::from(b"three".to_vec()));
        assert_eq!(
            grap::evaluate(&left, |_| None, &foreign(), 10).result,
            Ok(Value::from(vocabulary::LEFT_NOT_F64))
        );
        assert_eq!(
            grap::evaluate(&right, |_| None, &foreign(), 10).result,
            Ok(Value::from(vocabulary::RIGHT_NOT_F64))
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library();
        assert_eq!(
            library.value(vocabulary::F64).and_then(progred_name::read),
            Some("f64")
        );
        assert_eq!(
            library.value(vocabulary::ADD).and_then(progred_name::read),
            Some("add")
        );
        assert_eq!(
            library
                .value(vocabulary::LEFT_NOT_F64)
                .and_then(progred_name::read),
            Some("left is not f64")
        );
        assert_eq!(
            library
                .value(vocabulary::RIGHT_NOT_F64)
                .and_then(progred_name::read),
            Some("right is not f64")
        );
        assert!(grap_error::is_error(
            library.value(vocabulary::LEFT_NOT_F64).unwrap()
        ));
        assert!(grap_error::is_error(
            library.value(vocabulary::RIGHT_NOT_F64).unwrap()
        ));
        assert!(library.value(vocabulary::ADD).is_some());
    }
}
