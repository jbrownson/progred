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
    Some(crate::display::line_edit(crate::display::LineEdit {
        text: gid::hex_string(input.value?.as_blob()?),
        update: line_edit::native(edit),
        prefix: "0x".into(),
        suffix: String::new(),
        family: TextFamily::Monospace,
    }))
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(ID, "blob", [], ::grap::ForeignFunctions::default())
        .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests;
