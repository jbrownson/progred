//! Hex spelling and editing for GID blobs.

use crate::display::TextFamily;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, line_edit, name};
use gid::Value;

pub const ID: gid::CellId = gid::CellId::from_u128(0x4ab5da466a7c5f1202f5ef862f5ff915);

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
    name::named_library(ID, "blob", crate::libraries::Definitions::default())
        .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests;
