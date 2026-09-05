use super::*;
use progred_libraries::control;

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
    doc.cells.set_value(unnamed, crate::test_values::text("x"));
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
fn contextual_completion_starts_narrow_and_everything_widens_it() {
    let stack = crate::stack::load::<()>();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = src(&doc, &stack.libraries);
    let entries = crate::completion::completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "sdf",
        Some(&stack.root_completions),
        false,
    );
    assert_eq!(entries[0].display, "fidget");
    assert_eq!(activated(&entries[0]).value, Some(Value::record([])));
    assert!(activated(&entries[0]).on_commit.is_some());
    assert_eq!(entries.len(), 1);

    let widened = crate::completion::completion_entries_with(
        &sources,
        false,
        &Commit::Value(Rc::new(value_commit)),
        "sdf",
        Some(&stack.root_completions),
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
            && entry
                .detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("fidget · "))
    }));
}

#[test]
fn root_completions_open_the_domain_value_without_a_placeholder() {
    let stack = crate::stack::load::<()>();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let root = crate::workspace::Root::document();
    let pending = crate::selection::pending_value(&root, vec![]);
    let offers = (stack.root_completions)("");
    assert_eq!(
        offers
            .iter()
            .map(|offer| offer.display.as_str())
            .collect::<Vec<_>>(),
        ["fidget", "grap"]
    );
    for (offer, field) in offers.iter().zip([
        fidget::vocabulary::FIDGET,
        progred_libraries::grap::vocabulary::GRAP,
    ]) {
        let mut annotations = Annotations::default();
        let prepared = crate::completion::prepare(
            &src(&doc, &stack.libraries),
            &pending,
            &annotations,
            offer.value.clone(),
            None,
            offer.on_commit.as_ref(),
        )
        .unwrap();
        assert!(prepared.document_changed);
        assert_eq!(prepared.document.root, Some(Value::record([])));
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
        assert_eq!(selected.path(), [Step::Key(field)]);
        assert_eq!(selected.stage(), Stage::Pending);
        let choices = projected_completion_entries(&prepared.document, &selected);
        assert!(
            choices
                .iter()
                .any(|choice| activated(choice).value == Some(Value::record([])))
        );
        assert!(
            choices
                .iter()
                .any(|choice| activated(choice).value == Some(Value::list([])))
        );
        let filled = crate::completion::prepare(
            &src(&prepared.document, &stack.libraries),
            &selected,
            &annotations,
            Value::record([]),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            filled.document.root,
            Some(Value::record([(field, Value::record([]))]))
        );
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
    let offer = (stack.root_completions)("")
        .into_iter()
        .find(|offer| offer.display == "grap")
        .unwrap();
    let prepared = crate::completion::prepare(
        &src(&doc, &stack.libraries),
        &pending,
        &Annotations::default(),
        offer.value.clone(),
        None,
        offer.on_commit.as_ref(),
    )
    .unwrap();
    assert_eq!(
        prepared.effects.selection.as_ref().unwrap().0,
        [
            Step::Key(field),
            Step::Key(progred_libraries::grap::vocabulary::GRAP)
        ]
    );
    assert_eq!(
        prepared.document.root,
        Some(Value::record([(field, Value::record([]))]))
    );

    let declined = grap::lambda(
        [],
        grap::call(
            control::vocabulary::DO.into(),
            [(
                control::vocabulary::EXPRESSIONS,
                Value::list([
                    grap::call(offer.on_commit.unwrap(), []),
                    new_cell_id().into(),
                ]),
            )],
        ),
    );
    assert!(
        crate::completion::prepare(
            &src(&doc, &stack.libraries),
            &pending,
            &Annotations::default(),
            offer.value,
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
            root_completions: Some(&stack.root_completions),
            root_field_completions: Some(&stack.root_field_completions),
        },
        &mut tcx,
        Hooks {
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|_, _, _| {}),
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
            vec![Completion::new("offered", Value::record([]))]
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
        vec![
            progred_display::Completion {
                on_commit: None,
                display: "text".into(),
                detail: None,
                aliases: vec![],
                value: text::value("not a label"),
            },
            progred_display::Completion {
                on_commit: None,
                display: "existing".into(),
                detail: None,
                aliases: vec![],
                value: Value::from(existing),
            },
        ]
    });
    let labels = completion_entries_with(
        &sources,
        false,
        &Commit::Label(Rc::new(label_commit)),
        "",
        Some(&provider),
        false,
    );
    assert_eq!(labels.len(), 1);
    assert_eq!(activated(&labels[0]).label, Some((existing, None)));
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
