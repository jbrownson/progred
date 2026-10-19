//! Small combinators for a projection's completion vocabulary.

use crate::display::{Completion, CompletionProvider, CompletionText};
use gid::Value;
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
