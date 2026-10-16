use super::*;
use crate::display::{CompletionKind, CompletionRequest, CompletionScope};

#[test]
fn combined_providers_preserve_order_and_explicit_empty_vocabularies() {
    let request = CompletionRequest {
        raw: false,
        query: "",
        kind: CompletionKind::Field,
        scope: CompletionScope::Everything,
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
        Rc::new(move |_| Some(vec![select(first, first.into())])),
        empty.clone(),
        Rc::new(move |_| Some(vec![select(second, second.into())])),
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
