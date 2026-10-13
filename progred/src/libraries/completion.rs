//! Small combinators for a projection's completion vocabulary.

use crate::display::{Completion, CompletionProvider, CompletionText, Face};
use gid::{CellId, Value};
use std::rc::Rc;

pub fn select(display: impl Into<CompletionText>, value: Value) -> Completion {
    insert(
        display,
        value,
        Some(crate::libraries::selection::at(
            &[],
            crate::libraries::selection::edge(),
        )),
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
    let mut offer = Completion::new(display, move |world| {
        let value = create();
        match world
            .model
            .selection
            .as_ref()
            .map(|s| s.stage(&world.sources()))
        {
            Some(crate::selection::Stage::Pending) => {
                crate::editing::commit_value(world, value, on_commit.clone())
            }
            Some(crate::selection::Stage::Label) => {
                if let Some(cell) = value.as_cell() {
                    crate::editing::commit_label(world, cell, None, on_commit.clone());
                }
            }
            _ => (),
        }
    });
    offer.preview = preview;
    offer.face = face;
    offer
}

pub fn label(cell: CellId) -> Completion {
    insert(
        cell,
        cell.into(),
        Some(crate::libraries::selection::pending_at(&[])),
    )
}

pub fn labels(cells: impl IntoIterator<Item = CellId>) -> Vec<Completion> {
    cells.into_iter().map(label).collect()
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
mod tests {
    use super::*;
    use crate::display::{CompletionKind, CompletionRequest, CompletionScope};

    #[test]
    fn combined_providers_preserve_order_and_explicit_empty_vocabularies() {
        let request = CompletionRequest {
            raw: false,
            query: "",
            kind: CompletionKind::Field,
            scope: CompletionScope::Suggested,
            path: &[],
            value_at: &|_| None,
            resolve: &|_| None,
            cells: &Vec::new,
        };
        let calls = Rc::new(std::cell::Cell::new(0));
        let counted = calls.clone();
        let unspecified: CompletionProvider = Rc::new(move |_| {
            counted.set(counted.get() + 1);
            None
        });
        let empty: CompletionProvider = Rc::new(|_| Some(vec![]));
        let first = gid::new_cell_id();
        let second = gid::new_cell_id();
        let combined = combine([
            unspecified.clone(),
            Rc::new(move |_| Some(labels([first]))),
            empty.clone(),
            Rc::new(move |_| Some(labels([second]))),
        ]);
        assert_eq!(calls.get(), 0);
        let offers = combined(&request).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(
            offers
                .iter()
                .map(|offer| offer.test_value())
                .collect::<Vec<_>>(),
            [first.into(), second.into()]
        );
        assert!(combine([])(&request).is_none());
        assert!(combine([unspecified.clone()])(&request).is_none());
        assert!(combine([unspecified, empty])(&request).unwrap().is_empty());
    }
}
