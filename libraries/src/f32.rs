//! An open f32 convention used by libraries whose host boundary is
//! single precision. It remains ordinary GID data and ordinary Grap
//! library behavior.

use crate::{Library, line_edit, name, number};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xf8daecede6e48de724408cfb0e3090f8);
use grap_runtime::{ForeignFunction, ForeignFunctions};
use progred_display::{Layout, ProjectionInput, overlay_value};

pub mod vocabulary {
    use gid::CellId;

    pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
    pub const UPDATE: CellId = CellId::from_u128(0x9c34c242d73e090cbd62de1242ad74ae);
}

pub fn value(number: f32) -> Value {
    Value::record([(vocabulary::F32, Value::from(number.to_le_bytes().to_vec()))])
}

pub fn read(value: &Value) -> Option<f32> {
    value
        .as_record()?
        .get(&vocabulary::F32)?
        .as_blob()?
        .try_into()
        .ok()
        .map(f32::from_le_bytes)
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

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(number::layout(
        input,
        read(input.value)?,
        vocabulary::UPDATE,
        value,
    ))
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::UPDATE,
        ForeignFunction::new(|context, call, environment| {
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context
                .field(call, line_edit::vocabulary::CURRENT)
                .map(|current| context.eval(current, environment))
                .transpose()?;
            let input = context.eval(input, environment)?;
            Ok(
                match crate::text::read(&input).and_then(|text| text.trim().parse().ok()) {
                    Some(number) => current
                        .as_ref()
                        .map(|current| overlay_value(current, value(number)))
                        .unwrap_or_else(|| value(number)),
                    None => crate::absent::value(),
                },
            )
        }),
    )
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::F32, name::record("f32", []));
    cells.set_value(vocabulary::UPDATE, name::record("f32 update", []));
    Library::named(
        "f32",
        crate::Definitions::from_parts(cells, functions()),
        vec![progred_display::partial(display::<World, Hover>)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn representation_is_open_library_data() {
        assert_eq!(read(&value(1.25)), Some(1.25));
        assert_eq!(read(&Value::from(1.25_f32.to_le_bytes().to_vec())), None);

        let metadata = new_cell_id();
        let enriched = Value::record(
            value(1.25)
                .as_record()
                .unwrap()
                .clone()
                .update(metadata, Value::from(vec![1])),
        );
        assert_eq!(read(&enriched), Some(1.25));
    }

    #[test]
    fn spelling_round_trips_through_the_update_function() {
        let updated = crate::test_evaluate(
            &grap_runtime::call(
                grap_runtime::ffi(vocabulary::UPDATE),
                [(line_edit::vocabulary::INPUT, crate::text::value("3.5"))],
            ),
            |_| None,
            &functions(),
            20,
        );

        assert!(updated.diagnostics.is_empty());
        assert_eq!(read(&updated.result), Some(3.5));
    }
}
