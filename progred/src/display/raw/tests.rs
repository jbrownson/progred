use super::*;
use crate::libraries::f64;
use crate::libraries::{name, text};
use crate::library::Libraries;
use crate::selection::payload as selection_payload;
use crate::sources::Sources;
use crate::test_support::{
    CompletionResult, activated, completion_entries_with, core_libraries, src,
};
use gid::{Cells, Document, Step};
use ui_events::keyboard::NamedKey;

/// Raw's offers for `query`, reading what's typed through the core libraries.
fn completion_entries(
    sources: &Sources,
    labels: bool,
    query: &str,
) -> Vec<Entry<CompletionResult>> {
    completion_entries_with(
        sources,
        &if labels {
            CompletionKind::Field
        } else {
            CompletionKind::Value
        },
        query,
        Some(&crate::stack::load().completions),
        None,
    )
}

#[test]
fn raw_offers_constructors_and_typed_values_but_no_cells_or_library_vocabulary() {
    let libraries = core_libraries();
    let roof = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(roof, name::record("roof", []));
    let doc = Document { root: None, cells };
    let sources = src(&doc, &libraries);
    let displays = |labels: bool, query: &str| {
        completion_entries(&sources, labels, query)
            .into_iter()
            .map(|entry| entry.display)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        displays(false, ""),
        ["new cell", "new list", "new record", "\"\""]
    );
    assert_eq!(displays(true, ""), ["new cell"]);
    assert_eq!(displays(true, "roof"), ["roof"]);
    // An existing cell is pointed at or pasted, not named here, and what a
    // projection offers, such as a new lambda, is that projection's own.
    for query in ["roof", "lambda", "λ"] {
        for labels in [false, true] {
            assert!(
                completion_entries(&sources, labels, query)
                    .iter()
                    .all(|entry| entry.source.is_none() && entry.display != "new lambda"),
                "{query}"
            );
        }
    }
    // A Grap slot offers one.
    assert!(
        completion_entries_with(
            &sources,
            &CompletionKind::Value,
            "lambda",
            None,
            Some(&crate::libraries::grap::slot_offers()),
        )
        .iter()
        .any(|entry| entry.display == "new lambda")
    );
}

#[test]
fn a_typed_blob_leads_the_value_offers_and_names_a_new_label() {
    let lib = crate::stack::load().libraries;
    let doc = Document::default();
    let sources = src(&doc, &lib);

    let label_blob = completion_entries(&sources, true, "0xff");
    assert_eq!(label_blob[0].display, "0xff");
    assert_eq!(label_blob[0].detail.as_deref(), Some("new label"));
    assert_eq!(
        name::read(activated(&label_blob[0]).label.unwrap().1.as_ref().unwrap()),
        Some("0xff")
    );

    // A blob query leads with the blob, its string form below.
    let value_blob: Vec<_> = completion_entries(&sources, false, "0xff")
        .into_iter()
        .map(|entry| entry.display)
        .collect();
    assert_eq!(value_blob[0], "0xff");
    assert_eq!(value_blob[1], "\"0xff\"");
}

#[test]
fn typed_numbers_offer_each_valid_representation_then_literal_text() {
    use crate::libraries::{f32, u64};
    let stack = crate::stack::load();
    let doc = Document::default();
    let sources = src(&doc, &stack.libraries);
    for (query, expected) in [
        (
            "12",
            vec![f32::value(12.0), f64::value(12.0), u64::value(12)],
        ),
        ("-2.5", vec![f32::value(-2.5), f64::value(-2.5)]),
        ("1e3", vec![f32::value(1000.0), f64::value(1000.0)]),
        (
            " +2 ",
            vec![f32::value(2.0), f64::value(2.0), u64::value(2)],
        ),
    ] {
        let entries = completion_entries(&sources, false, query);
        assert_eq!(
            entries
                .iter()
                .take(expected.len())
                .map(|entry| activated(entry).value.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            activated(&entries[expected.len()]).value,
            Some(text::value(query))
        );
        assert!(
            entries
                .iter()
                .take(expected.len())
                .all(|entry| entry.detail.is_some())
        );
    }
    // An empty query is no number, but it is the empty string; whitespace is
    // text too.
    for (query, expected) in [
        ("", &["new cell", "new list", "new record", "\"\""][..]),
        (" \t", &["\" \t\""]),
    ] {
        let displays = completion_entries(&sources, false, query)
            .into_iter()
            .map(|entry| entry.display)
            .collect::<Vec<_>>();
        assert_eq!(displays, expected, "{query:?}");
    }
    let quote = completion_entries(&sources, false, "\"");
    assert_eq!(activated(&quote[0]).value, Some(text::value("")));
    for query in ["word", "1e", "\"12\"", "0xff"] {
        assert!(
            completion_entries(&sources, false, query)
                .iter()
                .all(|entry| {
                    let value = activated(entry).value.unwrap();
                    f32::read(&value).is_none()
                        && f64::read(&value).is_none()
                        && u64::read(&value).is_none()
                })
        );
    }
    let labels = completion_entries(&sources, true, "12");
    assert!(labels.iter().all(|entry| activated(entry).value.is_none()));
    assert!(labels.iter().any(|entry| {
        activated(entry)
            .label
            .and_then(|(_, definition)| definition)
            .is_some_and(|definition| name::read(&definition) == Some("12"))
    }));
    let entries = completion_entries(&sources, false, "18446744073709551615");
    let integer = entries
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("u64"))
        .unwrap();
    assert_eq!(
        activated(integer).value.as_ref().and_then(u64::read),
        Some(u64::MAX)
    );
    let rounded = completion_entries(&sources, false, "16777217");
    let single = rounded
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("f32"))
        .unwrap();
    assert_eq!(single.display, "16777216");
    assert_eq!(
        activated(single).value.as_ref().and_then(f32::read),
        Some(16777216.0)
    );
}

#[test]
fn exact_readings_precede_text_unless_explicitly_quoted() {
    use crate::libraries::{f32, u64};
    let document = Document::default();
    let libraries = core_libraries();
    let sources = src(&document, &libraries);
    let entries = completion_entries(&sources, false, "12");
    assert_eq!(
        entries
            .iter()
            .take(4)
            .map(|entry| activated(entry).value.unwrap())
            .collect::<Vec<_>>(),
        [
            f32::value(12.0),
            f64::value(12.0),
            u64::value(12),
            text::value("12"),
        ]
    );
    for query in ["\"12", "\"12\""] {
        let entries = completion_entries(&sources, false, query);
        assert_eq!(activated(&entries[0]).value, Some(text::value("12")));
    }
}

#[test]
fn completion_interpretation_order_is_independent_of_the_value_type() {
    let document = Document::default();
    let libraries = Libraries::default();
    let provider: crate::display::CompletionProvider = Rc::new(|request| {
        (request.scope == crate::display::CompletionScope::Literal).then(|| {
            vec![crate::libraries::completion::insert(
                request.query,
                Value::record([]),
                None,
            )]
        })
    });
    let values = |provider: &crate::display::CompletionProvider| {
        completion_entries_with(
            &src(&document, &libraries),
            &CompletionKind::Value,
            "custom",
            Some(provider),
            None,
        )
        .iter()
        .map(|entry| activated(entry).value.unwrap())
        .collect::<Vec<_>>()
    };
    let with_text = crate::libraries::completion::combine([
        provider.clone(),
        text::library::<crate::Editor>().completions.unwrap(),
    ]);
    assert_eq!(
        values(&with_text),
        [Value::record([]), text::value("custom")]
    );
    // Strings come from the text library, so without it nothing reads the
    // query as text.
    assert_eq!(values(&provider), [Value::record([])]);
}

#[test]
fn raw_completions_explicitly_select_structure_without_mounting_hidden_editors() {
    let libraries = core_libraries();
    let doc = Document::default();
    let entries = completion_entries(&src(&doc, &libraries), false, "2.5");
    for value in [text::value("2.5"), f64::value(2.5)] {
        let offer = activated(
            entries
                .iter()
                .find(|entry| activated(entry).value.as_ref() == Some(&value))
                .unwrap(),
        );
        assert_eq!(offer.selected, Some((vec![], selection_payload::edge())));
    }
}

#[test]
fn library_readings_are_asked_only_for_raw_s_values() {
    let doc = Document::default();
    let libraries = Libraries::default();
    let sources = src(&doc, &libraries);
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let provider: crate::display::CompletionProvider = Rc::new(move |request| {
        (request.kind == crate::display::CompletionKind::Value
            && request.scope == crate::display::CompletionScope::Literal)
            .then(|| {
                count.set(count.get() + 1);
                vec![crate::libraries::completion::insert(
                    request.query,
                    Value::record([]),
                    None,
                )]
            })
    });
    // A projection's offers are the hole's whole vocabulary.
    let projection: crate::display::CompletionProvider = Rc::new(|_| Some(vec![]));
    let entries = completion_entries_with(
        &sources,
        &CompletionKind::Value,
        "custom",
        Some(&provider),
        Some(&projection),
    );
    assert!(entries.is_empty());
    assert_eq!(calls.get(), 0);
    let entries = completion_entries_with(
        &sources,
        &CompletionKind::Value,
        "custom",
        Some(&provider),
        None,
    );
    assert_eq!(activated(&entries[0]).value, Some(Value::record([])));
    assert_eq!(calls.get(), 1);
    completion_entries_with(
        &sources,
        &CompletionKind::Field,
        "custom",
        Some(&provider),
        None,
    );
    completion_entries_with(
        &sources,
        &CompletionKind::Value,
        "\"custom\"",
        Some(&provider),
        None,
    );
    // Asked of every value, quoted or not, so a provider answers cheaply.
    assert_eq!(calls.get(), 2);
}

#[test]
fn completion_callbacks_create_values_and_mint_only_on_activation() {
    let doc = Document::default();
    let libraries = crate::library::Libraries::default();
    let sources = src(&doc, &libraries);
    let entries = completion_entries(&sources, false, "");
    let bare = entries
        .iter()
        .find(|entry| entry.display == "new cell")
        .unwrap();
    assert_eq!(bare.source, None);
    let first = activated(bare).value.unwrap().as_cell().unwrap();
    let second = activated(&bare.clone()).value.unwrap().as_cell().unwrap();
    assert_ne!(first, second);
    for (display, expected) in [
        ("new list", Value::list([])),
        ("new record", Value::record([])),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.display == display)
            .unwrap();
        assert_eq!(activated(entry).value, Some(expected));
    }
    let labels = completion_entries(&sources, true, "asdf");
    let label = &labels[0];
    assert_eq!(label.source, None);
    let (first, definition) = activated(label).label.unwrap();
    assert_eq!(name::read(&definition.unwrap()), Some("asdf"));
    assert_ne!(first, activated(label).label.unwrap().0);
    let existing = new_cell_id();
    let provider: crate::display::CompletionProvider = Rc::new(move |_| {
        Some(vec![
            crate::libraries::completion::insert("text", text::value("not a label"), None),
            crate::libraries::completion::insert(
                "existing",
                Value::from(existing),
                Some(crate::selection::pending_at(&[])),
            ),
        ])
    });
    let labels =
        completion_entries_with(&sources, &CompletionKind::Field, "", None, Some(&provider));
    assert_eq!(labels.len(), 1);
    assert_eq!(activated(&labels[0]).label, Some((existing, None)));
}

#[test]
fn completion_constructor_aliases_rank_ahead_of_literal_text() {
    let document = Document::default();
    let libraries = crate::library::Libraries::default();
    let sources = src(&document, &libraries);
    for (query, display, value) in [
        ("[", "new list", Some(Value::list([]))),
        ("(", "new cell", None),
        ("{", "new record", Some(Value::record([]))),
    ] {
        let entries = completion_entries(&sources, false, query);
        assert_eq!(entries[0].display, display);
        assert!(entries[0].matches.is_empty());
        match value {
            Some(value) => assert_eq!(activated(&entries[0]).value, Some(value)),
            None => {
                let first = activated(&entries[0]).value.unwrap().as_cell().unwrap();
                let second = activated(&entries[0]).value.unwrap().as_cell().unwrap();
                assert_ne!(first, second);
            }
        }
        assert_eq!(activated(&entries[1]).value, Some(text::value(query)));
        let quoted = completion_entries(&sources, false, &format!("\"{query}\""));
        assert_eq!(activated(&quoted[0]).value, Some(text::value(query)));
        let labels = completion_entries(&sources, true, query);
        if query == "(" {
            assert_eq!(labels[0].display, "new cell");
            assert!(activated(&labels[0]).label.is_some());
        } else {
            assert!(labels.iter().all(|entry| entry.display != display));
        }
    }
}

fn document(root: Option<Value>) -> crate::driver::Driver {
    crate::driver::Driver::new(
        crate::test_editor(Document {
            root,
            cells: Cells::new(),
        }),
        kurbo::Size::new(600.0, 400.0),
    )
}

/// A hole where the middle of three empty lists was, with `delimiter` typed
/// into its picker.
fn made(delimiter: &str) -> (crate::driver::Driver, Vec<Step>) {
    let root = Value::list([Value::list([]), Value::list([]), Value::list([])]);
    let place = vec![Step::Element(
        crate::test_support::positions(&root)[1].clone(),
    )];
    let mut driver = document(Some(root));
    driver.select(&place);
    driver.press(NamedKey::Backspace);
    driver.type_text(delimiter);
    (driver, place)
}

#[test]
fn a_new_cell_list_or_record_opens_for_what_goes_inside_it() {
    use crate::selection::Stage;
    let typed = |driver: &mut crate::driver::Driver, text: &str| {
        driver.type_text(text);
        driver.press(NamedKey::Enter);
    };
    let cell = [Step::Follow(gid::Resolution::Document)];
    for key in ["(", "[", "{"] {
        let (mut driver, place) = made(key);
        let inside = driver.selection();
        let (path, stage) = match key {
            "(" => (
                crate::occurrence::of(&[&place[..], &cell].concat()),
                Stage::Pending,
            ),
            "[" => (inside.clone(), Stage::Pending),
            _ => (crate::occurrence::of(&place), Stage::Label),
        };
        assert_eq!(inside, path, "{key} selects inside what it made");
        assert_eq!(driver.stage(), Some(stage), "{key}");
        if key == "[" {
            assert!(matches!(
                inside.split_last(),
                Some((crate::occurrence::Step::Element(_), list)) if list == place
            ));
        }
        assert!(driver.picker_entries().is_some(), "{key} shows its picker");
        driver.press(NamedKey::Escape);
        assert_eq!(driver.selection(), place, "Escape steps back out of {key}");
        assert_eq!(driver.stage(), Some(Stage::Edge));
    }

    let (mut driver, place) = made("(");
    typed(&mut driver, "5");
    assert!(
        driver
            .editor()
            .sources()
            .resolve_path(&[&place[..], &cell].concat())
            .is_some()
    );
    let (mut driver, place) = made("[");
    typed(&mut driver, "5");
    assert_eq!(driver.value(&place).as_list().unwrap().len(), 1);
    let (mut driver, place) = made("{");
    typed(&mut driver, "width");
    typed(&mut driver, "5");
    assert_eq!(driver.value(&place).as_record().unwrap().len(), 1);
}

#[test]
fn constructor_keys_make_their_offer_from_an_empty_query() {
    use puri::handler::ImeEvent;
    use ui_events::keyboard::{Key, Modifiers};
    for labels in [false, true] {
        for key in ["[", "(", "{"] {
            // An empty document's root, or a new field's label on a record.
            let mut driver = document(labels.then(|| Value::record([])));
            driver.select::<Step>(&[]);
            if labels {
                driver.press(NamedKey::Enter);
            }
            driver.type_text(key);
            if labels && key != "(" {
                assert_eq!(driver.value(&[]), Value::record([]));
                assert_eq!(driver.line(), Some(key));
            } else {
                let value = driver.value(&[]);
                if labels {
                    assert_eq!(value.as_record().unwrap().len(), 0);
                    assert!(matches!(
                        driver.selection().as_slice(),
                        [crate::occurrence::Step::Key(_)]
                    ));
                } else {
                    match key {
                        "[" => assert_eq!(value, Value::list([])),
                        "{" => assert_eq!(value, Value::record([])),
                        _ => assert!(value.as_cell().is_some()),
                    };
                }
            }
        }
    }
    for modifiers in [Modifiers::CONTROL, Modifiers::META] {
        let mut driver = document(None);
        driver.select::<Step>(&[]);
        assert!(!driver.keystroke(Key::Character("[".into()), modifiers));
        assert!(driver.editor().model.doc.root.is_none());
    }
    for query in ["\"", "search"] {
        let mut driver = document(None);
        driver.select::<Step>(&[]);
        driver.type_text(query);
        driver.type_text("[");
        assert!(driver.editor().model.doc.root.is_none());
        assert_eq!(driver.line(), Some(format!("{query}[").as_str()));
    }
    // While an input method composes, the key is its own, and what it
    // commits is typed.
    let mut driver = document(None);
    driver.select::<Step>(&[]);
    assert!(driver.ime(ImeEvent::Preedit("[".into(), Some((1, 1)))));
    assert!(!driver.keystroke(Key::Character("[".into()), Modifiers::empty()));
    assert!(driver.ime(ImeEvent::Commit("[".into())));
    assert!(driver.editor().model.doc.root.is_none());
    assert_eq!(driver.line(), Some("["));
}

#[test]
fn delimiters_make_constructors_in_raw_s_holes_and_are_text_in_grap_s() {
    use crate::selection::Stage;
    let function = Value::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(::grap::vocabulary::LAMBDA).into(),
        ),
        (
            ::grap::vocabulary::PARAMS,
            Value::list([new_cell_id().into()]),
        ),
    ]);
    let body = vec![Step::Key(::grap::vocabulary::BODY)];
    for key in ["(", "[", "{"] {
        // Raw draws a plain document's root: the key makes what it does and
        // opens it, its picker showing.
        let mut driver = document(None);
        driver.select::<Step>(&[]);
        driver.type_text(key);
        assert!(
            driver.editor().model.doc.root.is_some(),
            "{key} makes a value"
        );
        assert!(
            matches!(driver.stage(), Some(stage) if stage != Stage::Edge),
            "{key} opens what it made: {:?}",
            driver.selection()
        );
        assert!(driver.picker_entries().is_some(), "{key} shows its picker");
        // Grap draws a function's body, and its hole takes the key as text.
        let mut driver = document(Some(function.clone()));
        driver.select(&body);
        driver.type_text(key);
        assert_eq!(
            driver.editor().model.doc.root,
            Some(function.clone()),
            "{key} makes nothing"
        );
        assert_eq!(driver.selection(), body);
        assert_eq!(driver.line(), Some(key));
    }
}

#[test]
fn a_short_blob_takes_only_whole_bytes_typed_as_hex() {
    let update = hex_editor(&[]).update;
    for (input, expected) in [
        ("DEad", Some(vec![0xde, 0xad])),
        ("", Some(vec![])),
        (" 00ff ", Some(vec![0x00, 0xff])),
        ("f", None),
        ("xyz", None),
    ] {
        assert_eq!(update(input, None), expected.map(gid::Value::from));
    }
}
