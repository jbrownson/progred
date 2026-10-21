//! A name on a record: text, or a formatted name, a list of parts, each
//! text, a cell spelled as that cell's name, or `{subscript: part}`. The
//! field is ordinary GID data. This editor assumes the library; other hosts
//! need not.

use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope, Face, Name};
use crate::libraries::{Library, text};
use crate::library::{Definitions, Libraries};
use crate::sources::Sources;
use ::grap::ForeignFunctions;
use gid::{CellId, Cells, Value};
use puri::text::Script;

pub const ID: CellId = CellId::from_u128(0x3209ad5d23a0c8513f6bd76324a5cf60);

pub mod vocabulary {
    use gid::CellId;

    pub const NAME: CellId = CellId::from_u128(0x02e562654d6d0828d3a7559e6f75fffe);
    /// A formatted name's part drawn as a subscript.
    pub const SUBSCRIPT: CellId = CellId::from_u128(0x90fc0fdf37ba457e81cabee50c0ddc81);
}

pub fn field(name: impl Into<String>) -> (CellId, Value) {
    (vocabulary::NAME, text::value(name))
}

pub fn record(name: impl Into<String>, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(std::iter::once(field(name)).chain(fields))
}

/// A formatted name of `parts`, for a name field.
pub fn formatted(parts: impl IntoIterator<Item = Value>) -> Value {
    Value::list(parts)
}

/// `part` of a formatted name, drawn as a subscript.
pub fn subscript(part: Value) -> Value {
    Value::record([(vocabulary::SUBSCRIPT, part)])
}

/// The name `value` gives when it's plain text.
pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&vocabulary::NAME)
        .and_then(text::read)
}

/// The name `cell`'s definition gives it, formatted or not.
pub fn of<'a>(sources: &Sources<'a>, cell: CellId) -> Option<Name<'a>> {
    of_cell(&|cell| sources.resolve(cell).map(|cell| cell.value), cell)
}

/// The name `cell`'s definition gives it, each cell's definition read by
/// `value_of`.
pub fn of_cell<'a>(
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
    cell: CellId,
) -> Option<Name<'a>> {
    named(value_of, cell, DEPTH)
}

/// How many cells deep a formatted name spells the cells it names, so one
/// that names itself still reads.
const DEPTH: usize = 4;

fn named<'a>(
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
    cell: CellId,
    depth: usize,
) -> Option<Name<'a>> {
    let name = value_of(cell)?.as_record()?.get(&vocabulary::NAME)?;
    Some(match text::read(name) {
        // An empty name names nothing.
        Some("") => return None,
        Some(text) => Name::Plain(text),
        None => {
            let mut runs = Vec::new();
            parts(name, Script::Normal, value_of, depth, &mut runs)?;
            Name::Formatted(runs.into())
        }
    })
}

/// Appends `part`'s runs, in `script` unless it's a subscript: text as
/// itself, a cell as its name or short id, and a list as its parts.
fn parts<'a>(
    part: &'a Value,
    script: Script,
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
    depth: usize,
    runs: &mut Vec<(String, Script)>,
) -> Option<()> {
    let mut push = |text: &str, script: Script| match runs.last_mut() {
        Some((last, last_script)) if *last_script == script => last.push_str(text),
        _ => runs.push((text.to_owned(), script)),
    };
    if let Some(text) = text::read(part) {
        push(text, script);
    } else if let Some(cell) = part.as_cell() {
        match depth
            .checked_sub(1)
            .and_then(|depth| named(value_of, cell, depth))
        {
            Some(Name::Plain(text)) => push(text, script),
            Some(Name::Formatted(named)) => {
                for (text, inner) in named.iter() {
                    push(
                        text,
                        if script == Script::Subscript {
                            script
                        } else {
                            *inner
                        },
                    );
                }
            }
            None => push(&crate::identity::short_id(cell), script),
        }
    } else if let Some(list) = part.as_list() {
        for part in list.values() {
            parts(part, script, value_of, depth, runs)?;
        }
    } else {
        let fields = part.as_record()?;
        (fields.len() == 1).then_some(())?;
        parts(
            fields.get(&vocabulary::SUBSCRIPT)?,
            Script::Subscript,
            value_of,
            depth,
            runs,
        )?;
    }
    Some(())
}

/// A library that names itself `name` at its identity `id` and each of
/// `names`' cells by its spelling, defining `functions`.
pub fn named_library<'a, W: Host>(
    id: CellId,
    name: &'a str,
    names: impl IntoIterator<Item = (CellId, &'a str)>,
    functions: ForeignFunctions,
) -> Library<W> {
    library_named(
        id,
        name,
        names
            .into_iter()
            .map(|(cell, spelling)| (cell, text::value(spelling))),
        functions,
    )
}

/// A library like [`named_library`] whose cells' names are name fields'
/// values, text or formatted.
pub fn library_named<W: Host>(
    id: CellId,
    name: &str,
    names: impl IntoIterator<Item = (CellId, Value)>,
    functions: ForeignFunctions,
) -> Library<W> {
    let mut cells = Cells::new();
    for (cell, name) in std::iter::once((id, text::value(name))).chain(names) {
        cells.set_value(cell, Value::record([(vocabulary::NAME, name)]));
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
    if has_name || input.pending == Some(Pending::Child(step.clone())) {
        crate::display::row(
            6.0,
            [
                descend_local([step], projection(None), &input.default_projection),
                content,
            ],
        )
    } else {
        content
    }
}

pub fn library<W: Host>() -> Library<W> {
    named_library(
        ID,
        "name",
        [
            (vocabulary::NAME, "name"),
            (vocabulary::SUBSCRIPT, "subscript"),
        ],
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

/// A named cell as its name, a reference to it, where a projection means the
/// cell rather than what it holds, as Grap's references do. One without a
/// name is left to show its definition.
pub(crate) fn shallow_cell<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    shallow_cell_with(input, |label| label)
}

/// [`shallow_cell`], its name drawn by `decorate`.
pub(crate) fn shallow_cell_with<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    decorate: impl FnOnce(Layout<W>) -> Layout<W>,
) -> Option<Layout<W>> {
    let cell = input.value?.as_cell()?;
    let name = input.env.name(cell)?;
    Some(shallow_reference(
        input,
        decorate(crate::display::named(&name, Face::Name)),
    ))
}

/// A cell reference drawn as `label`, a stop that a click selects.
pub(crate) fn shallow_reference<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    label: Layout<W>,
) -> Layout<W> {
    crate::display::stop(crate::display::activatable(label, &input.site()))
}

/// A name as its line editor, there or missing: typing into a missing one
/// writes it from the first keystroke. A projection passes this where it
/// descends into a name; a missing or empty name shows `placeholder` dim
/// until one is typed.
pub(crate) fn projection<W: Host>(placeholder: Option<String>) -> Partial<W> {
    crate::display::partial(move |input| {
        Some(crate::display::line_edit(crate::display::LineEdit {
            placeholder: placeholder.clone(),
            ..line(input.value)?
        }))
    })
}

/// The editor of the name `value`, or of a missing one, which its first
/// keystroke writes.
pub(crate) fn line(value: Option<&Value>) -> Option<crate::display::LineEdit> {
    match value {
        Some(value) => editor(value),
        None => editor(&text::value("")),
    }
}

#[cfg(test)]
mod tests;
