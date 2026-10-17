//! Small combinators for a projection's completion vocabulary.

use crate::display::{Completion, CompletionProvider, CompletionText, Face};
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
    let face = if preview.as_ref().and_then(super::text::read).is_some() {
        Face::String
    } else if preview.as_ref().and_then(Value::as_blob).is_some() {
        Face::Id
    } else {
        Face::Label
    };
    let mut offer = Completion::new(display, move |session| {
        let value = create();
        match session
            .model
            .selection
            .as_ref()
            .map(|s| s.stage(&session.sources()))
        {
            Some(crate::selection::Stage::Pending) => {
                crate::editing::commit_value(session, value, on_commit.clone())
            }
            Some(crate::selection::Stage::Label) => {
                if let Some(cell) = value.as_cell() {
                    crate::editing::commit_label(session, cell, None, on_commit.clone());
                }
            }
            _ => (),
        }
    });
    offer.preview = preview;
    offer.face = face;
    offer
}

/// Combine applicable providers in order. An explicit empty vocabulary stays
/// specified; only an entirely unspecified composition returns None.
pub fn combine(providers: impl IntoIterator<Item = CompletionProvider>) -> CompletionProvider {
    let providers: Vec<_> = providers.into_iter().collect();
    Rc::new(move |request| {
        providers
            .iter()
            .filter_map(|provider| provider(request))
            .fold(None, |offers, next| {
                let mut offers = offers.unwrap_or_else(Vec::new);
                offers.extend(next);
                Some(offers)
            })
    })
}

#[cfg(test)]
mod tests;
