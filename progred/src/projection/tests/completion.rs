use super::*;
use gid::Resolution;
use progred_libraries::{control, presentation};

#[derive(Default)]
struct CompletionResult {
    value: Option<Value>,
    label: Option<(CellId, Option<Value>)>,
    on_commit: Option<Value>,
}

fn value_commit(result: &mut CompletionResult, value: Value, on_commit: Option<Value>) {
    result.value = Some(value);
    result.on_commit = on_commit;
}

fn label_commit(
    result: &mut CompletionResult,
    cell: CellId,
    definition: Option<Value>,
    on_commit: Option<Value>,
) {
    result.label = Some((cell, definition));
    result.on_commit = on_commit;
}

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
            Commit::Label(Rc::new(label_commit))
        } else {
            Commit::Value(Rc::new(value_commit))
        },
        query,
        Some(&crate::stack::load::<()>().completions),
        None,
        true,
    )
}

fn activated(entry: &Entry<CompletionResult>) -> CompletionResult {
    let mut result = CompletionResult::default();
    (entry.activate)(&mut result);
    result
}

#[test]
fn retained_completion_offers_follow_live_names_before_filtering() {
    use fidget::vocabulary::{FIDGET, SPHERE};
    use progred_display::{CompletionKind, CompletionProvider, CompletionRequest, CompletionScope};

    let stack = crate::stack::load::<()>();
    let path = [Step::Key(FIDGET)];
    let mut document = Document {
        root: None,
        cells: Cells::new(),
    };
    let offer = (stack.completions)(&CompletionRequest {
        query: "",
        kind: CompletionKind::Value,
        scope: CompletionScope::Suggested,
        path: &path,
        value_at: &|_| None,
        resolve: &|cell| src(&document, &stack.libraries).definition(cell),
    })
    .unwrap()
    .into_iter()
    .find(|offer| offer.display == SPHERE.into())
    .unwrap();
    let expected_value = offer.value.instantiate();
    let expected_continuation = offer.on_commit.clone();
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
            let (entries, _) = crate::completion::completion_entries_with(
                &sources,
                false,
                &Commit::Value(Rc::new(value_commit)),
                &CompletionRequest {
                    query,
                    kind: CompletionKind::Value,
                    scope: CompletionScope::Suggested,
                    path: &path,
                    value_at: &|_| None,
                    resolve: &|cell| sources.definition(cell),
                },
                None,
                Some(&provider),
            );
            if query.is_empty() || query == display {
                let [entry] = entries.as_slice() else {
                    panic!("expected one offer");
                };
                assert_eq!(entry.display, display);
                assert_eq!(entry.detail.as_deref(), Some(detail));
                let result = activated(entry);
                assert_eq!(result.value.as_ref(), Some(&expected_value));
                assert_eq!(result.on_commit, expected_continuation);
            } else {
                assert!(entries.is_empty(), "stale name matched {query:?}");
            }
        }
    }
}

#[test]
fn completion_source_attribution_uses_the_document_vocabulary_name() {
    let stack = crate::stack::load::<()>();
    let cell = new_cell_id();
    let mut document = Document {
        root: None,
        cells: Cells::new(),
    };
    document
        .cells
        .set_value(cell, name::record("local entry", []));
    document.cells.set_value(
        progred_libraries::path::vocabulary::DOCUMENT,
        name::record("documento", []),
    );
    let entries = completion_entries(
        &src(&document, &stack.libraries),
        false,
        false,
        "local entry",
    );
    let entry = entries
        .iter()
        .find(|entry| entry.source == Some(cell))
        .unwrap();
    assert_eq!(entry.detail.as_deref(), Some("documento"));
}

#[test]
fn duplicate_definitions_offer_one_reference_with_the_selected_name() {
    let cell = new_cell_id();
    let mut library_cells = Cells::new();
    library_cells.set_value(cell, name::record("library name", []));
    let lib = libraries(library_cells);
    let mut doc = Document {
        root: None,
        cells: Cells::new(),
    };
    for (definition, expected) in [
        (None, "library name".to_owned()),
        (
            Some(name::record("document name", [])),
            "document name".to_owned(),
        ),
        (Some(Value::record([])), short_id(cell)),
    ] {
        if let Some(value) = definition {
            doc.cells.set_value(cell, value);
        }
        let entries = completion_entries(&src(&doc, &lib), false, false, "");
        let references: Vec<_> = entries
            .iter()
            .filter(|entry| entry.source == Some(cell))
            .collect();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].display, expected);
        assert_eq!(activated(references[0]).value, Some(cell.into()));
    }
}

#[test]
fn completion_offers_follow_the_stage() {
    let lib = crate::stack::load::<()>().libraries;
    let (mut doc, cell) = doc_of(vec![
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

    // The value stage offers the string, references, the value
    // constructors, and the mint.
    let value_stage = displays(false, "");
    assert!(
        value_stage.len() > 8,
        "the full ranked set remains navigable"
    );
    assert!(value_stage.iter().any(|d| d == "roof"));
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

    // Reference commits are links.
    let roof = completion_entries(&sources, false, false, "roof");
    assert_eq!(activated(&roof[0]).value, Some(Value::from(cell)));
    let sum = completion_entries(&sources, false, false, "+");
    assert_eq!(
        activated(&sum[0]).value,
        Some(Value::from(f64::vocabulary::SUM))
    );

    // A bare id never outranks the typed text: the string the
    // query spells comes before every unnamed reference, however
    // exactly the id matches — ids are for reading; want it
    // reachable, name it.
    let unnamed = new_cell_id();
    Rc::make_mut(&mut doc)
        .cells
        .set_value(unnamed, crate::test_values::text("x"));
    let sources = src(&doc, &lib);
    let entries = completion_entries(&sources, false, false, &short_id(unnamed));
    let atom = entries
        .iter()
        .position(|e| {
            activated(e)
                .value
                .as_ref()
                .is_some_and(|value| text::read(value).is_some())
        })
        .unwrap();
    let reference = entries
        .iter()
        .position(|e| e.source == Some(unnamed))
        .unwrap();
    assert!(atom < reference);
}

#[test]
fn typed_numbers_offer_each_valid_representation_then_literal_text() {
    use progred_libraries::{f32, u64};
    let stack = crate::stack::load::<()>();
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
        for query in ["", " \t"] {
            let entries = completion_entries(&sources, raw, false, query);
            let numbers = entries
                .iter()
                .filter_map(|entry| activated(entry).value)
                .filter(|value| {
                    f32::read(value).is_some()
                        || f64::read(value).is_some()
                        || u64::read(value).is_some()
                })
                .collect::<Vec<_>>();
            assert_eq!(numbers, [f32::value(0.0), f64::value(0.0), u64::value(0)]);
            if query.is_empty() {
                let first_number = entries
                    .iter()
                    .position(|entry| {
                        activated(entry).value.as_ref().and_then(f32::read) == Some(0.0)
                    })
                    .unwrap();
                for constructor in ["new cell", "new list", "new record"] {
                    assert!(
                        entries
                            .iter()
                            .position(|entry| entry.display == constructor)
                            .unwrap()
                            < first_number
                    );
                }
                assert_eq!(
                    activated(&entries[first_number + 3]).value,
                    Some(text::value(""))
                );
            }
        }
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
fn completion_name_matches_precede_numeric_interpretations_unless_explicitly_quoted() {
    use progred_libraries::{f32, u64};
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, name::record("length 12", []));
    let document = Document { root: None, cells };
    let libraries = core_libraries();
    let sources = src(&document, &libraries);
    let entries = completion_entries(&sources, false, false, "12");
    assert_eq!(
        entries
            .iter()
            .take(5)
            .map(|entry| activated(entry).value.unwrap())
            .collect::<Vec<_>>(),
        [
            cell.into(),
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
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, name::record("custom", []));
    let document = Document { root: None, cells };
    let libraries = Libraries::default();
    let provider: progred_display::CompletionProvider = Rc::new(|request| {
        (request.scope == progred_display::CompletionScope::Everything).then(|| {
            vec![progred_display::Completion::new(
                request.query,
                Value::record([]),
            )]
        })
    });
    let entries = completion_entries_with(
        &src(&document, &libraries),
        false,
        &Commit::Value(Rc::new(value_commit)),
        "custom",
        Some(&provider),
        None,
        true,
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| activated(entry).value.unwrap())
            .collect::<Vec<_>>(),
        [cell.into(), Value::record([]), text::value("custom"),]
    );
}

#[test]
fn projected_numeric_offer_commits_the_typed_value() {
    let libraries = core_libraries();
    let doc = Document {
        root: Some(Value::list([])),
        cells: Cells::new(),
    };
    let path = vec![Step::Element(gid::position::between(None, None).unwrap())];
    let pending = crate::selection::pending_with_query(
        &crate::workspace::Root::document(),
        path.clone(),
        "2.5",
    );
    let entries = projected_completion_entries(&doc, &pending);
    let entry = entries
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("f64"))
        .unwrap();
    let prepared = crate::completion::prepare(
        &src(&doc, &libraries),
        &pending,
        &Annotations::default(),
        activated(entry).value.unwrap(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        src(&prepared.document, &libraries).resolve_path(&path),
        Some(&f64::value(2.5))
    );
}

#[test]
fn atomic_completions_select_and_the_projection_supplies_default_editing() {
    use progred_libraries::{f32, u64};
    let libraries = core_libraries();
    let root = crate::workspace::Root::document();
    let doc = Document {
        root: Some(Value::list([])),
        cells: Cells::new(),
    };
    let path = vec![Step::Element(gid::position::between(None, None).unwrap())];
    for (query, value, spelling, edit, changed) in [
        (
            "hello",
            text::value("hello"),
            "hello",
            "hello!",
            text::value("hello!"),
        ),
        (
            "\"hë🦀\"",
            text::value("hë🦀"),
            "hë🦀",
            "new",
            text::value("new"),
        ),
        (
            "\"open",
            text::value("open"),
            "open",
            "closed",
            text::value("closed"),
        ),
        ("\"\"", text::value(""), "", "x", text::value("x")),
        ("2.5", f64::value(2.5), "2.5", "3.5", f64::value(3.5)),
        ("2.5", f32::value(2.5), "2.5", "3.5", f32::value(3.5)),
        ("42", u64::value(42), "42", "43", u64::value(43)),
        (
            "0xff",
            Value::from(vec![0xff]),
            "ff",
            "aabb",
            Value::from(vec![0xaa, 0xbb]),
        ),
    ] {
        let mut pending = crate::selection::pending_with_query(&root, path.clone(), query);
        pending.edit_mut().unwrap().cursor_to_start();
        let entries = projected_completion_entries(&doc, &pending);
        let offer = activated(
            entries
                .iter()
                .find(|entry| activated(entry).value.as_ref() == Some(&value))
                .unwrap(),
        );
        assert!(
            offer.on_commit.is_some(),
            "{query:?} supplies selection policy"
        );
        let prepared = crate::completion::prepare(
            &src(&doc, &libraries),
            &pending,
            &Annotations::default(),
            offer.value.unwrap(),
            None,
            offer.on_commit.as_ref(),
        )
        .unwrap();
        let mut selected = Some(pending);
        let mut document = prepared.document;
        crate::site::install(
            prepared.effects,
            &src(&document, &libraries),
            &root,
            &prepared.path,
            &mut Annotations::default(),
            &mut selected,
        );
        let selected = selected.unwrap();
        assert_eq!(selected.path(), path);
        assert_eq!(selected.stage(), Stage::Edge);
        assert!(selected.edit().is_none());
        assert_eq!(
            selected.payload(),
            make_projected_selection(&document, &libraries, path.clone()).payload()
        );
        let mut world = EditingWorld::new(&document, &libraries);
        world.selection = Some(selected);
        let frame = editing_frame(&mut world, false);
        assert!(
            world.selection.as_ref().unwrap().edit().is_none(),
            "projection is pure"
        );
        frame
            .handler
            .unwrap()
            .dispatch_key(&mut world, &arrow(NamedKey::End));
        let mut selected = world.selection.unwrap();
        let editor = selected.edit().expect("the selected line handles input");
        assert_eq!(editor.text(), spelling);
        assert_eq!(editor.selection_offsets(), (spelling.len(), spelling.len()));
        selected.edit_mut().unwrap().set_text(edit);
        assert!(write_through(&mut document, &libraries, &mut selected));
        assert_eq!(
            src(&document, &libraries).resolve_path(&path),
            Some(&changed)
        );
    }
}

#[test]
fn completion_insertion_never_invents_or_overwrites_selection_policy() {
    let libraries = core_libraries();
    let root = crate::workspace::Root::document();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_with_query(&root, vec![], "original query");
    let old_payload = pending.payload();
    for continuation in [None, Some(grap::lambda([], Value::record([])))] {
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
    for payload in [custom.clone(), progred_libraries::absent::value()] {
        let continuation = progred_libraries::selection::at(&[], payload.clone());
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
    let root = crate::workspace::Root::document();
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let pending = pending_edge(&root, &src(&doc, &libraries), vec![]).unwrap();
    let field = new_cell_id();
    let offer = progred_libraries::completion::label(field);
    let prepared = crate::completion::prepare(
        &src(&doc, &libraries),
        &pending,
        &Annotations::default(),
        offer.value.instantiate(),
        None,
        offer.on_commit.as_ref(),
    )
    .unwrap();
    assert!(!prepared.document_changed);
    assert!(
        src(&prepared.document, &libraries)
            .resolve_path(&[Step::Key(field)])
            .is_none()
    );
    assert!(prepared.effects.selection_changed);
    let (path, payload) = prepared.effects.selection.unwrap();
    assert_eq!(path, [Step::Key(field)]);
    assert_eq!(
        Selection::from_payload(&root, &src(&doc, &libraries), path, payload).stage(),
        Stage::Pending
    );
}

#[test]
fn raw_completions_explicitly_select_structure_without_mounting_hidden_editors() {
    let libraries = core_libraries();
    let root = crate::workspace::Root::document();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_with_query(&root, vec![], "2.5");
    let entries = completion_entries(&src(&doc, &libraries), true, false, "2.5");
    for value in [text::value("2.5"), f64::value(2.5)] {
        let offer = activated(
            entries
                .iter()
                .find(|entry| activated(entry).value.as_ref() == Some(&value))
                .unwrap(),
        );
        let prepared = crate::completion::prepare(
            &src(&doc, &libraries),
            &pending,
            &Annotations::default(),
            offer.value.unwrap(),
            None,
            offer.on_commit.as_ref(),
        )
        .unwrap();
        assert!(prepared.effects.selection_changed);
        assert_eq!(
            prepared.effects.selection,
            Some((vec![], selection_payload::edge()))
        );
    }
}

#[test]
fn general_value_providers_are_lazy_and_respect_narrow_and_label_pickers() {
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = Libraries::default();
    let sources = src(&doc, &libraries);
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let provider: progred_display::CompletionProvider = Rc::new(move |request| {
        (request.kind == progred_display::CompletionKind::Value
            && request.scope == progred_display::CompletionScope::Everything)
            .then(|| {
                count.set(count.get() + 1);
                vec![progred_display::Completion::new(
                    request.query,
                    Value::record([]),
                )]
            })
    });
    let narrow: progred_display::CompletionProvider = Rc::new(|_| Some(vec![]));
    let entries = completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "custom",
        Some(&provider),
        Some(&narrow),
        false,
    );
    assert!(entries.is_empty());
    assert_eq!(calls.get(), 0);
    let entries = completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "custom",
        Some(&provider),
        Some(&narrow),
        true,
    );
    assert_eq!(activated(&entries[0]).value, Some(Value::record([])));
    assert_eq!(calls.get(), 1);
    completion_entries_with(
        &sources,
        false,
        &Commit::Label(Rc::new(label_commit)),
        "custom",
        Some(&provider),
        None,
        true,
    );
    completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "\"custom\"",
        Some(&provider),
        None,
        true,
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn contextual_completion_starts_narrow_and_everything_widens_it() {
    let stack = crate::stack::load::<()>();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = src(&doc, &stack.libraries);
    let entries = completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "sdf",
        Some(&stack.completions),
        Some(&stack.completions),
        false,
    );
    assert_eq!(entries[0].display, "fidget");
    assert!(
        activated(&entries[0])
            .value
            .unwrap()
            .as_record()
            .unwrap()
            .get(&fidget::vocabulary::FIDGET)
            .and_then(Value::as_cell)
            .is_some()
    );
    assert!(activated(&entries[0]).on_commit.is_some());
    assert_eq!(entries.len(), 1);

    let widened = completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "sdf",
        Some(&stack.completions),
        Some(&stack.completions),
        true,
    );
    assert!(widened.len() > entries.len());
    assert!(widened.iter().any(|entry| {
        activated(entry)
            .value
            .as_ref()
            .is_some_and(|value| text::read(value) == Some("sdf"))
    }));

    let entries = completion_entries(&sources, false, false, "fidget");
    assert!(entries.iter().any(|entry| {
        (entry.source == Some(fidget::vocabulary::FIDGET))
            && entry.detail.as_deref() == Some("fidget")
    }));
}

#[test]
fn root_completions_share_a_bare_cell_with_a_left_pane_and_open_its_definition() {
    let stack = crate::stack::load::<()>();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let root = crate::workspace::Root::document();
    let pending = crate::selection::pending_value(&root, vec![]);
    let offers = root_completions(&stack);
    assert_eq!(
        offers
            .iter()
            .map(|offer| offer.display.clone())
            .collect::<Vec<_>>(),
        [
            fidget::vocabulary::FIDGET.into(),
            progred_libraries::grap::vocabulary::GRAP.into()
        ]
    );
    for (offer, field) in offers.iter().zip([
        fidget::vocabulary::FIDGET,
        progred_libraries::grap::vocabulary::GRAP,
    ]) {
        let mut annotations = Annotations::default();
        let inserted = offer.value.instantiate();
        let cell = inserted
            .as_record()
            .unwrap()
            .get(&field)
            .and_then(Value::as_cell)
            .unwrap();
        let prepared = crate::completion::prepare(
            &src(&doc, &stack.libraries),
            &pending,
            &annotations,
            inserted.clone(),
            None,
            offer.on_commit.as_ref(),
        )
        .unwrap();
        assert!(prepared.document_changed);
        assert_eq!(prepared.document.root, Some(inserted));
        assert!(prepared.document.cells.value(cell).is_none());
        let panes = crate::workspace::declarations(prepared.document.root.as_ref());
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].side, crate::workspace::Side::Left);
        let pane =
            crate::spine::get(prepared.document.root.as_ref().unwrap(), &panes[0].path).unwrap();
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
        let mut selected = None;
        crate::site::install(
            prepared.effects,
            &src(&prepared.document, &stack.libraries),
            &root,
            &prepared.path,
            &mut annotations,
            &mut selected,
        );
        let selected = selected.unwrap();
        assert_eq!(
            selected.path(),
            [Step::Key(field), Step::Follow(Resolution::Document)]
        );
        assert_eq!(selected.stage(), Stage::Pending);
        let choices = projected_completion_entries(&prepared.document, &selected);
        if field == fidget::vocabulary::FIDGET {
            assert!(choices.iter().any(|choice| activated(choice).value
                == Some(grap::call(fidget::vocabulary::SPHERE.into(), []))));
        } else {
            for value in [Value::record([]), Value::list([])] {
                assert!(
                    choices
                        .iter()
                        .any(|choice| activated(choice).value == Some(value.clone()))
                );
            }
        }
        let filled = crate::completion::prepare(
            &src(&prepared.document, &stack.libraries),
            &selected,
            &annotations,
            Value::record([]),
            None,
            None,
        )
        .unwrap();
        assert_eq!(filled.document.root, prepared.document.root);
        assert_eq!(filled.document.cells.value(cell), Some(&Value::record([])));
    }
}

#[test]
fn completion_continuations_use_the_insertion_site_and_decline_atomically() {
    let stack = crate::stack::load::<()>();
    let root = crate::workspace::Root::document();
    let field = new_cell_id();
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_value(&root, vec![Step::Key(field)]);
    let offer = root_completions(&stack)
        .into_iter()
        .find(|offer| offer.display == progred_libraries::grap::vocabulary::GRAP.into())
        .unwrap();
    let inserted = offer.value.instantiate();
    let prepared = crate::completion::prepare(
        &src(&doc, &stack.libraries),
        &pending,
        &Annotations::default(),
        inserted.clone(),
        None,
        offer.on_commit.as_ref(),
    )
    .unwrap();
    assert_eq!(
        prepared.effects.selection.as_ref().unwrap().0,
        [
            Step::Key(field),
            Step::Key(progred_libraries::grap::vocabulary::GRAP),
            Step::Follow(Resolution::Document),
        ]
    );
    assert_eq!(
        prepared.document.root,
        Some(Value::record([(field, inserted.clone())]))
    );

    let declined = grap::lambda(
        [],
        grap::call(
            control::vocabulary::DO.into(),
            [(
                control::vocabulary::EXPRESSIONS,
                Value::list([
                    grap::call(offer.on_commit.unwrap(), []),
                    progred_libraries::absent::decline(),
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
            Some(&declined),
        )
        .is_none()
    );
    assert_eq!(doc.root, Some(Value::record([])));
}

fn projected_completion_entries(
    doc: &Document,
    selection: &Selection,
) -> Vec<Entry<CompletionResult>> {
    projected_completion_entries_with(doc, selection, None)
}

fn projected_completion_entries_with(
    doc: &Document,
    selection: &Selection,
    projection: Option<&Projection<CompletionResult>>,
) -> Vec<Entry<CompletionResult>> {
    let stack = crate::stack::load::<CompletionResult>();
    let styles = crate::styles::editor(1.0);
    let annotations = Annotations::default();
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let measured = project::<CompletionResult, crate::frame::Paint>(
        ProjectDescription {
            sources: src(doc, &stack.libraries),
            root: doc.root.as_ref(),
            root_path: &[],
            selection: Some(selection),
            scrub_spelling: None,
            source_selection: Some(selection),
            annotations: &annotations,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: Some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
        Hooks {
            completions: Some(stack.completions.clone()),
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            edit_line: Rc::new(|_, _, _| None),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            state_drag: Rc::new(|_, _, _, _, _| {}),
            scrub: Rc::new(|_, _, _, _, _| false),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(value_commit),
            commit_label: Rc::new(label_commit),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        },
    );
    let extent = measured.extent;
    measured::place(
        measured,
        Placement::root(Rect::from_origin_size(Point::ZERO, extent.size())),
    )
    .completion
    .expect("the selected pending emits its offers")
    .entries
}

#[test]
fn only_the_active_empty_requests_completion_offers() {
    use progred_display::{Completion, CompletionKind, completion, descend};

    let fields = std::array::from_fn::<_, 32, _>(|_| new_cell_id());
    let requests = Rc::new(std::cell::Cell::new(0));
    let provider: progred_display::CompletionProvider = {
        let requests = requests.clone();
        Rc::new(move |_| {
            requests.set(requests.get() + 1);
            Some(vec![Completion::new("offered", Value::record([]))])
        })
    };
    let projection = Projection::new([progred_display::partial(move |_| {
        Some(progred_display::col(
            0,
            0.0,
            fields.map(|field| {
                descend(
                    Step::Key(field),
                    None,
                    Some(completion(CompletionKind::Value, Some(provider.clone()))),
                )
            }),
        ))
    })]);
    let document = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let selection = pending_value(
        &crate::workspace::Root::document(),
        vec![Step::Key(fields[12])],
    );
    let entries = projected_completion_entries_with(&document, &selection, Some(&projection));
    assert_eq!(requests.get(), 1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].display, "offered");
}

#[test]
fn root_completions_do_not_leak_into_nested_pending_values() {
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    let root_entries = projected_completion_entries(
        &empty,
        &crate::selection::pending_with_query(&crate::workspace::Root::document(), Vec::new(), ""),
    );
    assert!(!root_entries.is_empty());
    assert!(
        root_entries
            .iter()
            .all(|entry| activated(entry).value != Some(Value::list([])))
    );

    let position = gid::position::between(None, None).unwrap();
    let nested = Document {
        root: Some(Value::list([])),
        cells: Cells::new(),
    };
    let nested_entries = projected_completion_entries(
        &nested,
        &crate::selection::pending_with_query(
            &crate::workspace::Root::document(),
            vec![Step::Element(position)],
            "",
        ),
    );
    assert!(
        nested_entries
            .iter()
            .any(|entry| activated(entry).value == Some(Value::list([])))
    );
}

#[test]
fn root_field_completion_offers_only_root_vocabulary_until_widened() {
    let document = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let stack = crate::stack::load::<()>();
    let selection = pending_edge(
        &crate::workspace::Root::document(),
        &src(&document, &stack.libraries),
        Vec::new(),
    )
    .unwrap();
    let entries = projected_completion_entries(&document, &selection);
    let cells = entries
        .iter()
        .filter_map(|entry| entry.source)
        .collect::<Vec<_>>();

    assert_eq!(
        cells,
        [
            fidget::vocabulary::FIDGET,
            progred_libraries::grap::vocabulary::GRAP,
            crate::workspace::vocabulary::PANES,
        ]
    );
    assert!(entries.iter().all(|entry| activated(entry).label.is_some()));
}

#[test]
fn grap_call_field_completion_offers_missing_parameters() {
    let function = gid::new_cell_id();
    let left = gid::new_cell_id();
    let right = gid::new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        function,
        Value::record([
            (
                grap::vocabulary::PARAMS,
                Value::list([Value::from(left), Value::from(right)]),
            ),
            (grap::vocabulary::BODY, Value::record([])),
        ]),
    );
    let document = Document {
        root: Some(Value::record([(
            grap::vocabulary::FUNCTION,
            Value::from(function),
        )])),
        cells,
    };
    let stack = crate::stack::load::<()>();
    let selection = pending_edge(
        &crate::workspace::Root::document(),
        &src(&document, &stack.libraries),
        Vec::new(),
    )
    .unwrap();
    let entries = projected_completion_entries(&document, &selection);
    let parameters = entries
        .iter()
        .filter_map(|entry| entry.source)
        .collect::<Vec<_>>();

    assert_eq!(parameters, [left, right]);

    let document = Document {
        root: Some(grap::call(function.into(), [(left, Value::record([]))])),
        ..document
    };
    let entries = projected_completion_entries(&document, &selection);
    assert_eq!(
        entries
            .iter()
            .filter_map(|entry| entry.source)
            .collect::<Vec<_>>(),
        [right]
    );
}

#[test]
fn existing_root_fields_are_not_offered_again() {
    let document = Document {
        root: Some(Value::record([(
            fidget::vocabulary::FIDGET,
            Value::record([]),
        )])),
        cells: Cells::new(),
    };
    let stack = crate::stack::load::<()>();
    let selection = pending_edge(
        &crate::workspace::Root::document(),
        &src(&document, &stack.libraries),
        vec![],
    )
    .unwrap();
    let entries = projected_completion_entries(&document, &selection);
    assert_eq!(
        entries
            .iter()
            .filter_map(|entry| entry.source)
            .collect::<Vec<_>>(),
        [
            progred_libraries::grap::vocabulary::GRAP,
            crate::workspace::vocabulary::PANES,
        ]
    );
}

#[test]
fn fidget_shape_completion_opens_a_real_missing_radius_and_offers_f32() {
    use fidget::vocabulary::{FIDGET, RADIUS, SPHERE};
    let stack = crate::stack::load::<()>();
    let root = crate::workspace::Root::document();
    let cell = new_cell_id();
    let document = Document {
        root: Some(Value::record([(FIDGET, cell.into())])),
        cells: Cells::new(),
    };
    let path = vec![Step::Key(FIDGET), Step::Follow(Resolution::Document)];
    let selection = pending_value(&root, path.clone());
    let entries = projected_completion_entries(&document, &selection);
    assert_eq!(
        entries.first().map(|entry| entry.display.as_str()),
        Some("sphere")
    );
    assert!(entries.iter().any(|entry| {
        activated(entry)
            .value
            .as_ref()
            .and_then(progred_libraries::f32::read)
            == Some(0.0)
    }));
    let offer = activated(
        entries
            .iter()
            .find(|entry| entry.display == "sphere")
            .unwrap(),
    );
    let mut annotations = Annotations::default();
    let prepared = crate::completion::prepare(
        &src(&document, &stack.libraries),
        &selection,
        &annotations,
        offer.value.unwrap(),
        None,
        offer.on_commit.as_ref(),
    )
    .unwrap();
    let mut selected = None;
    crate::site::install(
        prepared.effects,
        &src(&prepared.document, &stack.libraries),
        &root,
        &prepared.path,
        &mut annotations,
        &mut selected,
    );
    let selected = selected.unwrap();
    assert_eq!(
        prepared.document.cells.value(cell),
        Some(&grap::call(SPHERE.into(), []))
    );
    let radius = [path.as_slice(), &[Step::Key(RADIUS)]].concat();
    assert_eq!(selected.path(), radius);
    assert_eq!(selected.stage(), Stage::Pending);
    assert!(
        src(&prepared.document, &stack.libraries)
            .resolve_path(&radius)
            .is_none()
    );

    for (query, number) in [("", 0.0), ("1.25", 1.25)] {
        let selected = Selection::from_payload(
            &root,
            &src(&prepared.document, &stack.libraries),
            radius.clone(),
            selection_payload::pending(query, 0),
        );
        let entries = projected_completion_entries(&prepared.document, &selected);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            activated(&entries[0])
                .value
                .as_ref()
                .and_then(progred_libraries::f32::read),
            Some(number)
        );
    }

    let fields = pending_edge(
        &root,
        &src(&prepared.document, &stack.libraries),
        path.clone(),
    )
    .unwrap();
    let entries = projected_completion_entries(&prepared.document, &fields);
    assert_eq!(
        entries
            .iter()
            .filter_map(|entry| entry.source)
            .collect::<Vec<_>>(),
        [RADIUS]
    );
}

#[test]
fn fidget_call_completions_follow_the_current_lambda_parameters() {
    use fidget::vocabulary::{FIDGET, RADIUS, SPHERE};
    let stack = crate::stack::load::<()>();
    let root = crate::workspace::Root::document();
    let cell = new_cell_id();
    let extra = new_cell_id();
    let path = vec![Step::Key(FIDGET), Step::Follow(Resolution::Document)];
    let mut cells = Cells::new();
    cells.set_value(SPHERE, grap::lambda([extra, RADIUS], Value::record([])));
    let mut document = Document {
        root: Some(Value::record([(FIDGET, cell.into())])),
        cells,
    };
    let selection = pending_value(&root, path.clone());
    let entries = projected_completion_entries(&document, &selection);
    let offer = entries
        .iter()
        .map(activated)
        .find(|offer| offer.value == Some(grap::call(SPHERE.into(), [])))
        .unwrap();
    assert_eq!(
        offer.on_commit,
        Some(progred_libraries::selection::pending_at(&[Step::Key(
            extra
        )]))
    );

    document
        .cells
        .set_value(cell, grap::call(SPHERE.into(), []));
    let selected = pending_edge(&root, &src(&document, &stack.libraries), path).unwrap();
    for parameters in [[extra, RADIUS], [RADIUS, extra]] {
        document
            .cells
            .set_value(SPHERE, grap::lambda(parameters, Value::record([])));
        let entries = projected_completion_entries(&document, &selected);
        assert_eq!(
            entries
                .iter()
                .filter_map(|entry| entry.source)
                .collect::<Vec<_>>(),
            parameters
        );
    }
}

#[test]
fn providers_receive_the_query_kind_and_source_qualified_list_path() {
    use progred_display::{
        Completion, CompletionKind, CompletionProvider, CompletionRequest, CompletionScope,
    };
    let cell = new_cell_id();
    let present = new_cell_id();
    let missing = new_cell_id();
    let library_id = new_cell_id();
    let definition = Value::record([(present, text::value("library"))]);
    let mut definitions = Cells::new();
    definitions.set_value(cell, definition.clone());
    let libraries = Libraries::from_contributions([(
        library_id,
        progred_libraries::Library::<(), ()>::new(
            progred_libraries::Definitions::from_parts(
                definitions,
                grap::ForeignFunctions::default(),
            ),
            progred_display::partial(|_| None),
        ),
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
    let provider: progred_display::CompletionProvider = Rc::new(move |request| {
        assert_eq!(request.path, expected_path);
        assert_eq!(request.query, "offered");
        assert_eq!(request.kind, CompletionKind::Field);
        assert_eq!(request.value(), Some(&definition));
        Some(vec![
            Completion::new("offered present", present.into()),
            Completion::new("offered missing", missing.into()),
        ])
    });
    let value_at = |path: &[Step]| sources.resolve_path(path);
    let request = CompletionRequest {
        query: "offered",
        kind: CompletionKind::Field,
        scope: CompletionScope::Suggested,
        path: &path,
        value_at: &value_at,
        resolve: &|cell| sources.definition(cell),
    };
    let commit = Commit::Label(Rc::new(label_commit));
    let (entries, everything) = crate::completion::completion_entries_with(
        &sources,
        false,
        &commit,
        &request,
        None,
        Some(&provider),
    );
    assert!(!everything);
    assert_eq!(entries.len(), 1);
    assert_eq!(activated(&entries[0]).label, Some((missing, None)));

    let unspecified: CompletionProvider = Rc::new(|_| None);
    let empty: CompletionProvider = Rc::new(|_| Some(vec![]));
    assert!(
        crate::completion::completion_entries_with(
            &sources,
            false,
            &commit,
            &request,
            None,
            Some(&unspecified)
        )
        .1
    );
    let (entries, everything) = crate::completion::completion_entries_with(
        &sources,
        false,
        &commit,
        &request,
        None,
        Some(&empty),
    );
    assert!(!everything);
    assert!(entries.is_empty());
    let (entries, everything) = crate::completion::completion_entries_with(
        &sources,
        true,
        &commit,
        &request,
        None,
        Some(&empty),
    );
    assert!(everything);
    assert!(!entries.is_empty());
}

#[test]
fn fidget_suggestions_cover_source_list_items_and_grap_calls() {
    use fidget::vocabulary::{FIDGET, RADIUS, SPHERE};
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, grap::call(SPHERE.into(), []));
    let list = Value::list([cell.into()]);
    let position = list.as_list().unwrap().keys().next().unwrap().clone();
    let document = Document {
        root: Some(Value::record([(FIDGET, list)])),
        cells,
    };
    let stack = crate::stack::load::<()>();
    let root = crate::workspace::Root::document();
    let sources = src(&document, &stack.libraries);
    let selected = pending_into(&root, &sources, &[Step::Key(FIDGET)]).unwrap();
    assert!(
        projected_completion_entries(&document, &selected)
            .iter()
            .any(|entry| entry.display == "sphere")
    );

    let path = vec![
        Step::Key(FIDGET),
        Step::Element(position),
        Step::Follow(Resolution::Document),
    ];
    let selected = pending_edge(&root, &sources, path.clone()).unwrap();
    let entries = projected_completion_entries(&document, &selected);
    assert_eq!(
        entries
            .iter()
            .filter_map(|entry| entry.source)
            .collect::<Vec<_>>(),
        [RADIUS]
    );

    let selected = Selection::from_payload(
        &root,
        &sources,
        [path.as_slice(), &[Step::Key(RADIUS)]].concat(),
        selection_payload::pending("32", 0),
    );
    let entries = projected_completion_entries(&document, &selected);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        activated(&entries[0])
            .value
            .as_ref()
            .and_then(progred_libraries::f32::read),
        Some(32.0)
    );
}

#[test]
fn completion_callbacks_create_values_and_mint_only_on_activation() {
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = progred_libraries::Libraries::default();
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
    let provider: progred_display::CompletionProvider = Rc::new(move |_| {
        Some(vec![
            progred_display::Completion::new("text", text::value("not a label")),
            progred_display::Completion::new("existing", Value::from(existing)),
        ])
    });
    let labels = completion_entries_with(
        &sources,
        false,
        &Commit::Label(Rc::new(label_commit)),
        "",
        None,
        Some(&provider),
        false,
    );
    assert_eq!(labels.len(), 1);
    assert_eq!(activated(&labels[0]).label, Some((existing, None)));
}

#[test]
fn generated_completions_run_only_on_activation_and_mint_fresh_shared_cells() {
    let stack = crate::stack::load::<()>();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&doc, &stack.libraries);
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let provider: progred_display::CompletionProvider = Rc::new(move |_| {
        let count = count.clone();
        Some(vec![progred_display::Completion::generated(
            "generated",
            move || {
                count.set(count.get() + 1);
                new_cell_id().into()
            },
        )])
    });
    let entries = completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "generated",
        None,
        Some(&provider),
        false,
    );
    let labels = completion_entries_with(
        &sources,
        false,
        &Commit::Label(Rc::new(label_commit)),
        "generated",
        None,
        Some(&provider),
        false,
    );
    assert_eq!(calls.get(), 0);
    assert!(labels.is_empty());
    assert_ne!(activated(&entries[0]).value, activated(&entries[0]).value);
    assert_eq!(calls.get(), 2);

    for (display, field) in [
        ("fidget", fidget::vocabulary::FIDGET),
        ("grap", progred_libraries::grap::vocabulary::GRAP),
    ] {
        let entry = completion_entries_with(
            &sources,
            false,
            &Commit::Value(Rc::new(value_commit)),
            "",
            Some(&stack.completions),
            Some(&stack.completions),
            false,
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
    let libraries = progred_libraries::Libraries::default();
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

#[test]
fn entry_hover_marks_follow_the_visible_offers() {
    let doc = sample_document();
    let lib = crate::stack::load::<()>().libraries;
    let sources = src(&doc, &lib);
    let cell = new_cell_id();
    let offers = |value: Value| Offers {
        entries: vec![Entry {
            display: "offer".to_string(),
            detail: None,
            matches: vec![],
            face: progred_display::Face::Label,
            source: value.as_cell(),
            activate: Rc::new(|_: &mut ()| {}),
        }],
    };

    assert_eq!(
        hover_secondary(&sources, Some(&offers(Value::from(cell))), &Hover::Entry(0)),
        Some(Secondary::Cell(cell))
    );
    assert_eq!(
        hover_secondary(
            &sources,
            Some(&offers(crate::test_values::text("offer"))),
            &Hover::Entry(0)
        ),
        None
    );
    assert_eq!(
        hover_secondary::<()>(&sources, None, &Hover::Entry(0)),
        None
    );
}
