//! The document's workspace vocabulary: the rows and columns a view's root
//! splits into, and the document's own libraries. The editor defines and
//! interprets them at a view's root, and draws a row or column deeper in a
//! value as its parts laid out the same way; libraries can use the same
//! ordinary data when constructing templates.

use crate::display::Host;
use crate::display::host::{Layout, Partial, ProjectionInput};
use gid::{CellId, Step};

pub use crate::workspace::row;

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LIBRARIES, PARTS, kind};
}

/// The room between the parts of a row or column drawn inside a value.
const GAP: f64 = 12.0;

/// How a row or column deeper in a value than a view's root draws, by kind.
pub(crate) fn projections<W: Host>() -> [(CellId, Partial<W>); 2] {
    [
        (
            vocabulary::kind::ROW,
            crate::display::runtime_partial(row_display),
        ),
        (
            vocabulary::kind::COLUMN,
            crate::display::runtime_partial(column_display),
        ),
    ]
}

/// A row: its parts side by side, the row as tall as its tallest.
fn row_display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    parts(
        input,
        vocabulary::kind::ROW,
        crate::display::runtime_partial(|list| {
            let parts = crate::display::structure::list_items(list, Some(sized_part()))?;
            Some(crate::display::structure::authoring(
                crate::display::aligned_row(
                    crate::display::RowAlignment::Top { baseline: 0 },
                    GAP,
                    parts.into_iter().map(|(_, part)| part),
                ),
            ))
        }),
    )
}

/// A column: its parts one above another.
fn column_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    parts(
        input,
        vocabulary::kind::COLUMN,
        crate::display::structure::list_column(GAP, Some(sized_part())),
    )
}

/// A row's or column's parts drawn by `list`, then any other fields it has.
/// One with no parts is drawn as the record it is.
fn parts<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    kind: CellId,
    list: Partial<W>,
) -> Option<Layout<W>> {
    crate::display::structure::kind_form(input, kind, [vocabulary::PARTS], |[parts]| {
        (!parts?.list_positions()?.is_empty()).then_some(())?;
        Some(crate::display::descend(
            [Step::Key(vocabulary::PARTS)],
            Some(list),
            None,
        ))
    })
}

/// A part that's a sized value drawn at the size one has where nothing
/// assigns it one. Others draw as they otherwise would.
fn sized_part<W: Host>() -> Partial<W> {
    use crate::libraries::presentation;
    crate::display::partial(|input| {
        presentation::viewport_display(
            input,
            presentation::DEFAULT_SIZE,
            presentation::DEFAULT_SIZE,
        )
    })
}

#[cfg(test)]
mod tests;
