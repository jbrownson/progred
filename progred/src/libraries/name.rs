//! A simple name on a record. The field is ordinary GID data. This
//! editor assumes the library; other hosts need not.

use crate::libraries::{Library, text};
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
pub(crate) fn with_name(
    input: &crate::display::ProjectionInput<
        '_,
        crate::Editor,
        crate::frame::Hovered,
        ::grap::RuntimeValue,
    >,
    content: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
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
            descend_local(step, projection(), &input.default_projection),
            content,
        ],
    )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::NAME, record("name", []));
    Library::named(
        ID,
        "name",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
}

/// A name as its line editor or, missing, a hole for one. A projection
/// passes this where it descends into a name.
pub(crate) fn projection() -> crate::display::Partial<crate::Editor, crate::frame::Hovered> {
    crate::display::partial(|input| match input.value {
        Some(value) => editor(value).map(crate::display::line_edit),
        None => Some(hole()),
    })
}

/// An empty name, which takes whatever is typed as its text.
pub(crate) fn hole() -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    crate::display::completion(
        crate::display::CompletionKind::Value,
        Some(std::rc::Rc::new(|request| {
            Some(vec![text::completion(text::query_spelling(request.query))])
        })),
    )
}

#[cfg(test)]
mod tests;
