//! A UTF-8 text library over ordinary GID data. Text is a positively
//! recognized record facet, not a GID-core atom; Grap owns its encoding,
//! beside f64's, and keeps text unboxed as it does numbers.
//!
//! When a real string-function library lands here, funnel argument
//! reads and result construction through shared helpers, the way f64
//! routes through `binary`/`unary`: read with `RuntimeValue::as_text`
//! and build native results the way `RuntimeValue::f64` does.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, line_edit, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xeaaf309c36a65d2811083944da29aec9);

pub mod vocabulary {
    pub use ::grap::text::UTF8;
}

// The convention itself lives in Grap, beside f64's; this library remains
// its owner in vocabulary and projections.
pub use ::grap::text::{read, value};

pub fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    Some(
        current
            .map(|current| current.overlaid(value(spelling)))
            .unwrap_or_else(|| value(spelling)),
    )
}

pub fn completion(spelling: &str) -> crate::display::Completion {
    crate::libraries::completion::select(format!("\"{spelling}\""), value(spelling))
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
    Some(crate::display::structure::with_extra_fields(
        input,
        |key| key == vocabulary::UTF8,
        crate::display::line_edit(editor_for_text(text)),
    ))
}

pub(crate) fn editor(value: &Value) -> Option<crate::display::LineEdit> {
    Some(editor_for_text(read(value)?))
}

fn editor_for_text(text: &str) -> crate::display::LineEdit {
    line_edit::description(
        text,
        None::<String>,
        line_edit::native(edit),
        "\"",
        "\"",
        crate::display::TextFamily::SystemUi,
    )
}

pub fn library<W: Host>() -> Library<W> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::UTF8, name::record("utf8", []));
    name::named_library(
        ID,
        "text",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
    .with_projection(crate::display::runtime_partial(display))
    .with_completions(completions)
}

/// Whatever was typed reads as text, once anything has been: the empty string
/// takes at least a `"`.
fn completions(
    request: &crate::display::CompletionRequest<'_>,
) -> Option<Vec<crate::display::Completion>> {
    use crate::display::{CompletionKind, CompletionScope};
    (request.scope == CompletionScope::Literal
        && request.kind == CompletionKind::Value
        && !request.query.trim().is_empty())
    .then(|| vec![completion(query_spelling(request.query))])
}

#[cfg(test)]
mod tests;
