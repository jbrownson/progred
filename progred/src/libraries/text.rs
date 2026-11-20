//! UTF-8 text over ordinary GID data: a literal of the text kind, `{isa:
//! text, bytes}`, not a GID-core atom. Grap owns its encoding, beside f64's,
//! and keeps text unboxed as it does numbers.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, name};
use gid::Value;
use std::rc::Rc;

pub const ID: gid::CellId = gid::CellId::from_u128(0xeaaf309c36a65d2811083944da29aec9);

pub mod vocabulary {
    pub use ::grap::text::TEXT;
}

// The convention itself lives in Grap, beside f64's; this library remains
// its owner in vocabulary and projections.
pub use ::grap::text::{read, value};

pub fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    Some(super::keeping_fields(current, value(spelling)))
}

pub fn completion(spelling: &str) -> crate::display::Completion {
    crate::display::Completion {
        face: crate::display::Face::String,
        ..crate::libraries::completion::select(format!("\"{spelling}\""), value(spelling))
    }
}

pub fn query_spelling(query: &str) -> &str {
    query
        .trim()
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(query)
}

/// Text reads as its characters, then any other fields, including one
/// being added.
pub fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let text = input.value?.as_text()?;
    Some(crate::display::structure::Fields::default().after(
        input,
        crate::libraries::isa::literal_field,
        crate::display::line_edit(editor_for_text(text)),
    ))
}

pub(crate) fn editor(value: &Value) -> Option<crate::display::LineEdit> {
    Some(editor_for_text(read(value)?))
}

fn editor_for_text(text: &str) -> crate::display::LineEdit {
    crate::display::LineEdit {
        text: text.into(),
        update: Rc::new(edit),
        prefix: "\"".into(),
        suffix: "\"".into(),
        family: crate::display::TextFamily::SystemUi,
        placeholder: None,
    }
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "text",
        [(vocabulary::TEXT, "text")],
        ::grap::ForeignFunctions::default(),
    )
    .with_kinds([(
        vocabulary::TEXT,
        vec![crate::libraries::isa::vocabulary::BYTES],
    )])
    .with_projections_for([(vocabulary::TEXT, crate::display::runtime_partial(display))])
    .with_completions(completions)
}

/// Whatever was typed reads as text, nothing as the empty string, which is
/// text too, unlike an empty number.
fn completions(
    request: &crate::display::CompletionRequest<'_>,
) -> Option<Vec<crate::display::Completion>> {
    use crate::display::{CompletionKind, CompletionScope};
    (request.scope == CompletionScope::Literal && request.kind == CompletionKind::Value)
        .then(|| vec![completion(query_spelling(request.query))])
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
