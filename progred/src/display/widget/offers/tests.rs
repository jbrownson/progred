use super::*;
use crate::libraries::{fidget, name, text};
use crate::library::Libraries;
use crate::selection::payload as selection_payload;
use crate::test_support::{activated, src, test_entry};
use gid::{Cells, Document, Resolution, Step, new_cell_id};

#[test]
fn retained_completion_offers_follow_live_names_before_filtering() {
    use crate::display::{CompletionKind, CompletionProvider, CompletionRequest, CompletionScope};
    use fidget::vocabulary::{FIDGET, SPHERE};

    let stack = crate::stack::load();
    let path = [Step::Key(FIDGET)];
    let mut document = Document {
        root: None,
        cells: Cells::new(),
    };
    let offer = crate::libraries::completion::select(SPHERE, SPHERE.into()).with_detail(fidget::ID);
    let expected_value = offer.test_value();
    let expected_selection = crate::libraries::test_widgets::activate_completion(&offer)
        .model
        .selection
        .unwrap();
    let provider: CompletionProvider = Rc::new(move |_| Some(vec![offer.clone()]));

    for (definition, display, detail) in [
        (None, "sphere".to_owned(), "fidget"),
        (
            Some(name::record("boule", [])),
            "boule".to_owned(),
            "formes",
        ),
        (Some(name::record("", [])), String::new(), ""),
        (Some(Value::record([])), short_id(SPHERE), "sans nom"),
    ] {
        if let Some(value) = definition {
            document.cells.set_value(SPHERE, value);
            document
                .cells
                .set_value(fidget::ID, name::record(detail, []));
        }
        let sources = src(&document, &stack.libraries);
        for query in ["", display.as_str(), "unrelated query", "sphere"] {
            let entries = ranked(provider.clone())(&CompletionRequest {
                raw: false,
                query,
                kind: CompletionKind::Value,
                scope: CompletionScope::Everything,
                path: &path,
                value_at: &|_| None,
                resolve: &|cell| sources.resolve(cell),
                cells: &|| sources.cells(),
                name: &|cell| crate::libraries::name::of(&sources, cell),
                libraries: None,
            });
            if query.is_empty() || query == display {
                let [entry] = entries.as_slice() else {
                    panic!("expected one offer");
                };
                assert_eq!(entry.display, display);
                assert_eq!(entry.detail.as_deref(), Some(detail));
                let result = activated(&test_entry(entry.clone(), CompletionKind::Value));
                assert_eq!(result.value.as_ref(), Some(&expected_value));
                let (path, payload) = result.selected.unwrap();
                let expected_payload = expected_selection.payload();
                assert_eq!(path, expected_selection.path());
                assert_eq!(
                    selection_payload::stage(&payload),
                    selection_payload::stage(&expected_payload)
                );
            } else {
                assert!(entries.is_empty(), "stale name matched {query:?}");
            }
        }
    }
}

#[test]
fn providers_receive_the_query_kind_and_source_qualified_list_path() {
    use crate::display::{CompletionKind, CompletionProvider, CompletionRequest, CompletionScope};
    let cell = new_cell_id();
    let present = new_cell_id();
    let missing = new_cell_id();
    let library_id = new_cell_id();
    let definition = Value::record([(present, text::value("library"))]);
    let mut definitions = Cells::new();
    definitions.set_value(cell, definition.clone());
    let libraries = Libraries::from_contributions([(
        library_id,
        crate::library::Library::<(), ()>::new(crate::library::Definitions::from_parts(
            definitions,
            grap::ForeignFunctions::default(),
        )),
    )])
    .0;
    let mut cells = Cells::new();
    cells.set_value(cell, text::value("document"));
    let list = Value::list([cell.into()]);
    let position = list.as_list().unwrap().keys().next().unwrap().clone();
    let document = Document {
        root: Some(list),
        cells,
    };
    let sources = src(&document, &libraries);
    let path = vec![
        Step::Element(position),
        Step::Follow(Resolution::Library(library_id)),
    ];
    let expected_path = path.clone();
    let provider: crate::display::CompletionProvider = Rc::new(move |request| {
        assert_eq!(request.path, expected_path);
        assert_eq!(request.query, "offered");
        assert_eq!(request.kind, CompletionKind::Field);
        assert_eq!(request.value(), Some(&definition));
        Some(vec![
            crate::libraries::completion::insert(
                "offered present",
                present.into(),
                Some(crate::selection::pending_at(&[])),
            ),
            crate::libraries::completion::insert(
                "offered missing",
                missing.into(),
                Some(crate::selection::pending_at(&[])),
            ),
        ])
    });
    let value_at = |path: &[Step]| sources.resolve_path(path);
    let request = CompletionRequest {
        raw: false,
        query: "offered",
        kind: CompletionKind::Field,
        scope: CompletionScope::Everything,
        path: &path,
        value_at: &value_at,
        resolve: &|cell| sources.resolve(cell),
        cells: &|| sources.cells(),
        name: &|cell| crate::libraries::name::of(&sources, cell),
        libraries: None,
    };
    let entries = ranked(provider)(&request);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        activated(&test_entry(entries[0].clone(), CompletionKind::Field)).label,
        Some((missing, None))
    );

    // A projection that answers nothing offers nothing; only a hole without
    // a projection's offers gets everything.
    let unspecified: CompletionProvider = Rc::new(|_| None);
    let empty: CompletionProvider = Rc::new(|_| Some(vec![]));
    for provider in [unspecified, empty] {
        assert!(ranked::<crate::Editor>(provider)(&request).is_empty());
    }
    let libraries = crate::stack::load().completions;
    assert!(
        !(crate::display::raw::picker::<crate::Editor>(CompletionKind::Field).offers)(
            &CompletionRequest {
                libraries: Some(&libraries),
                ..request
            }
        )
        .is_empty()
    );
}
