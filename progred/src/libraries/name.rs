//! A simple name on a record. The field is ordinary GID data. This
//! editor assumes the library; other hosts need not.

use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope, Face};
use crate::libraries::{Library, text};
use crate::library::{Definitions, Libraries};
use crate::sources::Sources;
use ::grap::ForeignFunctions;
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0x3209ad5d23a0c8513f6bd76324a5cf60);

pub mod vocabulary {
    use gid::CellId;

    pub const NAME: CellId = CellId::from_u128(0x02e562654d6d0828d3a7559e6f75fffe);
}

pub fn field(name: impl Into<String>) -> (CellId, Value) {
    (vocabulary::NAME, text::value(name))
}

pub fn record(name: impl Into<String>, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(std::iter::once(field(name)).chain(fields))
}

pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&vocabulary::NAME)
        .and_then(text::read)
}

/// The name `cell`'s definition gives it.
pub fn of<'a>(sources: &Sources<'a>, cell: CellId) -> Option<&'a str> {
    sources.resolve(cell).and_then(|value| read(value.value))
}

/// A library that names itself `name` at its identity `id` and each of
/// `names`' cells by its spelling, defining `functions`.
pub fn named_library<'a, W: Host>(
    id: CellId,
    name: &'a str,
    names: impl IntoIterator<Item = (CellId, &'a str)>,
    functions: ForeignFunctions,
) -> Library<W> {
    let mut cells = Cells::new();
    for (cell, spelling) in std::iter::once((id, name)).chain(names) {
        cells.set_value(cell, record(spelling, []));
    }
    Library::new(Definitions::from_parts(cells, functions))
}

/// Each loaded library and the name it gives itself, in load order.
pub fn library_names(libraries: &Libraries) -> impl Iterator<Item = (CellId, &str)> + '_ {
    libraries
        .iter()
        .filter_map(|(id, definitions)| definitions.value(id).and_then(read).map(|name| (id, name)))
}

/// An editable name uses the ordinary text editor, without literal quotes.
pub(crate) fn editor(value: &Value) -> Option<crate::display::LineEdit> {
    Some(crate::display::LineEdit {
        prefix: String::new(),
        suffix: String::new(),
        ..text::editor(value)?
    })
}

/// Explicitly include the stored name beside a facet's own presentation.
/// This is decoration, not a binding or a record field/value pair.
pub(crate) fn with_name<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    content: Layout<W>,
) -> Layout<W> {
    use crate::display::{Pending, descend_local};
    let step = gid::Step::Key(vocabulary::NAME);
    let has_name = input
        .value
        .is_some_and(|value| value.contains_field(vocabulary::NAME));
    if !has_name && input.pending != Some(Pending::Child(step.clone())) {
        return content;
    }
    crate::display::row(
        6.0,
        [
            descend_local([step], projection(), &input.default_projection),
            content,
        ],
    )
}

pub fn library<W: Host>() -> Library<W> {
    named_library(
        ID,
        "name",
        [(vocabulary::NAME, "name")],
        ForeignFunctions::default(),
    )
    .with_completions(completions)
}

/// What's typed as a new field's label reads as a new cell named that, once
/// anything has been.
fn completions(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.scope == CompletionScope::Literal
        && request.kind == CompletionKind::Field
        && !request.query.trim().is_empty())
    .then(|| vec![new_label(text::query_spelling(request.query))])
}

/// A fresh cell named `spelling`, as the new field's label, its value to be
/// filled in next.
fn new_label(spelling: &str) -> Completion {
    let name = spelling.to_string();
    Completion {
        face: Face::Dim,
        ..Completion::new(spelling, move |session| {
            if let Some(picking) = session.pending() {
                picking.commit_label(
                    gid::new_cell_id(),
                    Some(record(&name, [])),
                    Some(crate::selection::pending_at(&[])),
                );
            }
        })
        .with_detail("new label")
    }
}

/// A name as its line editor or, missing, a hole for one. A projection
/// passes this where it descends into a name.
pub(crate) fn projection<W: Host>() -> Partial<W> {
    crate::display::partial(|input| match input.value {
        Some(value) => editor(value).map(crate::display::line_edit),
        None => Some(hole()),
    })
}

/// An empty name, which takes whatever is typed as its text.
pub(crate) fn hole<W: Host>() -> Layout<W> {
    crate::display::picker(crate::display::Picker::new(
        crate::display::CompletionKind::Value,
        crate::display::ranked(std::rc::Rc::new(|request| {
            Some(vec![text::completion(text::query_spelling(request.query))])
        })),
    ))
}

#[cfg(test)]
mod tests;
