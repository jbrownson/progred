//! The stock Rust line-edit control shared by atomic-value libraries.
//! A library supplies spelling, affixes, and a conversion callback;
//! the native widget composes Puri text, painting, and handlers directly.

use crate::display::host::{Host, Layout};
use crate::libraries::{Library, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x26b5394bf7beb5dce00140f9e03bc465);
use crate::display::{LineEdit, LineUpdate, TextFamily, line_edit};
use std::rc::Rc;

pub fn native(update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static) -> LineUpdate {
    Rc::new(move |_, spelling, current| update(spelling, current))
}

#[cfg(test)]
pub fn grap(function: Value) -> LineUpdate {
    let function = ::grap::RuntimeValue::from(function);
    Rc::new(move |env, spelling, current| {
        let arguments = std::iter::once((
            vocabulary::INPUT,
            crate::libraries::text::value(spelling).into(),
        ))
        .chain(current.map(|current| (vocabulary::CURRENT, current.into())))
        .collect::<Vec<_>>();
        let result = env.apply_expression_runtime(&function, &arguments);
        (!result.is_absent()).then(|| result.into_value())
    })
}

pub mod vocabulary {
    use gid::CellId;

    /// Arguments passed to an atomic line editor's write-back rule.
    pub const CURRENT: CellId = CellId::from_u128(0x0e6a49d1c78325bfa9231c05e84d67fb);
    pub const INPUT: CellId = CellId::from_u128(0xd58c17f3402b96ea6f0e4a2b91c738d5);
}

pub fn layout<W: Host>(
    text: impl Into<String>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
) -> Layout<W> {
    layout_with_family(text, update, prefix, suffix, TextFamily::SystemUi)
}

pub fn layout_with_family<W: Host>(
    text: impl Into<String>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
    family: TextFamily,
) -> Layout<W> {
    line_edit(description(
        text,
        None::<String>,
        update,
        prefix,
        suffix,
        family,
    ))
}

pub fn description(
    text: impl Into<String>,
    placeholder: Option<impl Into<String>>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
    family: TextFamily,
) -> LineEdit {
    LineEdit {
        text: text.into(),
        placeholder: placeholder.map(Into::into),
        update,
        prefix: prefix.into(),
        suffix: suffix.into(),
        family,
    }
}

pub fn library<W: Host>() -> Library<W> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::CURRENT, name::record("current", []));
    cells.set_value(vocabulary::INPUT, name::record("input", []));
    name::named_library(
        ID,
        "line edit",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
}

#[cfg(test)]
mod tests;
