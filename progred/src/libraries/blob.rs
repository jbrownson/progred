//! Hex spelling and editing for GID blobs.

use crate::display::TextFamily;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, line_edit, name, text};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x4ab5da466a7c5f1202f5ef862f5ff915);

pub mod vocabulary {
    pub const UPDATE: gid::CellId = gid::CellId::from_u128(0xb691acb5f895285755e9fc74d6da09e4);
    pub const INVALID_INPUT: gid::CellId =
        gid::CellId::from_u128(0xce1d043c5c2b81b1e5b9f8bc35bf54fb);
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::UPDATE,
        ForeignFunction::from_value(|context, call, environment| {
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let input = context.eval_to_value(input, environment)?;
            Ok(text::read(&input)
                .and_then(|text| edit(text, None))
                .unwrap_or_else(|| {
                    ::grap::absent::with_detail(
                        vocabulary::INVALID_INPUT,
                        line_edit::vocabulary::INPUT,
                        input.clone(),
                    )
                }))
        })
        .parameters([line_edit::vocabulary::INPUT])
        .tracked(),
    )
}

pub fn edit(spelling: &str, _: Option<&Value>) -> Option<Value> {
    gid::read_hex(spelling.trim()).map(Value::from)
}

pub fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    Some(line_edit::layout_with_family(
        gid::hex_string(input.value?.as_blob()?),
        line_edit::native(edit),
        "0x",
        "",
        TextFamily::Monospace,
    ))
}

pub fn library<W: Host>() -> Library<W> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::UPDATE, name::record("blob update", []));
    cells.set_value(
        vocabulary::INVALID_INPUT,
        name::record("invalid hex input", []),
    );
    name::named_library(
        ID,
        "blob",
        crate::libraries::Definitions::from_parts(cells, functions()),
    )
    .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests;
