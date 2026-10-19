//! The stock Rust line-edit control shared by atomic-value libraries.
//! A library supplies spelling, affixes, and a conversion callback;
//! the native widget composes Puri text, painting, and handlers directly.

use crate::display::host::{Host, Layout};
use crate::display::{LineEdit, LineUpdate, TextFamily, line_edit};
use gid::Value;
use std::rc::Rc;

pub fn native(update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static) -> LineUpdate {
    Rc::new(update)
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

#[cfg(test)]
mod tests;
