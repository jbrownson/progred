use super::*;
use crate::display::CompletionKind;
use crate::libraries::grap::control;
use crate::test_support::{activate_projected, inserted_value, projected_completion_entries};
use gid::Resolution;

#[test]
fn a_plain_missing_name_selection_offers_and_commits_text() {
    let stack = crate::stack::load();
    let root = crate::test_root();
    let doc = Document {
        root: Some(grap::lambda([], Value::record([]))),
        cells: Cells::new(),
    };
    let path = vec![gid::Step::Key(name::vocabulary::NAME)];
    for payload in [Value::record([]), crate::selection::payload::edge()] {
        let selected = Selection::from_payload(
            &root,
            &src(&doc, &stack.libraries),
            crate::occurrence::of(&path),
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
fn a_grap_program_offers_grap_suggestions() {
    use crate::libraries::grap::vocabulary::GRAP;
    let doc = Document {
        root: Some(Value::record([(GRAP, new_cell_id().into())])),
        cells: Cells::new(),
    };
    let program = vec![Step::Key(GRAP), Step::Follow(Resolution::Document)];
    let offered = |query| {
        projected_completion_entries(
            &doc,
            &crate::selection::pending_with_query(&crate::test_root(), program.clone(), query),
        )
        .into_iter()
        .map(|entry| entry.display)
        .collect::<Vec<_>>()
    };
    assert_eq!(offered("\"")[0], "quote");
    let lambda = offered("lam");
    assert!(lambda.contains(&"new lambda".to_owned()));
    assert!(
        !lambda.contains(&"malformed lambda".to_owned()),
        "error messages stay behind the expansion"
    );
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
            crate::occurrence::read(&world.sources(), &path),
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
        assert!(crate::occurrence::read(&world.sources(), &body_path).is_none());
        assert!(crate::occurrence::read(&world.sources(), &name_path).is_none());
        assert!(editing_frame(&mut world).completion.is_some());

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
fn projected_do_insertion_offers_calls() {
    expression_list_insertion(control::vocabulary::DO);
}

#[test]
fn projected_all_insertion_offers_calls() {
    expression_list_insertion(control::vocabulary::ALL);
}

fn expression_list_insertion(callable: CellId) {
    let function = new_cell_id();
    let argument = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        function,
        name::record(
            "clouds",
            [
                (grap::vocabulary::PARAMS, Value::list([argument.into()])),
                (grap::vocabulary::BODY, Value::record([])),
            ],
        ),
    );
    let document = Document {
        root: Some(grap::call(
            callable.into(),
            [(control::vocabulary::EXPRESSIONS, Value::list([]))],
        )),
        cells,
    };
    let path = vec![
        Step::Key(control::vocabulary::EXPRESSIONS),
        Step::Element(gid::position::between(None, None).unwrap()),
    ];
    let selection =
        crate::selection::pending_with_query(&crate::test_root(), path.clone(), "clouds");
    let entries = projected_completion_entries(&document, &selection);
    let world = activate_projected(&document, &selection, entries.first().unwrap());
    assert_eq!(
        crate::occurrence::read(&world.sources(), &path),
        Some(&grap::call(function.into(), []))
    );
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        [path.as_slice(), &[Step::Key(argument)]].concat()
    );
}

#[test]
fn without_the_text_library_nothing_offers_a_string() {
    let parameter = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(parameter, name::record("growth", []));
    let document = Document {
        root: Some(Value::record([(
            grap::vocabulary::PARAMS,
            Value::list([parameter.into()]),
        )])),
        cells,
    };
    let empty = Document::default();
    let ids = crate::stack::load()
        .libraries
        .iter()
        .map(|(id, _)| id)
        .filter(|id| *id != text::ID)
        .collect::<Vec<_>>();
    let stack = crate::stack::load_selected(&ids).unwrap();
    // Grap's own offers, and everything in a hole no projection supplies.
    for (document, path) in [
        (&document, vec![Step::Key(grap::vocabulary::BODY)]),
        (&empty, vec![]),
    ] {
        for query in ["\"hello", "hello"] {
            let selected =
                crate::selection::pending_with_query(&crate::test_root(), path.clone(), query);
            let entries = offered(
                &mut crate::test_editor_with_stack(document.clone(), stack.clone()),
                &selected,
            );
            assert!(
                entries
                    .iter()
                    .all(|entry| inserted_value(document, &selected, entry)
                        .is_none_or(|value| text::read(&value).is_none())),
                "{query:?} at {path:?}"
            );
        }
    }
}

#[test]
fn expression_completion_offers_grap_s_own_vocabulary() {
    let parameter = new_cell_id();
    let unrelated = new_cell_id();
    let callable = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(parameter, name::record("growth", []));
    cells.set_value(unrelated, name::record("unrelated parameter", []));
    cells.set_value(
        callable,
        name::record(
            "clouds",
            [
                (grap::vocabulary::PARAMS, Value::list([unrelated.into()])),
                (grap::vocabulary::BODY, Value::record([])),
            ],
        ),
    );
    let document = Document {
        root: Some(Value::record([(
            grap::vocabulary::PARAMS,
            Value::list([parameter.into()]),
        )])),
        cells,
    };
    let path = vec![Step::Key(grap::vocabulary::BODY)];
    let selected = crate::selection::pending_value(&crate::test_root(), path.clone());
    let entries = projected_completion_entries(&document, &selected);
    assert!(entries.iter().any(|entry| entry.source == Some(parameter)));
    // Grap's slot offers only what grap does: other cells aren't among them.
    assert!(!entries.iter().any(|entry| entry.source == Some(unrelated)));
    assert!(!entries.iter().any(|entry| entry.display == "name"));
    assert!(
        entries
            .iter()
            .any(|entry| entry.display == "clouds" && entry.detail.as_deref() == Some("call"))
    );
    assert!(entries.iter().any(|entry| entry.display == "new lambda"));
    // The editor's constructors open as they're made: the cell's value
    // pending, an element pending in the list, a field's label on the record.
    // Grap's slot has no keys of its own, so a delimiter is typed as text,
    // which finds its constructor.
    let fresh_cell = |value: &Value| value.as_cell().is_some();
    let list = |value: &Value| *value == Value::list([]);
    let record = |value: &Value| *value == Value::record([]);
    let made: [(&str, &str, &dyn Fn(&Value) -> bool, _, _); 3] = [
        (
            "new cell",
            "(",
            &fresh_cell,
            crate::selection::Stage::Pending,
            path.len() + 1,
        ),
        (
            "new list",
            "[",
            &list,
            crate::selection::Stage::Pending,
            path.len() + 1,
        ),
        (
            "new record",
            "{",
            &record,
            crate::selection::Stage::Label,
            path.len(),
        ),
    ];
    for (display, delimiter, made, stage, depth) in made {
        let typed =
            crate::selection::pending_with_query(&crate::test_root(), path.clone(), delimiter);
        assert_eq!(
            projected_completion_entries(&document, &typed)
                .first()
                .map(|entry| entry.display.as_str()),
            Some(display),
            "{delimiter}"
        );
        let entry = entries
            .iter()
            .find(|entry| entry.display == display)
            .unwrap();
        let mut world = activate_projected(&document, &selected, entry);
        assert!(
            crate::occurrence::read(&world.sources(), &path).is_some_and(made),
            "{display}"
        );
        let opened = world.model.selection.as_ref().unwrap();
        assert_eq!(opened.stage(&world.sources()), stage, "{display}");
        assert!(opened.path().starts_with(&path), "{display}");
        assert_eq!(opened.path().len(), depth, "{display}");
        assert!(
            editing_frame(&mut world).completion.is_some(),
            "{display} shows its picker"
        );
    }

    for (query, expected) in [
        ("cloud", grap::call(callable.into(), [])),
        ("growth", parameter.into()),
        ("\"hello", text::value("hello")),
    ] {
        let selected =
            crate::selection::pending_with_query(&crate::test_root(), path.clone(), query);
        let entries = projected_completion_entries(&document, &selected);
        assert_eq!(
            inserted_value(&document, &selected, &entries[0]),
            Some(expected)
        );
    }
    let selected = crate::selection::pending_with_query(&crate::test_root(), path, "2.5");
    let entries = projected_completion_entries(&document, &selected);
    assert!(
        entries
            .iter()
            .any(|entry| entry.detail.as_deref() == Some("f64"))
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
    assert_eq!(
        crate::occurrence::read(&world.sources(), &path),
        Some(&f64::value(2.5))
    );
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
    let path = vec![gid::Step::Element(
        gid::position::between(None, None).unwrap(),
    )];
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
        let mut pending =
            crate::selection::pending_with_query(&root, crate::occurrence::of(&path), query);
        pending.edit_mut().unwrap().cursor_to_start();
        let entries = projected_completion_entries(&doc, &pending);
        let entry = entries
            .iter()
            .find(|entry| inserted_value(&doc, &pending, entry).as_ref() == Some(&value))
            .unwrap();
        let world = activate_projected(&doc, &pending, entry);
        let document = world.model.doc.clone();
        let selected = world.model.selection.as_ref().unwrap();
        assert_eq!(selected.path(), path);
        assert_eq!(selected.stage(&src(&document, &libraries)), Stage::Edge);
        assert!(selected.edit().is_none());
        assert_eq!(
            selected.payload(),
            make_projected_selection(&document, &libraries, crate::occurrence::of(&path)).payload()
        );
        let mut driver = crate::driver::Driver::new(world, crate::driver::Driver::VIEWPORT);
        assert!(driver.line().is_none(), "projection is pure");
        driver.press(NamedKey::End);
        assert_eq!(
            driver.line(),
            Some(spelling),
            "the selected line handles input"
        );
        assert_eq!(
            driver.line_selection(),
            Some((spelling.len(), spelling.len()))
        );
        driver.keystroke(
            ui_events::keyboard::Key::Named(NamedKey::Home),
            ui_events::keyboard::Modifiers::SHIFT,
        );
        assert!(driver.ime(puri::handler::ImeEvent::Commit(edit.into())));
        assert_eq!(driver.value(&path), changed);
    }
}

#[test]
fn an_empty_document_s_root_offers_starting_points_then_raw_s() {
    let doc = Document::default();
    let displays = |query: &str| {
        projected_completion_entries(
            &doc,
            &crate::selection::pending_with_query(&crate::test_root(), vec![], query),
        )
        .into_iter()
        .map(|entry| entry.display)
        .collect::<Vec<_>>()
    };
    assert_eq!(
        displays(""),
        ["fidget", "grap", "new cell", "new list", "new record"]
    );
    let selection = crate::selection::pending_with_query(&crate::test_root(), vec![], "sdf");
    let entries = projected_completion_entries(&doc, &selection);
    assert_eq!(entries[0].display, "fidget");
    let world = activate_projected(&doc, &selection, &entries[0]);
    assert!(
        world
            .model
            .doc
            .root
            .as_ref()
            .and_then(|root| crate::test_support::value_at(
                root,
                &[
                    gid::Step::Key(crate::workspace::vocabulary::MAIN),
                    gid::Step::Key(fidget::vocabulary::FIDGET)
                ]
            ))
            .and_then(Value::as_cell)
            .is_some()
    );
    assert!(!world.model.selection.as_ref().unwrap().path().is_empty());
    assert!(entries.iter().any(|entry| entry.display == "\"sdf\""));
    // Below the root, a hole is Raw's alone.
    let list = Document {
        root: Some(Value::list([])),
        cells: gid::Cells::new(),
    };
    let element = vec![Step::Element(gid::position::between(None, None).unwrap())];
    let entries = projected_completion_entries(
        &list,
        &crate::selection::pending_with_query(&crate::test_root(), element, ""),
    );
    assert_eq!(entries[0].display, "new cell");
}

#[test]
fn only_the_active_empty_requests_completion_offers() {
    use crate::display::descend;

    let fields = std::array::from_fn::<_, 32, _>(|_| new_cell_id());
    let requests = Rc::new(std::cell::Cell::new(0));
    let provider: crate::display::CompletionProvider = {
        let requests = requests.clone();
        Rc::new(move |_| {
            requests.set(requests.get() + 1);
            Some(vec![crate::libraries::completion::insert(
                "offered",
                Value::record([]),
                None,
            )])
        })
    };
    // Each of the record's empty fields is a hole the projection supplies.
    let hole = crate::display::partial(move |input| {
        input.value.is_none().then(|| {
            crate::display::picker(crate::display::Picker::new(
                CompletionKind::Value,
                crate::display::ranked(provider.clone()),
            ))
        })
    });
    let projection = Projection::new([crate::display::partial(move |input| {
        input.value?.as_record()?;
        Some(crate::display::col(
            0,
            0.0,
            fields.map(|field| descend([gid::Step::Key(field)], Some(hole.clone()), None)),
        ))
    })]);
    let document = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let selection = pending_value(&crate::test_root(), vec![Step::Key(fields[12])]);
    let mut world = crate::test_editor(document);
    world.stack.projection = projection;
    let entries = offered(&mut world, &selection);
    assert_eq!(requests.get(), 1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].display, "offered");
}

#[test]
fn root_completions_do_not_leak_into_nested_pending_values() {
    let template = |value: Option<Value>| {
        value
            .as_ref()
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&crate::workspace::vocabulary::MAIN))
            .and_then(Value::as_record)
            .is_some_and(|fields| {
                fields.contains_key(&crate::libraries::grap::vocabulary::GRAP)
                    || fields.contains_key(&fidget::vocabulary::FIDGET)
            })
    };
    let empty = Document::default();
    let root_selection = crate::selection::pending_with_query(&crate::test_root(), Vec::new(), "");
    let root_entries = projected_completion_entries(&empty, &root_selection);
    assert!(root_entries.iter().any(|entry| template(inserted_value(
        &empty,
        &root_selection,
        entry
    ))));

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
    assert!(!nested_entries.iter().any(|entry| template(inserted_value(
        &nested,
        &nested_selection,
        entry
    ))));
    assert!(
        nested_entries
            .iter()
            .any(|entry| inserted_value(&nested, &nested_selection, entry) == Some(Value::list([])))
    );
}

#[test]
fn missing_call_argument_slots_edit_real_fields_without_materializing_other_arguments() {
    let function = new_cell_id();
    let arguments = [new_cell_id(), new_cell_id(), new_cell_id()];
    let mut cells = Cells::new();
    cells.set_value(function, grap::lambda(arguments, Value::record([])));
    let document = Document {
        root: Some(grap::call(function.into(), [])),
        cells,
    };
    for argument in arguments {
        let path = vec![Step::Key(argument)];
        let selected =
            crate::selection::pending_with_query(&crate::test_root(), path.clone(), "0.5");
        let entries = projected_completion_entries(&document, &selected);
        let offer = entries
            .iter()
            .find(|entry| entry.detail.as_deref() == Some("f64"))
            .unwrap();
        let world = activate_projected(&document, &selected, offer);
        assert_eq!(
            world.model.doc.root,
            Some(grap::call(function.into(), [(argument, f64::value(0.5))]))
        );
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
    }
    assert_eq!(document.root, Some(grap::call(function.into(), [])));
}
