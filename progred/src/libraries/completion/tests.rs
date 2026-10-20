use super::*;
use crate::display::{CompletionKind, CompletionScope};
use crate::libraries::fidget;
use crate::test_support::{activated, bare_request, completion_entries_with, library_offers, src};
use gid::{Document, new_cell_id};

#[test]
fn combined_providers_preserve_order_and_explicit_empty_vocabularies() {
    let request = bare_request(CompletionKind::Field, CompletionScope::Everything, "");
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

#[test]
fn generated_completions_run_only_on_activation_and_mint_fresh_shared_cells() {
    let stack = crate::stack::load();
    let doc = Document::default();
    let sources = src(&doc, &stack.libraries);
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let provider: crate::display::CompletionProvider = Rc::new(move |request| {
        if request.kind != CompletionKind::Value {
            return Some(vec![]);
        }
        let count = count.clone();
        Some(vec![crate::libraries::completion::generated(
            "generated",
            move || {
                count.set(count.get() + 1);
                new_cell_id().into()
            },
            None,
        )])
    });
    let entries = completion_entries_with(
        &sources,
        &CompletionKind::Value,
        "generated",
        None,
        Some(&provider),
    );
    let labels = completion_entries_with(
        &sources,
        &CompletionKind::Field,
        "generated",
        None,
        Some(&provider),
    );
    assert_eq!(calls.get(), 0);
    assert!(labels.is_empty());
    assert_ne!(activated(&entries[0]).value, activated(&entries[0]).value);
    assert_eq!(calls.get(), 2);

    for (display, field) in [
        ("fidget", fidget::vocabulary::FIDGET),
        ("grap", crate::libraries::grap::vocabulary::GRAP),
    ] {
        let entry = completion_entries_with(
            &sources,
            &CompletionKind::Value,
            "",
            None,
            Some(&library_offers(
                stack.completions.clone(),
                crate::display::CompletionScope::Start,
            )),
        )
        .into_iter()
        .find(|entry| entry.display == display)
        .unwrap();
        let first = activated(&entry).value.unwrap();
        let second = activated(&entry.clone()).value.unwrap();
        let cell = |root: &Value| {
            crate::test_support::value_at(
                root,
                &[
                    Step::Key(crate::workspace::vocabulary::MAIN),
                    Step::Key(field),
                ],
            )
            .cloned()
            .unwrap()
        };
        assert_ne!(cell(&first), cell(&second));
    }
}
