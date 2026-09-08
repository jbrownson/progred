//! Small combinators for a projection's completion vocabulary.

use crate::display::{Completion, CompletionProvider};
use gid::CellId;
use std::rc::Rc;

pub fn select(offer: Completion) -> Completion {
    offer.on_commit(crate::libraries::selection::at(
        &[],
        crate::libraries::selection::edge(),
    ))
}

pub fn label(cell: CellId) -> Completion {
    Completion::new(cell, cell.into()).on_commit(crate::libraries::selection::pending_at(&[]))
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
            query: "",
            kind: CompletionKind::Field,
            scope: CompletionScope::Suggested,
            path: &[],
            value_at: &|_| None,
            resolve: &|_| None,
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
                .map(|offer| offer.value.instantiate())
                .collect::<Vec<_>>(),
            [first.into(), second.into()]
        );
        assert!(combine([])(&request).is_none());
        assert!(combine([unspecified.clone()])(&request).is_none());
        assert!(combine([unspecified, empty])(&request).unwrap().is_empty());
    }
}
