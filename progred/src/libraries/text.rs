//! A UTF-8 text library over ordinary GID data. Text is a positively
//! recognized record facet, not a GID-core atom; Grap owns its encoding,
//! beside f64's, and keeps text unboxed as it does numbers.
//!
//! When a real string-function library lands here, funnel argument
//! reads and result construction through shared helpers, the way f64
//! routes through `binary`/`unary`: read with `RuntimeValue::as_text`
//! and build with `RuntimeValue::text`.

use crate::libraries::{Library, line_edit, name};
use crate::view::{Layout, ProjectionInput};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xeaaf309c36a65d2811083944da29aec9);
use crate::display::overlay_value;
use ::grap::{ForeignFunction, ForeignFunctions};

pub mod vocabulary {
    use gid::CellId;

    pub use ::grap::text::UTF8;
    /// The text line's write-back rule: overlay the typed spelling
    /// onto the current record, other fields carried.
    pub const UPDATE: CellId = CellId::from_u128(0x27c58b96e1f4d03a8d17b62c94e05fa3);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0xce8afed4a11b74c359ac5c99aa48aae2);
}

// The convention itself lives in Grap, beside f64's; this library remains
// its owner in vocabulary, functions, and projections.
pub use ::grap::text::{read, value};

pub fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    Some(
        current
            .map(|current| overlay_value(current, value(spelling)))
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

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::UPDATE,
        ForeignFunction::from_value(|context, call, environment| {
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context
                .field(call, line_edit::vocabulary::CURRENT)
                .map(|current| context.eval_to_value(current, environment))
                .transpose()?;
            let input = context.eval_to_value(input, environment)?;
            Ok(read(&input)
                .and_then(|text| edit(text, current.as_ref()))
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

/// Text reads as its characters, then any other fields, including one
/// being added.
pub fn display(input: &ProjectionInput<'_, ::grap::RuntimeValue>) -> Option<Layout> {
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

pub fn library() -> Library {
    let mut cells = Cells::new();
    cells.set_value(
        vocabulary::INVALID_INPUT,
        name::record("input is not text", []),
    );
    cells.set_value(vocabulary::UTF8, name::record("utf8", []));
    cells.set_value(vocabulary::UPDATE, name::record("text update", []));
    Library::named(
        ID,
        "text",
        crate::libraries::Definitions::from_parts(cells, functions()),
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
