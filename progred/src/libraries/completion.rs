//! Small combinators for a projection's completion vocabulary.

use crate::display::{Completion, CompletionProvider, CompletionText};
use gid::{CellId, Resolution, Step, Value};
use std::rc::Rc;

pub fn select(display: impl Into<CompletionText>, value: Value) -> Completion {
    insert(
        display,
        value,
        Some(crate::selection::at(&[], crate::selection::payload::edge())),
    )
}

pub fn insert(
    display: impl Into<CompletionText>,
    value: Value,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    insertion(
        display,
        Some(value.clone()),
        move || value.clone(),
        on_commit,
    )
}

pub fn generated(
    display: impl Into<CompletionText>,
    create: impl Fn() -> Value + 'static,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    insertion(display, None, create, on_commit)
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
        move || {
            let cell = gid::new_cell_id();
            row([pane(cell), Value::record([(key, cell.into())])])
        },
        Some(Rc::new(move |_, site, staged| {
            let mut path =
                crate::occurrence::below(site, &[Step::Key(PARTS), Step::Element(second.clone())]);
            path.extend(crate::occurrence::of(&[
                Step::Key(key),
                Step::Follow(Resolution::Document),
            ]));
            staged.select(path, crate::selection::payload::empty_pending());
            true
        })),
    )
}

fn insertion(
    display: impl Into<CompletionText>,
    preview: Option<Value>,
    create: impl Fn() -> Value + 'static,
    on_commit: Option<crate::site::Continuation>,
) -> Completion {
    Completion {
        preview,
        ..Completion::new(display, move |session| {
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
}

/// Combine applicable providers in order. An explicit empty vocabulary stays
/// specified; only an entirely unspecified composition returns None.
pub fn combine(providers: impl IntoIterator<Item = CompletionProvider>) -> CompletionProvider {
    let providers: Vec<_> = providers.into_iter().collect();
    Rc::new(move |request| {
        providers
            .iter()
            .filter_map(|provider| provider(request))
            .reduce(|mut offers, next| {
                offers.extend(next);
                offers
            })
    })
}

#[cfg(test)]
mod tests;
