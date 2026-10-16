use super::*;
use crate::libraries::control;
use gid::Document;

fn quote(value: Value) -> Value {
    grap::call(
        Value::from(control::vocabulary::QUOTE),
        [(grap::vocabulary::EXPRESSION, value)],
    )
}

#[test]
fn setters_return_unit_on_set_and_clear_while_missing_reads_are_specific() {
    let stack = crate::stack::load();
    for (get, set, reason) in [
        (
            site::vocabulary::GET,
            site::vocabulary::SET,
            site::vocabulary::NO_STATE,
        ),
        (
            selection_capability::vocabulary::GET,
            selection_capability::vocabulary::SET,
            selection_capability::vocabulary::NO_SELECTION,
        ),
    ] {
        let staged = RefCell::new(PendingChanges {
            annotation: None,
            annotation_changed: false,
            selection: None,
            selection_changed: false,
        });
        let foreign = |function,
                       context: &mut grap::Context<'_>,
                       call: &grap::Expression,
                       environment: &grap::Environment| {
            event_foreign(function, context, call, environment, &[], &staged)
        };
        let capabilities = [get, set];
        let scope = grap::ForeignOverlay::from_value(&capabilities, &foreign);
        let read = || {
            grap::evaluate_value_scoped(&grap::call(get.into(), []), &stack.libraries, &scope, 100)
                .result
        };
        assert_eq!(read(), absent::with_reason(reason));
        for payload in [
            Value::from(vec![42]),
            absent::with_reason(gid::new_cell_id()),
        ] {
            let expression = grap::call(
                set.into(),
                [
                    (
                        selection_capability::vocabulary::PATH,
                        path_data::value(&[]),
                    ),
                    (site::vocabulary::VALUE, payload.clone()),
                ],
            );
            let evaluation =
                grap::evaluate_value_scoped(&expression, &stack.libraries, &scope, 100);
            assert!(evaluation.completed);
            assert_eq!(evaluation.result, Value::record([]));
            assert_eq!(
                read(),
                if absent::is_absent(&payload) {
                    absent::with_reason(reason)
                } else {
                    payload
                }
            );
        }
    }
}

#[test]
fn capabilities_read_replace_and_clear_their_own_staged_value() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let annotation = Value::from(b"annotation".to_vec());
    let selection = Value::from(b"selection".to_vec());
    for (get, set, annotation_slot) in [
        (site::vocabulary::GET, site::vocabulary::SET, true),
        (
            selection_capability::vocabulary::GET,
            selection_capability::vocabulary::SET,
            false,
        ),
    ] {
        for clear in [false, true] {
            let function = grap::lambda(
                [],
                grap::call(
                    set.into(),
                    [
                        (
                            selection_capability::vocabulary::PATH,
                            grap::call(site::vocabulary::PATH.into(), []),
                        ),
                        (
                            site::vocabulary::VALUE,
                            if clear {
                                absent::with_reason(gid::new_cell_id())
                            } else {
                                grap::call(get.into(), [])
                            },
                        ),
                    ],
                ),
            );
            let staged = evaluate(
                &function,
                [],
                &[],
                Some(annotation.clone()),
                Some((vec![], selection.clone())),
                &sources,
                100,
            )
            .unwrap();
            assert_eq!(staged.annotation_changed, annotation_slot);
            assert_eq!(staged.selection_changed, !annotation_slot);
            assert_eq!(
                staged.annotation,
                (!(annotation_slot && clear)).then(|| annotation.clone())
            );
            assert_eq!(
                staged.selection,
                (!(!annotation_slot && clear)).then(|| (vec![], selection.clone()))
            );
        }
    }
}

#[test]
fn event_changes_commit_with_an_ordinary_absent_but_not_a_decline() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let annotation = Value::record([(site::vocabulary::FOLD, site::vocabulary::FOLDED.into())]);
    let selection = Value::record([]);
    let missing = gid::new_cell_id();
    let function = |result| {
        grap::lambda(
            [],
            grap::call(
                control::vocabulary::DO.into(),
                [(
                    control::vocabulary::EXPRESSIONS,
                    Value::list([
                        grap::call(
                            site::vocabulary::SET.into(),
                            [(site::vocabulary::VALUE, quote(annotation.clone()))],
                        ),
                        grap::call(
                            selection_capability::vocabulary::SET.into(),
                            [
                                (
                                    selection_capability::vocabulary::PATH,
                                    grap::call(site::vocabulary::PATH.into(), []),
                                ),
                                (site::vocabulary::VALUE, selection.clone()),
                            ],
                        ),
                        result,
                    ]),
                )],
            ),
        )
    };
    let accepted = function(missing.into());
    let staged = evaluate(&accepted, [], &[], None, None, &sources, 100).unwrap();
    assert!(staged.annotation_changed && staged.selection_changed);
    assert_eq!(staged.annotation, Some(annotation.clone()));
    assert_eq!(staged.selection, Some((vec![], selection.clone())));
    assert!(
        evaluate(
            &function(absent::decline()),
            [],
            &[],
            None,
            None,
            &sources,
            100
        )
        .is_none()
    );
    assert!(evaluate(&accepted, [], &[], None, None, &sources, 1).is_none());
}

fn set_selection(path: Value, payload: Value) -> Value {
    grap::call(
        selection_capability::vocabulary::SET.into(),
        [
            (selection_capability::vocabulary::PATH, path),
            (site::vocabulary::VALUE, payload),
        ],
    )
}

#[test]
fn declining_after_editing_discards_the_whole_editor_operation() {
    let mut stack = crate::stack::load();
    let function = gid::new_cell_id();
    let mut cells = gid::Cells::new();
    cells.set_value(
        function,
        sequence([
            set_selection(
                Value::list([]),
                crate::selection::payload::pending("discarded", 0),
            ),
            grap::call(
                site::vocabulary::SET.into(),
                [(site::vocabulary::VALUE, Value::from(vec![42]))],
            ),
            absent::decline(),
        ]),
    );
    let doc = Document {
        root: Some(Value::record([])),
        cells,
    };
    let mut fallback = gid::Cells::new();
    fallback.set_value(
        function,
        grap::lambda([], grap::call(site::vocabulary::GET.into(), [])),
    );
    stack.libraries.insert(
        gid::new_cell_id(),
        crate::libraries::Definitions::from_parts(fallback, Default::default()),
    );
    let original = Some((vec![], crate::selection::payload::edge()));
    let staged = evaluate(
        &function.into(),
        [],
        &[],
        None,
        original.clone(),
        &Sources {
            doc: &doc,
            libraries: &stack.libraries,
        },
        100,
    );
    assert!(staged.is_none());
}

fn sequence(expressions: impl IntoIterator<Item = Value>) -> Value {
    grap::lambda(
        [],
        grap::call(
            control::vocabulary::DO.into(),
            [(control::vocabulary::EXPRESSIONS, Value::list(expressions))],
        ),
    )
}

#[test]
fn selection_effect_moves_to_a_full_path_and_local_reads_observe_the_move() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let site_path = vec![gid::Step::Key(gid::new_cell_id())];
    let target = vec![
        gid::Step::Key(gid::new_cell_id()),
        gid::Step::Follow(gid::Resolution::Library(gid::new_cell_id())),
        gid::Step::Element(gid::position::between(None, None).unwrap()),
    ];
    let payload = crate::selection::payload::pending("", 0);
    let handler = sequence([
        set_selection(path_data::value(&target), payload.clone()),
        grap::call(
            site::vocabulary::SET.into(),
            [(
                site::vocabulary::VALUE,
                grap::call(selection_capability::vocabulary::GET.into(), []),
            )],
        ),
        Value::record([]),
    ]);
    let staged = evaluate(
        &handler,
        [],
        &site_path,
        Some(Value::record([])),
        Some((site_path.clone(), crate::selection::payload::edge())),
        &sources,
        100,
    )
    .unwrap();
    assert_eq!(staged.selection, Some((target, payload)));
    assert!(staged.selection_changed && staged.annotation_changed);
    assert_eq!(staged.annotation, None);

    let here = sequence([
        set_selection(
            grap::call(site::vocabulary::PATH.into(), []),
            crate::selection::payload::edge(),
        ),
        grap::call(selection_capability::vocabulary::GET.into(), []),
    ]);
    let staged = evaluate(&here, [], &site_path, None, None, &sources, 100).unwrap();
    assert_eq!(
        staged.selection,
        Some((site_path, crate::selection::payload::edge()))
    );
}

#[test]
fn clearing_another_location_does_not_clear_the_selection() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let selected = Some((
        vec![gid::Step::Key(gid::new_cell_id())],
        crate::selection::payload::edge(),
    ));
    let handler = sequence([
        set_selection(
            grap::call(site::vocabulary::PATH.into(), []),
            absent::with_reason(selection_capability::vocabulary::NO_SELECTION),
        ),
        Value::record([]),
    ]);
    let staged = evaluate(&handler, [], &[], None, selected.clone(), &sources, 100).unwrap();
    assert_eq!(staged.selection, selected);
    assert!(!staged.selection_changed);
}

#[test]
fn malformed_paths_do_not_stage_selection_effects() {
    let stack = crate::stack::load();
    let doc = Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let sources = Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let selected = Some((vec![], crate::selection::payload::edge()));
    let handler = sequence([
        set_selection(
            Value::list([Value::record([])]),
            crate::selection::payload::pending("", 0),
        ),
        Value::record([]),
    ]);
    let staged = evaluate(&handler, [], &[], None, selected.clone(), &sources, 100).unwrap();
    assert_eq!(staged.selection, selected);
    assert!(!staged.selection_changed);
}
