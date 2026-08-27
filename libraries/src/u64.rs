//! An open u64 record convention with the stock editable projection.

use crate::{Library, line_edit, name};
use gid::{Cells, Value};
use grap_runtime::{ForeignFunction, ForeignFunctions};
use progred_display::{Layout, ProjectionInput, overlay_value};

pub mod vocabulary {
    use gid::CellId;

    pub const U64: CellId = CellId::from_u128(0xfc77282178c2fa31b80aed2b8b6f7888);
    pub const UPDATE: CellId = CellId::from_u128(0xd02b945cdf6210536bdf4d0fcc895f05);
}

pub fn value(value: u64) -> Value {
    Value::record([(vocabulary::U64, Value::from(value.to_le_bytes().to_vec()))])
}

pub fn read(value: &Value) -> Option<u64> {
    value
        .as_record()?
        .get(&vocabulary::U64)
        .and_then(Value::as_blob)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes)
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(line_edit::layout(
        read(input.value)?.to_string(),
        grap_runtime::ffi(vocabulary::UPDATE),
        "",
        "",
    ))
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
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
                .and_then(|text| text.trim().parse::<u64>().ok())
                .map(|number| overlay_value(&current, value(number)))
                .unwrap_or_else(crate::absent::value))
        }),
    )
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::U64, name::record("u64", []));
    cells.set_value(vocabulary::UPDATE, name::record("u64 update", []));
    Library {
        cells,
        functions: functions(),
        projections: vec![display::<World, Hover>],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use grap_runtime as grap;

    #[test]
    fn representation_is_open_library_data() {
        let extra = new_cell_id();
        let current = Value::record(
            value(u64::MAX)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"metadata".to_vec())),
        );
        let update = |input: &str| {
            grap::evaluate(
                &grap::call(
                    grap::ffi(vocabulary::UPDATE),
                    [
                        (line_edit::vocabulary::CURRENT, current.clone()),
                        (line_edit::vocabulary::INPUT, crate::text::value(input)),
                    ],
                ),
                |_| None,
                &functions(),
                100,
            )
            .result
        };

        assert_eq!(read(&current), Some(u64::MAX));
        assert_eq!(
            update("42"),
            Value::record(
                value(42)
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(b"metadata".to_vec())),
            )
        );
        assert!(crate::absent::is_absent(&update("-1")));
    }
}
