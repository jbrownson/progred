//! Interpret site and selection effects while applying a Grap callable.
//! Reads observe staged writes; a declined handler commits neither.

use crate::Editor;
use crate::selection::Selection;
use crate::sources::Sources;
use crate::workspace::Root;
use gid::{Path, Value};
use grap::Effects;
use progred_libraries::{
    absent, layout, path as path_data, selection as selection_capability, site,
};

#[derive(Clone)]
pub(crate) struct PendingChanges {
    pub annotation: Option<Value>,
    pub annotation_changed: bool,
    pub selection: Option<(Path, Value)>,
    pub selection_changed: bool,
}

const EVENT_FUNCTIONS: [gid::CellId; 5] = [
    site::vocabulary::GET,
    site::vocabulary::SET,
    site::vocabulary::PATH,
    selection_capability::vocabulary::GET,
    selection_capability::vocabulary::SET,
];

/// Apply one event handler with its get/set functions bound to this
/// projection site. Explicit decline or evaluator halt returns without
/// committing any pending annotation or selection change.
pub fn apply_event(
    app: &mut Editor,
    root: Root,
    path: Path,
    function: Value,
    event: Value,
) -> bool {
    let current = app
        .model
        .selection
        .as_ref()
        .filter(|selection| selection.root() == &root)
        .map(|selection| (selection.path().to_vec(), selection.payload()));
    let annotation = app
        .model
        .workspace
        .view(&root)
        .and_then(|view| view.annotations.at(&path))
        .cloned();
    let staged = evaluate(
        &function,
        [(layout::vocabulary::EVENT, event)],
        &path,
        annotation,
        current,
        &app.sources(),
        grap::DEFAULT_FUEL,
    );
    let handled = staged.is_some();
    if let Some(staged) = staged {
        let Some(view) = app.model.workspace.view_mut(&root) else {
            return false;
        };
        install(
            staged,
            &Sources {
                doc: &app.model.doc,
                libraries: &app.stack.libraries,
            },
            &root,
            &path,
            &mut view.annotations,
            &mut app.model.selection,
        );
    }
    handled
}

pub(crate) fn install(
    staged: PendingChanges,
    sources: &Sources,
    root: &Root,
    path: &[gid::Step],
    annotations: &mut crate::annotations::Annotations,
    selection: &mut Option<Selection>,
) {
    if staged.annotation_changed {
        annotations.set(path, staged.annotation);
    }
    if staged.selection_changed {
        match staged.selection {
            Some((path, payload)) => {
                let recorded = selection.as_ref().is_some_and(|selection| {
                    selection.root() == root && selection.path() == path && selection.recorded()
                });
                let mut next = Selection::from_payload(root, sources, path, payload);
                next.preserve_recorded(recorded);
                *selection = Some(next);
            }
            None if selection
                .as_ref()
                .is_some_and(|selection| selection.root() == root) =>
            {
                *selection = None;
            }
            None => {}
        }
    }
}

pub(crate) fn evaluate(
    function: &Value,
    arguments: impl IntoIterator<Item = (gid::CellId, Value)>,
    path: &[gid::Step],
    annotation: Option<Value>,
    selection: Option<(Path, Value)>,
    sources: &Sources<'_>,
    fuel: usize,
) -> Option<PendingChanges> {
    let staged = Effects::new(PendingChanges {
        annotation,
        annotation_changed: false,
        selection,
        selection_changed: false,
    });
    let evaluation = {
        let call = |function,
                    context: &mut grap::Context<'_>,
                    call: grap::Expression,
                    environment: &grap::Environment| {
            event_foreign(function, context, call, environment, path, &staged)
        };
        let overlay = grap::ForeignOverlay::new(&EVENT_FUNCTIONS, &call).with_effects(&staged);
        grap::apply_scoped(
            function,
            arguments,
            |cell| sources.grap_definitions(cell),
            &overlay,
            fuel,
        )
    };
    (evaluation.completed && !absent::declines(&evaluation.result)).then(|| staged.into_inner())
}

fn event_foreign(
    function: gid::CellId,
    context: &mut grap::Context,
    call: grap::Expression,
    environment: &grap::Environment,
    path: &[gid::Step],
    staged: &Effects<PendingChanges>,
) -> Result<Value, grap::Halt> {
    if function == site::vocabulary::PATH {
        return Ok(path_data::value(path));
    }
    if function == site::vocabulary::GET {
        return Ok(staged
            .borrow()
            .annotation
            .clone()
            .unwrap_or_else(absent::value));
    }
    if function == selection_capability::vocabulary::GET {
        return Ok(staged
            .borrow()
            .selection
            .as_ref()
            .filter(|(selected_path, _)| selected_path == path)
            .map(|(_, payload)| payload.clone())
            .unwrap_or_else(absent::value));
    }
    if function == selection_capability::vocabulary::SET {
        let Some(expression) = context.field(call, selection_capability::vocabulary::PATH) else {
            return Ok(context.missing_argument(selection_capability::vocabulary::PATH));
        };
        let encoded = context.eval(expression, environment)?;
        let Some(path) = path_data::read(&encoded) else {
            return Ok(grap::absent::with_detail(
                path_data::vocabulary::INVALID_PATH,
                site::vocabulary::VALUE,
                encoded,
            ));
        };
        let Some(expression) = context.field(call, site::vocabulary::VALUE) else {
            return Ok(context.missing_argument(site::vocabulary::VALUE));
        };
        let value = context.eval(expression, environment)?;
        let mut staged = staged.borrow_mut();
        if !absent::is_absent(&value) {
            staged.selection = Some((path, value.clone()));
            staged.selection_changed = true;
        } else if staged
            .selection
            .as_ref()
            .is_some_and(|(selected, _)| selected == &path)
        {
            staged.selection = None;
            staged.selection_changed = true;
        }
        return Ok(value);
    }
    if function == site::vocabulary::SET {
        let Some(expression) = context.field(call, site::vocabulary::VALUE) else {
            return Ok(context.missing_argument(site::vocabulary::VALUE));
        };
        let value = context.eval(expression, environment)?;
        let value = (!absent::is_absent(&value)).then_some(value.clone());
        let result = value.clone().unwrap_or_else(absent::value);
        let mut staged = staged.borrow_mut();
        staged.annotation = value;
        staged.annotation_changed = true;
        return Ok(result);
    }
    Ok(absent::value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::Document;
    use progred_libraries::control;

    fn quote(value: Value) -> Value {
        grap::call(
            Value::from(control::vocabulary::QUOTE),
            [(grap::vocabulary::EXPRESSION, value)],
        )
    }

    #[test]
    fn capabilities_read_replace_and_clear_their_own_staged_value() {
        let stack = crate::stack::load::<()>();
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
                                    absent::value()
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
    fn event_changes_commit_by_result_even_after_a_discarded_absent() {
        let stack = crate::stack::load::<()>();
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
                            missing.into(),
                            result,
                        ]),
                    )],
                ),
            )
        };
        let accepted = function(Value::record([]));
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
    fn a_declined_definition_leaves_no_selection_or_annotation_for_the_next() {
        let mut stack = crate::stack::load::<()>();
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
            Value::record([]),
            progred_libraries::Definitions::from_parts(fallback, Default::default()),
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
        )
        .unwrap();
        assert_eq!(staged.selection, original);
        assert_eq!(staged.annotation, None);
        assert!(!staged.selection_changed && !staged.annotation_changed);
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
        let stack = crate::stack::load::<()>();
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
        let stack = crate::stack::load::<()>();
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
                absent::value(),
            ),
            Value::record([]),
        ]);
        let staged = evaluate(&handler, [], &[], None, selected.clone(), &sources, 100).unwrap();
        assert_eq!(staged.selection, selected);
        assert!(!staged.selection_changed);
    }

    #[test]
    fn malformed_paths_do_not_stage_selection_effects() {
        let stack = crate::stack::load::<()>();
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
}
