use super::*;
use crate::display::CompletionKind;
use crate::libraries::{control, presentation};
use gid::Resolution;

#[derive(Default)]
pub(crate) struct CompletionResult {
    pub(crate) value: Option<Value>,
    pub(crate) label: Option<(CellId, Option<Value>)>,
    pub(crate) selected: Option<(Path, Value)>,
}

pub(super) fn test_entry(
    entry: Entry<crate::Editor>,
    kind: CompletionKind,
) -> Entry<CompletionResult> {
    Entry {
        display: entry.display,
        detail: entry.detail,
        matches: entry.matches,
        face: entry.face,
        source: entry.source,
        activate: Rc::new(move |result| {
            let mut world = crate::test_editor(Document {
                root: (kind == CompletionKind::Field).then(|| Value::record([])),
                cells: Cells::new(),
            });
            let root = world.model.workspace.document_root().clone();
            world.model.selection = Some(match kind {
                CompletionKind::Value => crate::selection::pending_value(&root, vec![]),
                CompletionKind::Field => pending_edge(&root, &world.sources(), vec![]).unwrap(),
            });
            (entry.activate)(&mut world);
            result.selected = world
                .model
                .selection
                .as_ref()
                .map(|s| (s.path().to_vec(), s.payload()));
            match kind {
                CompletionKind::Value => result.value = world.model.doc.root.clone(),
                CompletionKind::Field => {
                    let cell = match world.model.selection.as_ref().unwrap().path().first() {
                        Some(Step::Key(cell)) => *cell,
                        _ => panic!("label activation must select its field"),
                    };
                    result.label = Some((cell, world.model.doc.cells.value(cell).cloned()));
                }
            }
        }),
    }
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
            CompletionKind::Field
        } else {
            CompletionKind::Value
        },
        query,
        Some(&crate::stack::load().completions),
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
    use crate::display::{CompletionKind, CompletionProvider, CompletionRequest, CompletionScope};
    use fidget::vocabulary::{FIDGET, SPHERE};

    let stack = crate::stack::load();
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
                let result = activated(&test_entry(entry.clone(), CompletionKind::Value));
                assert_eq!(result.value.as_ref(), Some(&expected_value));
                let mut effects = crate::site::PendingChanges {
                    annotation: None,
                    annotation_changed: false,
                    selection: None,
                    selection_changed: false,
                };
                assert!(expected_continuation.as_ref().unwrap()(
                    &sources,
                    &[],
                    &mut effects
                ));
                let (path, payload) = result.selected.unwrap();
                let (expected_path, expected_payload) = effects.selection.unwrap();
                assert_eq!(path, expected_path);
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
fn completion_source_attribution_uses_the_document_vocabulary_name() {
    let stack = crate::stack::load();
    let cell = new_cell_id();
    let mut document = Document {
        root: None,
        cells: Cells::new(),
    };
    document
        .cells
        .set_value(cell, name::record("local entry", []));
    document.cells.set_value(
        crate::libraries::path::vocabulary::DOCUMENT,
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
fn a_plain_missing_name_selection_offers_and_commits_text() {
    let stack = crate::stack::load();
    let root = crate::test_root();
    let doc = Document {
        root: Some(grap::lambda([], Value::record([]))),
        cells: Cells::new(),
    };
    let path = vec![Step::Key(name::vocabulary::NAME)];
    for payload in [Value::record([]), crate::libraries::selection::edge()] {
        let selected = Selection::from_payload(
            &root,
            &src(&doc, &stack.libraries),
            path.clone(),
            payload.clone(),
        );
        assert!(selected.edit().is_none());
        let entries = projected_completion_entries(&doc, &selected);
        assert_eq!(entries.len(), 1);
        let world = activate_projected(&doc, &selected, &entries[0]);
        assert_eq!(selected.payload(), payload);
        assert_eq!(world.sources().resolve_path(&path), Some(&text::value("")));
        assert_eq!(
            world
                .model
                .selection
                .as_ref()
                .unwrap()
                .stage(&world.sources()),
            Stage::Edge
        );
        assert!(src(&doc, &stack.libraries).resolve_path(&path).is_none());
    }
}

#[test]
fn lambda_completion_opens_a_real_missing_body_and_keeps_the_name_editable() {
    use crate::libraries::grap::vocabulary::GRAP;
    use grap::vocabulary::{BODY, PARAMS};

    let libraries = core_libraries();
    let root = crate::test_root();
    let cell = new_cell_id();
    let doc = Document {
        root: Some(Value::record([(GRAP, cell.into())])),
        cells: Cells::new(),
    };
    let path = vec![Step::Key(GRAP), Step::Follow(Resolution::Document)];
    for query in ["new lambda", "lambda", "λ"] {
        let pending = crate::selection::pending_with_query(&root, path.clone(), query);
        let entries = projected_completion_entries(&doc, &pending);
        let entry = entries
            .iter()
            .find(|entry| entry.display == "new lambda")
            .expect("lambda is available without expanding");
        assert_eq!(entry.detail.as_deref(), Some("grap"));
        let mut world = activate_projected(&doc, &pending, entry);
        assert_eq!(
            world.sources().resolve_path(&path),
            Some(&Value::record([(PARAMS, Value::list([]))]))
        );
        let selected = world.model.selection.as_ref().unwrap();
        let body_path = path
            .iter()
            .cloned()
            .chain([Step::Key(BODY)])
            .collect::<Path>();
        let name_path = path
            .iter()
            .cloned()
            .chain([Step::Key(name::vocabulary::NAME)])
            .collect::<Path>();
        assert_eq!(selected.path(), body_path);
        assert_eq!(selected.stage(&world.sources()), Stage::Pending);
        assert!(world.sources().resolve_path(&body_path).is_none());
        assert!(world.sources().resolve_path(&name_path).is_none());
        assert!(editing_frame(&mut world, false).completion.is_some());

        let name_selection = make_projected_selection(&world.model.doc, &libraries, name_path);
        let names = projected_completion_entries(&world.model.doc, &name_selection);
        assert_eq!(names.len(), 1);
        assert_eq!(
            inserted_value(&world.model.doc, &name_selection, &names[0]),
            Some(text::value(""))
        );
        assert!(
            doc.cells.value(cell).is_none(),
            "offering and staging do not edit the original"
        );
    }
}

#[test]
fn lambda_completion_is_available_in_everything_but_not_in_label_pickers() {
    let libraries = core_libraries();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&doc, &libraries);
    for query in ["lambda", "λ"] {
        for raw in [false, true] {
            let entries = completion_entries(&sources, raw, false, query);
            assert!(entries.iter().any(|entry| entry.display == "new lambda"));
            assert!(
                !completion_entries(&sources, raw, true, query)
                    .iter()
                    .any(|entry| entry.display == "new lambda")
            );
        }
    }
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
    let lib = crate::stack::load().libraries;
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
                let last_number = entries
                    .iter()
                    .rposition(|entry| {
                        activated(entry).value.as_ref().and_then(u64::read) == Some(0)
                    })
                    .unwrap();
                let empty_string = entries
                    .iter()
                    .position(|entry| activated(entry).value == Some(text::value("")))
                    .unwrap();
                assert!(last_number < empty_string);
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
    use crate::libraries::{f32, u64};
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
    let provider: crate::display::CompletionProvider = Rc::new(|request| {
        (request.scope == crate::display::CompletionScope::Everything).then(|| {
            vec![crate::display::Completion::new(
                request.query,
                Value::record([]),
            )]
        })
    });
    let entries = completion_entries_with(
        &src(&document, &libraries),
        false,
        &CompletionKind::Value,
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
    let doc = Document {
        root: Some(Value::list([])),
        cells: Cells::new(),
    };
    let path = vec![Step::Element(gid::position::between(None, None).unwrap())];
    let pending = crate::selection::pending_with_query(&crate::test_root(), path.clone(), "2.5");
    let entries = projected_completion_entries(&doc, &pending);
    let entry = entries
        .iter()
        .find(|entry| entry.detail.as_deref() == Some("f64"))
        .unwrap();
    let world = activate_projected(&doc, &pending, entry);
    assert_eq!(world.sources().resolve_path(&path), Some(&f64::value(2.5)));
}

#[test]
fn atomic_completions_select_and_the_projection_supplies_default_editing() {
    use crate::libraries::{f32, u64};
    let libraries = core_libraries();
    let root = crate::test_root();
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
        let entry = entries
            .iter()
            .find(|entry| inserted_value(&doc, &pending, entry).as_ref() == Some(&value))
            .unwrap();
        let mut world = activate_projected(&doc, &pending, entry);
        let document = world.model.doc.clone();
        let selected = world.model.selection.as_ref().unwrap();
        assert_eq!(selected.path(), path);
        assert_eq!(selected.stage(&src(&document, &libraries)), Stage::Edge);
        assert!(selected.edit().is_none());
        assert_eq!(
            selected.payload(),
            make_projected_selection(&document, &libraries, path.clone()).payload()
        );
        let mut frame = editing_frame(&mut world, false);
        assert!(
            world.model.selection.as_ref().unwrap().edit().is_none(),
            "projection is pure"
        );
        frame
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &arrow(NamedKey::End));
        let selected = world.model.selection.as_ref().unwrap();
        let editor = selected.edit().expect("the selected line handles input");
        assert_eq!(editor.text(), spelling);
        assert_eq!(editor.selection_offsets(), (spelling.len(), spelling.len()));
        *world.model.selection.as_mut().unwrap().edit_mut().unwrap() =
            LineEditState::from_parts(spelling, 0, spelling.len(), None, None);
        assert!(
            editing_frame(&mut world, false)
                .resolve_for_dispatch()
                .dispatch_ime(&mut world, &puri::handler::ImeEvent::Commit(edit.into()),)
        );
        assert_eq!(world.sources().resolve_path(&path), Some(&changed));
    }
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
    for payload in [custom.clone(), crate::libraries::absent::value()] {
        let continuation = crate::libraries::selection::at(&[], payload.clone());
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
    let offer = crate::libraries::completion::label(field);
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
        Selection::from_payload(&root, &src(&doc, &libraries), path, payload)
            .stage(&src(&doc, &libraries)),
        Stage::Pending
    );
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
fn general_value_providers_are_lazy_and_respect_narrow_and_label_pickers() {
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
            && request.scope == crate::display::CompletionScope::Everything)
            .then(|| {
                count.set(count.get() + 1);
                vec![crate::display::Completion::new(
                    request.query,
                    Value::record([]),
                )]
            })
    });
    let narrow: crate::display::CompletionProvider = Rc::new(|_| Some(vec![]));
    let entries = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
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
        &CompletionKind::Value,
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
        &CompletionKind::Field,
        "custom",
        Some(&provider),
        None,
        true,
    );
    completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
        "\"custom\"",
        Some(&provider),
        None,
        true,
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn contextual_completion_starts_narrow_and_everything_widens_it() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = src(&doc, &stack.libraries);
    let entries = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
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
    assert!(!activated(&entries[0]).selected.unwrap().0.is_empty());
    assert_eq!(entries.len(), 1);

    let widened = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Value,
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
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let root = crate::test_root();
    let pending = crate::selection::pending_value(&root, vec![]);
    let offers = root_completions(&stack);
    assert_eq!(
        offers
            .iter()
            .map(|offer| offer.display.clone())
            .collect::<Vec<_>>(),
        [
            fidget::vocabulary::FIDGET.into(),
            crate::libraries::grap::vocabulary::GRAP.into()
        ]
    );
    for (offer, field) in offers.iter().zip([
        fidget::vocabulary::FIDGET,
        crate::libraries::grap::vocabulary::GRAP,
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
        assert_eq!(
            selected.stage(&src(&prepared.document, &stack.libraries)),
            Stage::Pending
        );
        let choices = projected_completion_entries(&prepared.document, &selected);
        if field == fidget::vocabulary::FIDGET {
            assert!(choices.iter().any(|choice| inserted_value(
                &prepared.document,
                &selected,
                choice
            ) == Some(grap::call(
                fidget::vocabulary::SPHERE.into(),
                []
            ))));
        } else {
            for value in [Value::record([]), Value::list([])] {
                assert!(choices.iter().any(|choice| inserted_value(
                    &prepared.document,
                    &selected,
                    choice
                ) == Some(value.clone())));
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
            Step::Key(crate::libraries::grap::vocabulary::GRAP),
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
                    grap::call(
                        crate::libraries::selection::tests::grap_at(
                            &[],
                            crate::libraries::selection::edge(),
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

fn projected_completion_entries(
    doc: &Document,
    selection: &Selection,
) -> Vec<Entry<crate::Editor>> {
    projected_completion_entries_with(doc, selection, None, None)
}

fn projected_completion_entries_with(
    doc: &Document,
    selection: &Selection,
    projection: Option<&Projection<crate::Editor>>,
    provider: Option<&crate::display::CompletionProvider>,
) -> Vec<Entry<crate::Editor>> {
    let stack = crate::stack::load();
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
    let measured = project(
        ProjectDescription {
            view: &crate::test_root(),
            completions: provider.or(Some(&stack.completions)),
            sources: src(doc, &stack.libraries),
            root: doc.root.as_ref(),
            root_path: &[],
            selection: Some(selection),
            source_selection: Some(selection),
            annotations: &annotations,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: Some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
    );
    let extent = measured.extent;
    crate::display::widget::frame::place(
        measured,
        Placement::root(Rect::from_origin_size(Point::ZERO, extent.size())),
        &Default::default(),
    )
    .completion
    .expect("the selected pending emits its offers")
    .entries
}

fn activate_projected(
    doc: &Document,
    selection: &Selection,
    entry: &Entry<crate::Editor>,
) -> crate::Editor {
    let mut editor = crate::test_editor(doc.clone());
    editor.model.workspace.document.root = selection.root().clone();
    editor.model.selection = Some(Selection::from_payload(
        selection.root(),
        &editor.sources(),
        selection.path().to_vec(),
        selection.payload(),
    ));
    (entry.activate)(&mut editor);
    editor
}

fn inserted_value(
    doc: &Document,
    selection: &Selection,
    entry: &Entry<crate::Editor>,
) -> Option<Value> {
    activate_projected(doc, selection, entry)
        .sources()
        .resolve_path(selection.path())
        .cloned()
}

#[test]
fn only_the_active_empty_requests_completion_offers() {
    use crate::display::{Completion, descend};

    let fields = std::array::from_fn::<_, 32, _>(|_| new_cell_id());
    let requests = Rc::new(std::cell::Cell::new(0));
    let provider: crate::display::CompletionProvider = {
        let requests = requests.clone();
        Rc::new(move |_| {
            requests.set(requests.get() + 1);
            Some(vec![Completion::new("offered", Value::record([]))])
        })
    };
    let projection = Projection::new([crate::display::partial(move |input| {
        input.value?.as_record()?;
        Some(crate::display::col(
            0,
            0.0,
            fields.map(|field| descend(Step::Key(field), None, None)),
        ))
    })]);
    let document = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let selection = pending_value(&crate::test_root(), vec![Step::Key(fields[12])]);
    let entries = projected_completion_entries_with(
        &document,
        &selection,
        Some(&projection),
        Some(&provider),
    );
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
    let root_selection = crate::selection::pending_with_query(&crate::test_root(), Vec::new(), "");
    let root_entries = projected_completion_entries(&empty, &root_selection);
    assert!(!root_entries.is_empty());
    assert!(
        root_entries
            .iter()
            .all(|entry| inserted_value(&empty, &root_selection, entry) != Some(Value::list([])))
    );

    let position = gid::position::between(None, None).unwrap();
    let nested = Document {
        root: Some(Value::list([])),
        cells: Cells::new(),
    };
    let nested_selection = crate::selection::pending_with_query(
        &crate::test_root(),
        vec![Step::Element(position)],
        "",
    );
    let nested_entries = projected_completion_entries(&nested, &nested_selection);
    assert!(nested_entries.iter().any(|entry| inserted_value(&nested, &nested_selection, entry) == Some(Value::list([]))));
}

#[test]
fn root_field_completion_offers_only_root_vocabulary_until_widened() {
    let document = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let stack = crate::stack::load();
    let selection = pending_edge(
        &crate::test_root(),
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
            crate::libraries::grap::vocabulary::GRAP,
            crate::workspace::vocabulary::PANES,
        ]
    );
    for entry in &entries {
        let world = activate_projected(&document, &selection, entry);
        let selected = world.model.selection.as_ref().unwrap();
        assert_eq!(selected.path(), &[Step::Key(entry.source.unwrap())]);
        assert_eq!(selected.stage(&world.sources()), Stage::Pending);
    }
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
    let stack = crate::stack::load();
    let selection = pending_edge(
        &crate::test_root(),
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
    let stack = crate::stack::load();
    let selection = pending_edge(
        &crate::test_root(),
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
            crate::libraries::grap::vocabulary::GRAP,
            crate::workspace::vocabulary::PANES,
        ]
    );
}

#[test]
fn fidget_shape_completion_opens_a_real_missing_radius_and_offers_f32() {
    use fidget::vocabulary::{FIDGET, RADIUS, SPHERE};
    let stack = crate::stack::load();
    let root = crate::test_root();
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
        inserted_value(&document, &selection, entry)
            .as_ref()
            .and_then(crate::libraries::f32::read)
            == Some(0.0)
    }));
    let entry = entries
        .iter()
        .find(|entry| entry.display == "sphere")
        .unwrap();
    let world = activate_projected(&document, &selection, entry);
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(
        world.model.doc.cells.value(cell),
        Some(&grap::call(SPHERE.into(), []))
    );
    let radius = [path.as_slice(), &[Step::Key(RADIUS)]].concat();
    assert_eq!(selected.path(), radius);
    assert_eq!(
        selected.stage(&src(&world.model.doc, &stack.libraries)),
        Stage::Pending
    );
    assert!(
        src(&world.model.doc, &stack.libraries)
            .resolve_path(&radius)
            .is_none()
    );

    for (query, number) in [("", 0.0), ("1.25", 1.25)] {
        let selected = Selection::from_payload(
            &root,
            &src(&world.model.doc, &stack.libraries),
            radius.clone(),
            selection_payload::pending(query, 0),
        );
        let entries = projected_completion_entries(&world.model.doc, &selected);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            inserted_value(&world.model.doc, &selected, &entries[0])
                .as_ref()
                .and_then(crate::libraries::f32::read),
            Some(number)
        );
    }

    let fields = pending_edge(
        &root,
        &src(&world.model.doc, &stack.libraries),
        path.clone(),
    )
    .unwrap();
    let entries = projected_completion_entries(&world.model.doc, &fields);
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
    let stack = crate::stack::load();
    let root = crate::test_root();
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
    let entry = entries
        .iter()
        .find(|entry| {
            inserted_value(&document, &selection, entry) == Some(grap::call(SPHERE.into(), []))
        })
        .unwrap();
    let world = activate_projected(&document, &selection, entry);
    assert_eq!(
        world.sources().resolve_path(&path),
        Some(&grap::call(SPHERE.into(), []))
    );
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        [path.as_slice(), &[Step::Key(extra)]].concat()
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
    use crate::display::{
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
        crate::libraries::Library::<(), ()>::new(
            crate::libraries::Definitions::from_parts(
                definitions,
                grap::ForeignFunctions::default(),
            ),
            crate::display::partial(|_| None),
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
    let provider: crate::display::CompletionProvider = Rc::new(move |request| {
        assert_eq!(request.path, expected_path);
        assert_eq!(request.query, "offered");
        assert_eq!(request.kind, CompletionKind::Field);
        assert_eq!(request.value(), Some(&definition));
        Some(vec![
            Completion::new("offered present", present.into())
                .on_commit(crate::libraries::selection::pending_at(&[])),
            Completion::new("offered missing", missing.into())
                .on_commit(crate::libraries::selection::pending_at(&[])),
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
    let (entries, everything) = crate::completion::completion_entries_with(
        &sources,
        false,
        &request,
        None,
        Some(&provider),
    );
    assert!(!everything);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        activated(&test_entry(entries[0].clone(), CompletionKind::Field)).label,
        Some((missing, None))
    );

    let unspecified: CompletionProvider = Rc::new(|_| None);
    let empty: CompletionProvider = Rc::new(|_| Some(vec![]));
    assert!(
        crate::completion::completion_entries_with(
            &sources,
            false,
            &request,
            None,
            Some(&unspecified)
        )
        .1
    );
    let (entries, everything) =
        crate::completion::completion_entries_with(&sources, false, &request, None, Some(&empty));
    assert!(!everything);
    assert!(entries.is_empty());
    let (entries, everything) =
        crate::completion::completion_entries_with(&sources, true, &request, None, Some(&empty));
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
    let stack = crate::stack::load();
    let root = crate::test_root();
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
        inserted_value(&document, &selected, &entries[0])
            .as_ref()
            .and_then(crate::libraries::f32::read),
        Some(32.0)
    );
}

#[test]
fn completion_callbacks_create_values_and_mint_only_on_activation() {
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let libraries = crate::libraries::Libraries::default();
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
            crate::display::Completion::new("text", text::value("not a label")),
            crate::display::Completion::new("existing", Value::from(existing))
                .on_commit(crate::libraries::selection::pending_at(&[])),
        ])
    });
    let labels = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Field,
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
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&doc, &stack.libraries);
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let provider: crate::display::CompletionProvider = Rc::new(move |_| {
        let count = count.clone();
        Some(vec![crate::display::Completion::generated(
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
        &CompletionKind::Value,
        "generated",
        None,
        Some(&provider),
        false,
    );
    let labels = completion_entries_with(
        &sources,
        false,
        &CompletionKind::Field,
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
        ("grap", crate::libraries::grap::vocabulary::GRAP),
    ] {
        let entry = completion_entries_with(
            &sources,
            false,
            &CompletionKind::Value,
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
    let libraries = crate::libraries::Libraries::default();
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
    let lib = crate::stack::load().libraries;
    let sources = src(&doc, &lib);
    let cell = new_cell_id();
    let offers = |value: Value| Offers {
        entries: vec![Entry {
            display: "offer".to_string(),
            detail: None,
            matches: vec![],
            face: crate::display::Face::Label,
            source: value.as_cell(),
            activate: Rc::new(|_: &mut crate::Editor| {}),
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
