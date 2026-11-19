//! Small combinators for a projection's completion vocabulary.

use crate::display::{Completion, CompletionProvider, CompletionText, Offered};
use gid::{CellId, Step, Value};
use std::rc::Rc;

/// `value`, which is already there, a cell or a value as it reads, put in
/// and selected.
pub fn select(display: impl Into<CompletionText>, value: Value) -> Completion {
    insert(
        display,
        value,
        Some(crate::selection::at(&[], crate::selection::payload::edge())),
    )
}

/// `value`, which is already there, a cell or a value as it reads, put in.
pub fn insert(
    display: impl Into<CompletionText>,
    value: Value,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    let offered = Offered::Value(value.clone());
    insertion(display, offered, move || value.clone(), on_commit)
}

/// `value`, made around the existing `cell`, put in: listed as `cell`.
pub fn uses(
    display: impl Into<CompletionText>,
    cell: CellId,
    value: Value,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    insertion(
        display,
        Offered::Use(cell),
        move || value.clone(),
        on_commit,
    )
}

/// Something new, `value`, which is `what`, put in.
pub fn make(
    display: impl Into<CompletionText>,
    what: Option<CompletionText>,
    value: Value,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    generated(display, what, move || value.clone(), on_commit)
}

/// A new record of `kind`, `value`, named as the kind, put in.
pub fn make_kind(
    kind: CellId,
    value: Value,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    make(kind, Some(kind.into()), value, on_commit)
}

/// Something new, which is `what`, made as it's put in.
pub fn generated(
    display: impl Into<CompletionText>,
    what: Option<CompletionText>,
    create: impl Fn() -> Value + 'static,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    insertion(display, Offered::New(what), create, on_commit)
}

/// Starts an empty document as a row: what `pane` makes of a new cell on the
/// left, and on the right a record whose `key` names the cell, whose
/// definition is picked next.
pub fn starter(key: CellId, pane: impl Fn(CellId) -> Value + 'static) -> Completion {
    use crate::libraries::editor::workspace::{row, vocabulary::PARTS};
    // Where the row lists its second part.
    let second = gid::position::spread(2).remove(1);
    generated(
        key,
        None,
        move || {
            let cell = gid::new_cell_id();
            row([pane(cell), Value::record([(key, cell.into())])])
        },
        Some(Rc::new(move |_, site, staged| {
            let mut path =
                crate::occurrence::below(site, &[Step::Key(PARTS), Step::Element(second.clone())]);
            path.extend(crate::occurrence::of(&[Step::Key(key), Step::Follow]));
            staged.select(path, crate::selection::payload::empty_pending());
            true
        })),
    )
}

fn insertion(
    display: impl Into<CompletionText>,
    offered: Offered,
    create: impl Fn() -> Value + 'static,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    Completion::new(display, offered, move |session| {
        if let Some(picking) = session.pending() {
            let value = create();
            if picking.stage() == crate::selection::Stage::Label {
                if let Some(cell) = value.as_cell() {
                    picking.commit_label(cell, None, on_commit.clone());
                }
            } else {
                picking.commit(value, on_commit.clone());
            }
        }
    })
}

/// Combine applicable providers, each a library's, in order. An offer that
/// doesn't say what it is says which library it's from. An explicit empty
/// vocabulary stays specified; only an entirely unspecified composition
/// returns None.
pub fn combine(
    providers: impl IntoIterator<Item = (CellId, CompletionProvider)>,
) -> CompletionProvider {
    let providers: Vec<_> = providers.into_iter().collect();
    Rc::new(move |request| {
        providers
            .iter()
            .filter_map(|(library, provider)| {
                Some(
                    provider(request)?
                        .into_iter()
                        .map(|offer| match offer.detail {
                            Some(_) => offer,
                            None => offer.with_detail(*library),
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .reduce(|mut offers, next| {
                offers.extend(next);
                offers
            })
    })
}

#[cfg(test)]
#[path = "tests/completion.rs"]
mod tests;
