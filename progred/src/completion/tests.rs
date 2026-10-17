use super::*;
use crate::annotations::Annotations;
use crate::libraries::{control, f64, fidget, presentation};
use crate::library::Libraries;
use crate::selection::payload as selection_payload;
use crate::selection::{Stage, pending_edge};
use crate::test_support::{
    CompletionResult, completion_entries_with, core_libraries, doc_of, inserted_value,
    library_offers, projected_completion_entries, root_completions, src, test_entry,
};
use gid::{Cells, Document, Resolution, Step};

fn completion_entries(
    sources: &Sources,
    raw: bool,
    labels: bool,
    query: &str,
) -> Vec<Entry<CompletionResult>> {
    completion_entries_with(
        sources,
        raw,
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

fn activated(entry: &Entry<CompletionResult>) -> CompletionResult {
    let mut result = CompletionResult::default();
    (entry.activate)(&mut result);
    result
}

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
            let entries = crate::completion::completion_entries_with(
                &sources,
                &CompletionRequest {
                    raw: false,
                    query,
                    kind: CompletionKind::Value,
                    scope: CompletionScope::Everything,
                    path: &path,
                    value_at: &|_| None,
                    resolve: &|cell| sources.definition(cell),
                    cells: &|| sources.cells().collect(),
                },
                None,
                crate::completion::HoleOffers::Projection(&provider),
            );
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
fn raw_offers_constructors_and_typed_values_but_no_cells_or_library_vocabulary() {
    use crate::display::CompletionScope;
    let libraries = core_libraries();
    let roof = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(roof, name::record("roof", []));
    let doc = Document { root: None, cells };
    let sources = src(&doc, &libraries);
    let displays = |labels: bool, query: &str| {
        completion_entries(&sources, false, labels, query)
            .into_iter()
            .map(|entry| entry.display)
            .collect::<Vec<_>>()
    };
    assert_eq!(displays(false, ""), ["new cell", "new list", "new record"]);
    assert_eq!(displays(true, ""), ["new cell"]);
    assert_eq!(displays(true, "roof"), ["roof"]);
    // An existing cell is pointed at or pasted, not named here, and the
    // libraries' other offers, such as a new lambda, are for projections that
    // ask for them.
    for query in ["roof", "lambda", "λ"] {
        for labels in [false, true] {
            assert!(
                completion_entries(&sources, false, labels, query)
                    .iter()
                    .all(|entry| entry.source.is_none() && entry.display != "new lambda"),
                "{query}"
            );
        }
    }
    let everything = library_offers(
        crate::stack::load().completions,
        CompletionScope::Everything,
    );
    for (kind, offered) in [
        (CompletionKind::Value, true),
        (CompletionKind::Field, false),
    ] {
        assert_eq!(
            completion_entries_with(&sources, false, &kind, "lambda", None, Some(&everything))
                .iter()
                .any(|entry| entry.display == "new lambda"),
            offered
        );
    }
}

#[test]
fn completion_offers_follow_the_stage() {
    let lib = crate::stack::load().libraries;
    let (doc, _) = doc_of(vec![
        name::field("roof"),
        (
            crate::test_values::label("kind"),
            crate::test_values::text("building"),
        ),
    ]);
    let sources = src(&doc, &lib);
    let displays = |labels: bool, query: &str| -> Vec<String> {
        completion_entries(&sources, false, labels, query)
            .into_iter()
            .map(|entry| entry.display)
            .collect()
    };

    // The value stage offers the value constructors and what's typed, and
    // no references.
    let value_stage = displays(false, "");
    assert!(value_stage.iter().all(|d| d != "roof"));
    assert!(
        value_stage.iter().any(|d| d == "new list"),
        "{value_stage:?}"
    );
    assert!(value_stage.iter().any(|d| d == "new record"));
    assert!(value_stage.iter().any(|d| d == "new cell"));

    // The label stage narrows to what can label: no list, record,
    // or blob offers.
    let label_stage = displays(true, "");
    assert!(label_stage.iter().all(|d| d != "new list"));
    assert!(label_stage.iter().all(|d| d != "new record"));
    assert!(label_stage.iter().any(|d| d == "new cell"));
    let label_blob = completion_entries(&sources, false, true, "0xff");
    assert_eq!(label_blob[0].display, "0xff");
    assert_eq!(label_blob[0].detail.as_deref(), Some("new label"));
    assert_eq!(
        name::read(activated(&label_blob[0]).label.unwrap().1.as_ref().unwrap()),
        Some("0xff")
    );

    // A blob query leads with the blob, its string form below.
    let value_blob = displays(false, "0xff");
    assert_eq!(value_blob[0], "0xff");
    assert_eq!(value_blob[1], "\"0xff\"");
}

#[test]
fn typed_numbers_offer_each_valid_representation_then_literal_text() {
    use crate::libraries::{f32, u64};
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&doc, &stack.libraries);
    for raw in [false, true] {
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
            let entries = completion_entries(&sources, raw, false, query);
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
        // An empty query reads as nothing; the empty string takes a `"`.
        for query in ["", " \t"] {
            let displays = completion_entries(&sources, raw, false, query)
                .into_iter()
                .map(|entry| entry.display)
                .collect::<Vec<_>>();
            assert!(
                displays.iter().all(|display| display.starts_with("new ")),
                "{query:?}: {displays:?}"
            );
        }
        let quote = completion_entries(&sources, raw, false, "\"");
        assert_eq!(activated(&quote[0]).value, Some(text::value("")));
        for query in ["word", "1e", "\"12\"", "0xff"] {
            assert!(
                completion_entries(&sources, raw, false, query)
                    .iter()
                    .all(|entry| {
                        let value = activated(entry).value.unwrap();
                        f32::read(&value).is_none()
                            && f64::read(&value).is_none()
                            && u64::read(&value).is_none()
                    })
            );
        }
        let labels = completion_entries(&sources, raw, true, "12");
        assert!(labels.iter().all(|entry| activated(entry).value.is_none()));
        assert!(labels.iter().any(|entry| {
            activated(entry)
                .label
                .and_then(|(_, definition)| definition)
                .is_some_and(|definition| name::read(&definition) == Some("12"))
        }));
    }
    let entries = completion_entries(&sources, false, false, "18446744073709551615");
    let integer = entries
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("u64"))
        .unwrap();
    assert_eq!(
        activated(integer).value.as_ref().and_then(u64::read),
        Some(u64::MAX)
    );
    let rounded = completion_entries(&sources, false, false, "16777217");
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
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = core_libraries();
    let sources = src(&document, &libraries);
    let entries = completion_entries(&sources, false, false, "12");
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
        let entries = completion_entries(&sources, false, false, query);
        assert_eq!(activated(&entries[0]).value, Some(text::value("12")));
    }
}

#[test]
fn completion_interpretation_order_is_independent_of_the_value_type() {
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
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
            false,
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
        text::library().completions.unwrap(),
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
fn document_function_completion_inserts_a_call_and_opens_its_first_argument() {
    let (document, names) = crate::gid_text::parse(include_str!(
        "../../../website/public/lessons/growing-forest.gid"
    ))
    .unwrap();
    let clouds = names["clouds"];
    let time = names["sun_time"];
    let libraries = core_libraries();
    let sources = src(&document, &libraries);
    let everything = library_offers(
        crate::stack::load().completions,
        crate::display::CompletionScope::Everything,
    );
    let entries = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
        "clouds",
        None,
        Some(&everything),
    );
    let call = entries.first().unwrap();
    assert_eq!(call.detail.as_deref(), Some("call"));
    let result = activated(call);
    assert_eq!(result.value, Some(grap::call(clouds.into(), [])));
    let (selected, payload) = result.selected.unwrap();
    assert_eq!(selected, vec![Step::Key(time)]);
    assert_eq!(
        payload
            .as_record()
            .unwrap()
            .get(&crate::libraries::selection::vocabulary::STAGE),
        Some(&crate::libraries::selection::vocabulary::PENDING.into())
    );
}

#[test]
fn call_offers_follow_resolved_definitions_without_evaluating_them() {
    let function = new_cell_id();
    let mut cells = Cells::new();
    let library = crate::library::Library::<crate::Editor, crate::frame::Hovered>::new(
        crate::library::Definitions::from_parts(
            {
                let mut definitions = Cells::new();
                definitions.set_value(function, name::record("probe", []));
                definitions
            },
            grap::ForeignFunctions::default().register(
                function,
                grap::ForeignFunction::from_value(|_, _, _| panic!("completion must not run code")),
            ),
        ),
    );
    let libraries = Libraries::from_contributions([(new_cell_id(), library)]).0;
    let root = Some(Value::list([]));
    let document = Document {
        root: root.clone(),
        cells: cells.clone(),
    };
    let everything = library_offers(
        crate::stack::load().completions,
        crate::display::CompletionScope::Everything,
    );
    let offers = |document: &Document| {
        completion_entries_with(
            &src(document, &libraries),
            false,
            &CompletionKind::Value,
            "probe",
            None,
            Some(&everything),
        )
    };
    let entries = offers(&document);
    assert_eq!(
        activated(entries.first().unwrap()).value,
        Some(grap::call(function.into(), []))
    );

    cells.set_value(function, name::record("probe", []));
    let document = Document { root, cells };
    assert!(
        !offers(&document)
            .iter()
            .any(|entry| entry.detail.as_deref() == Some("call"))
    );
}

#[test]
fn completion_insertion_never_invents_or_overwrites_selection_policy() {
    let libraries = core_libraries();
    let root = crate::test_root();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_with_query(&root, vec![], "original query");
    let old_payload = pending.payload();
    for continuation in [
        None,
        Some(
            Rc::new(crate::site::grap(grap::lambda([], Value::record([])), []))
                as crate::site::Continuation,
        ),
    ] {
        let prepared = crate::completion::prepare(
            &src(&doc, &libraries),
            &pending,
            &Annotations::default(),
            text::value("inserted"),
            None,
            continuation.as_ref(),
        )
        .unwrap();
        assert_eq!(prepared.document.root, Some(text::value("inserted")));
        assert!(!prepared.effects.selection_changed);
        assert_eq!(
            prepared.effects.selection,
            Some((vec![], old_payload.clone()))
        );
    }
    let custom = Value::record([
        (new_cell_id(), text::value("custom state")),
        // A payload is data even when it looks like an application.
        (grap::vocabulary::FUNCTION, new_cell_id().into()),
    ]);
    for payload in [
        custom.clone(),
        crate::libraries::absent::with_reason(gid::new_cell_id()),
    ] {
        let continuation = crate::selection::at(&[], payload.clone());
        let prepared = crate::completion::prepare(
            &src(&doc, &libraries),
            &pending,
            &Annotations::default(),
            text::value("inserted"),
            None,
            Some(&continuation),
        )
        .unwrap();
        assert!(prepared.effects.selection_changed);
        assert_eq!(
            prepared.effects.selection,
            (payload == custom).then(|| (vec![], custom.clone()))
        );
    }
}

#[test]
fn label_offer_explicitly_opens_its_missing_value() {
    let libraries = core_libraries();
    let root = crate::test_root();
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let pending = pending_edge(&root, &src(&doc, &libraries), vec![]).unwrap();
    let field = new_cell_id();
    let offer = crate::libraries::completion::insert(
        field,
        field.into(),
        Some(crate::selection::pending_at(&[])),
    );
    let mut world = crate::test_editor(doc.clone());
    world.model.selection = Some(pending);
    (offer.activate)(&mut world);
    assert_eq!(world.model.doc.root, doc.root);
    assert!(!world.model.history.can_undo());
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), [Step::Key(field)]);
    assert_eq!(selected.stage(&world.sources()), Stage::Pending);
}

#[test]
fn raw_completions_explicitly_select_structure_without_mounting_hidden_editors() {
    let libraries = core_libraries();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let entries = completion_entries(&src(&doc, &libraries), true, false, "2.5");
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
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
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
        false,
        &CompletionKind::Value,
        "custom",
        Some(&provider),
        Some(&projection),
    );
    assert!(entries.is_empty());
    assert_eq!(calls.get(), 0);
    let entries = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
        "custom",
        Some(&provider),
        None,
    );
    assert_eq!(activated(&entries[0]).value, Some(Value::record([])));
    assert_eq!(calls.get(), 1);
    completion_entries_with(
        &sources,
        false,
        &CompletionKind::Field,
        "custom",
        Some(&provider),
        None,
    );
    completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
        "\"custom\"",
        Some(&provider),
        None,
    );
    // Asked of every value, quoted or not, so a provider answers cheaply.
    assert_eq!(calls.get(), 2);
}

#[test]
fn root_completions_share_a_bare_cell_with_a_left_pane_and_open_its_definition() {
    let stack = crate::stack::load();
    let templates = [
        fidget::vocabulary::FIDGET.into(),
        crate::libraries::grap::vocabulary::GRAP.into(),
    ];
    let offers: Vec<_> = root_completions(&stack)
        .into_iter()
        .filter(|offer| templates.contains(&offer.display))
        .collect();
    assert_eq!(
        offers
            .iter()
            .map(|offer| offer.display.clone())
            .collect::<Vec<_>>(),
        templates
    );
    for (offer, field) in offers.iter().zip([
        fidget::vocabulary::FIDGET,
        crate::libraries::grap::vocabulary::GRAP,
    ]) {
        let annotations = Annotations::default();
        let world = crate::libraries::test_widgets::activate_completion(offer);
        let document = &world.model.doc;
        let inserted = document.root.clone().unwrap();
        let cell = inserted
            .as_record()
            .unwrap()
            .get(&field)
            .and_then(Value::as_cell)
            .unwrap();
        assert!(document.cells.value(cell).is_none());
        let panes = crate::workspace::declarations(document.root.as_ref());
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].side, crate::workspace::Side::Left);
        let pane = crate::spine::get(document.root.as_ref().unwrap(), &panes[0].path).unwrap();
        assert_eq!(
            pane,
            &if field == fidget::vocabulary::FIDGET {
                Value::record([
                    (presentation::vocabulary::VALUE, cell.into()),
                    (
                        presentation::vocabulary::VIEWPORT,
                        fidget::vocabulary::PREVIEW_3D.into(),
                    ),
                ])
            } else {
                Value::record([(presentation::vocabulary::RENDER, cell.into())])
            }
        );
        let selected = world.model.selection.as_ref().unwrap();
        assert_eq!(
            selected.path(),
            [Step::Key(field), Step::Follow(Resolution::Document)]
        );
        assert_eq!(
            selected.stage(&src(document, &stack.libraries)),
            Stage::Pending
        );
        let choices = projected_completion_entries(document, selected);
        for value in [Value::record([]), Value::list([])] {
            assert!(
                choices
                    .iter()
                    .any(|choice| inserted_value(document, selected, choice) == Some(value.clone()))
            );
        }
        let filled = crate::completion::prepare(
            &src(document, &stack.libraries),
            selected,
            &annotations,
            Value::record([]),
            None,
            None,
        )
        .unwrap();
        assert_eq!(filled.document.root, document.root);
        assert_eq!(filled.document.cells.value(cell), Some(&Value::record([])));
    }
}

#[test]
fn completion_continuations_use_the_insertion_site_and_decline_atomically() {
    let stack = crate::stack::load();
    let root = crate::test_root();
    let field = new_cell_id();
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_value(&root, vec![Step::Key(field)]);
    let offer = root_completions(&stack)
        .into_iter()
        .find(|offer| offer.display == crate::libraries::grap::vocabulary::GRAP.into())
        .unwrap();
    let mut world = crate::test_editor(doc.clone());
    world.model.selection = Some(crate::selection::pending_value(
        &root,
        vec![Step::Key(field)],
    ));
    (offer.activate)(&mut world);
    let inserted = world
        .model
        .doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&field)
        .unwrap()
        .clone();
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        [
            Step::Key(field),
            Step::Key(crate::libraries::grap::vocabulary::GRAP),
            Step::Follow(Resolution::Document),
        ]
    );

    let declined = grap::lambda(
        [],
        grap::call(
            control::vocabulary::DO.into(),
            [(
                control::vocabulary::EXPRESSIONS,
                Value::list([
                    grap::call(
                        crate::libraries::selection::tests::grap_at(
                            &[],
                            crate::selection::payload::edge(),
                        ),
                        [],
                    ),
                    crate::libraries::absent::decline(),
                ]),
            )],
        ),
    );
    assert!(
        crate::completion::prepare(
            &src(&doc, &stack.libraries),
            &pending,
            &Annotations::default(),
            inserted,
            None,
            Some(&(Rc::new(crate::site::grap(declined, [])) as crate::site::Continuation)),
        )
        .is_none()
    );
    assert_eq!(doc.root, Some(Value::record([])));
}

#[test]
fn built_in_call_completion_opens_its_first_argument() {
    use crate::libraries::{f64::vocabulary::SUM, number::vocabulary::LEFT};
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = core_libraries();
    let everything = library_offers(
        crate::stack::load().completions,
        crate::display::CompletionScope::Everything,
    );
    let entries = completion_entries_with(
        &src(&document, &libraries),
        false,
        &CompletionKind::Value,
        "+",
        None,
        Some(&everything),
    );
    let call = entries
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("call · f64"))
        .expect("+ is offered as an f64 call");
    let result = activated(call);
    assert_eq!(result.value, Some(grap::call(SUM.into(), [])));
    let (selected, payload) = result.selected.unwrap();
    assert_eq!(selected, vec![Step::Key(LEFT)]);
    assert_eq!(
        payload
            .as_record()
            .unwrap()
            .get(&crate::libraries::selection::vocabulary::STAGE),
        Some(&crate::libraries::selection::vocabulary::PENDING.into())
    );
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
        resolve: &|cell| sources.definition(cell),
        cells: &|| sources.cells().collect(),
    };
    let entries = crate::completion::completion_entries_with(
        &sources,
        &request,
        None,
        crate::completion::HoleOffers::Projection(&provider),
    );
    assert_eq!(entries.len(), 1);
    assert_eq!(
        activated(&test_entry(entries[0].clone(), CompletionKind::Field)).label,
        Some((missing, None))
    );

    // A projection that answers nothing offers nothing; only a hole without
    // a projection's offers gets everything.
    let unspecified: CompletionProvider = Rc::new(|_| None);
    let empty: CompletionProvider = Rc::new(|_| Some(vec![]));
    for provider in [&unspecified, &empty] {
        assert!(
            crate::completion::completion_entries_with(
                &sources,
                &request,
                None,
                crate::completion::HoleOffers::Projection(provider),
            )
            .is_empty()
        );
    }
    assert!(
        !crate::completion::completion_entries_with(
            &sources,
            &request,
            None,
            crate::completion::HoleOffers::Raw,
        )
        .is_empty()
    );
}

#[test]
fn completion_callbacks_create_values_and_mint_only_on_activation() {
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = crate::library::Libraries::default();
    let sources = src(&doc, &libraries);
    let entries = completion_entries(&sources, false, false, "");
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
    let labels = completion_entries(&sources, false, true, "asdf");
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
    let labels = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Field,
        "",
        None,
        Some(&provider),
    );
    assert_eq!(labels.len(), 1);
    assert_eq!(activated(&labels[0]).label, Some((existing, None)));
}

#[test]
fn generated_completions_run_only_on_activation_and_mint_fresh_shared_cells() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
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
        false,
        &CompletionKind::Value,
        "generated",
        None,
        Some(&provider),
    );
    let labels = completion_entries_with(
        &sources,
        false,
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
            false,
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
        assert_ne!(
            first.as_record().unwrap().get(&field),
            second.as_record().unwrap().get(&field)
        );
    }
}

#[test]
fn completion_constructor_aliases_rank_ahead_of_literal_text() {
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = crate::library::Libraries::default();
    let sources = src(&document, &libraries);
    for raw in [false, true] {
        for (query, display, value) in [
            ("[", "new list", Some(Value::list([]))),
            ("(", "new cell", None),
            ("{", "new record", Some(Value::record([]))),
        ] {
            let entries = completion_entries(&sources, raw, false, query);
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
            let quoted = completion_entries(&sources, raw, false, &format!("\"{query}\""));
            assert_eq!(activated(&quoted[0]).value, Some(text::value(query)));
            let labels = completion_entries(&sources, raw, true, query);
            if query == "(" {
                assert_eq!(labels[0].display, "new cell");
                assert!(activated(&labels[0]).label.is_some());
            } else {
                assert!(labels.iter().all(|entry| entry.display != display));
            }
        }
    }
}
