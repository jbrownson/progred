//! An f64 Grap library. The number representation is ordinary Grap
//! data; arithmetic is supplied to the evaluator as Rust foreign
//! functions.

use crate::{Library, absent, layout, line_edit, name, text};
use gid::{Cells, Value};
#[cfg(test)]
use grap_runtime as grap;
use grap_runtime::{Context, Environment, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{Layout, ProjectionInput, overlay_value};

pub mod vocabulary {
    use gid::CellId;

    pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);
    pub const ADD: CellId = CellId::from_u128(0x201af445eb7e2c270bb5ead10b781fc1);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd6f384c439d9d69996d545df422efd79);
    pub const LEFT: CellId = CellId::from_u128(0x764f6afe17ba14e81f5ab61204be0bec);
    pub const RIGHT: CellId = CellId::from_u128(0x4f53ff25390f58472d31a6142644dec2);
    /// The f64 line's write-back rule: parse the typed spelling,
    /// other fields carried; unparseable input declines.
    pub const UPDATE: CellId = CellId::from_u128(0x6b95d2e04c7a1f38b1a08e57d24c96fb);
    pub const LEFT_NOT_F64: CellId = CellId::from_u128(0x50c0d2fd8fe0325a8e0e41f79ce86eff);
    pub const RIGHT_NOT_F64: CellId = CellId::from_u128(0xcab77cffe8c38745dd8e748ece331409);
}

pub fn value(value: f64) -> Value {
    Value::record([(vocabulary::F64, Value::from(value.to_le_bytes().to_vec()))])
}

pub fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    fields
        .get(&vocabulary::F64)
        .and_then(Value::as_blob)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(f64::from_le_bytes)
}

pub fn display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let content = read(input.value)?.to_string();
    let expression = line_edit::call(
        text::value(content),
        grap_runtime::ffi(vocabulary::UPDATE),
        text::value(""),
        text::value(""),
        input.selection.cloned().unwrap_or_else(absent::value),
    );
    let (display, _) = input.env.evaluate(&expression);
    layout::decode(&display, &input.select, &input.hover)
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::UPDATE,
            ForeignFunction::new(|context, call, environment| {
            let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
            };
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
                };
                let current = context.eval(current, environment)?;
                let input = context.eval(input, environment)?;
                Ok(crate::text::read(&input)
                    .and_then(|text| text.trim().parse::<f64>().ok())
                    .map(|number| overlay_value(&current, value(number)))
                    .unwrap_or_else(crate::absent::value))
            }),
        )
        .register(
            vocabulary::ADD,
            ForeignFunction::new(|context, call, environment| {
                binary(context, call, environment, |left, right| left + right)
            }),
        )
        .register(
            vocabulary::MULTIPLY,
            ForeignFunction::new(|context, call, environment| {
                binary(context, call, environment, |left, right| left * right)
            }),
        )
}

fn binary(
    context: &mut Context,
    call: &Value,
    environment: &Environment,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<Value, Halt> {
    let Some(left) = context.field(call, vocabulary::LEFT) else {
        return Ok(context.missing_argument(vocabulary::LEFT));
    };
    let Some(right) = context.field(call, vocabulary::RIGHT) else {
        return Ok(context.missing_argument(vocabulary::RIGHT));
    };
    let left = context.eval(left, environment)?;
    let right = context.eval(right, environment)?;
    Ok(match (read(&left), read(&right)) {
        (Some(left), Some(right)) => value(operation(left, right)),
        (None, _) => Value::from(vocabulary::LEFT_NOT_F64),
        (_, None) => Value::from(vocabulary::RIGHT_NOT_F64),
    })
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::F64, "f64"),
        (vocabulary::UPDATE, "f64 update"),
        (vocabulary::ADD, "add"),
        (vocabulary::MULTIPLY, "multiply"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
    ] {
        cells.set_value(cell, name::record(name, []));
    }
    for (cell, name) in [
        (vocabulary::LEFT_NOT_F64, "left is not f64"),
        (vocabulary::RIGHT_NOT_F64, "right is not f64"),
    ] {
        cells.set_value(cell, absent::named(name));
    }
    Library {
        cells,
        functions: functions(),
        projections: vec![display::<World, Hover>],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{CellId, new_cell_id};

    fn call(function: CellId, left: Value, right: Value) -> Value {
        grap::call(
            Value::from(function),
            [(vocabulary::LEFT, left), (vocabulary::RIGHT, right)],
        )
    }

    #[test]
    fn representation_is_library_data() {
        assert_eq!(read(&value(2.5)), Some(2.5));
        assert_eq!(read(&Value::from(b"2.5".to_vec())), None);

        let extra = new_cell_id();
        let with_extra = Value::record(
            value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"degrees".to_vec())),
        );
        assert_eq!(read(&with_extra), Some(2.5));
        let update = |input: &str| {
            grap::evaluate(
                &grap::call(
                    grap::ffi(vocabulary::UPDATE),
                    [
                        (line_edit::vocabulary::CURRENT, with_extra.clone()),
                        (line_edit::vocabulary::INPUT, crate::text::value(input)),
                    ],
                ),
                |_| None,
                &functions(),
                100,
            )
            .result
        };
        // Unparseable input declines as an absent — the editor drops
        // the write whole.
        assert_eq!(
            crate::isa::read(&update("junk")),
            Some(crate::absent::vocabulary::ABSENT)
        );
        assert_eq!(
            update("3"),
            Value::record(
                value(3.0)
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(b"degrees".to_vec())),
            )
        );
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let add = call(vocabulary::ADD, value(2.0), value(3.0));
        let multiply = call(vocabulary::MULTIPLY, add, value(4.0));
        assert_eq!(
            grap::evaluate(&multiply, |_| None, &functions(), 20).result,
            value(20.0)
        );
    }

    #[test]
    fn type_absences_are_library_values() {
        let left = call(vocabulary::ADD, Value::from(b"two".to_vec()), value(3.0));
        let right = call(vocabulary::ADD, value(2.0), Value::from(b"three".to_vec()));
        assert_eq!(
            grap::evaluate(&left, |_| None, &functions(), 10).result,
            Value::from(vocabulary::LEFT_NOT_F64)
        );
        assert_eq!(
            grap::evaluate(&right, |_| None, &functions(), 10).result,
            Value::from(vocabulary::RIGHT_NOT_F64)
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library::<(), ()>();
        assert_eq!(
            library.cells.value(vocabulary::F64).and_then(name::read),
            Some("f64")
        );
        assert_eq!(
            library.cells.value(vocabulary::ADD).and_then(name::read),
            Some("add")
        );
        assert_eq!(
            library
                .cells
                .value(vocabulary::LEFT_NOT_F64)
                .and_then(name::read),
            Some("left is not f64")
        );
        assert_eq!(
            library
                .cells
                .value(vocabulary::RIGHT_NOT_F64)
                .and_then(name::read),
            Some("right is not f64")
        );
        assert!(absent::is_absent(
            library.cells.value(vocabulary::LEFT_NOT_F64).unwrap()
        ));
        assert!(absent::is_absent(
            library.cells.value(vocabulary::RIGHT_NOT_F64).unwrap()
        ));
        assert!(library.cells.value(vocabulary::ADD).is_some());
    }
}
