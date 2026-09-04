//! Apply a Grap callable at a document path with GET/SET closed over
//! that place. The path stays in Rust.

use crate::Editor;
#[cfg(test)]
use crate::annotations::Annotations;
use crate::selection::Selection;
use crate::sources::Sources;
use crate::workspace::Root;
use gid::{Path, Value};
use progred_libraries::{absent, layout, selection as selection_capability, site};
use std::cell::RefCell;
#[cfg(test)]
use std::rc::Rc;

struct PendingChanges {
    annotation: Option<Value>,
    annotation_changed: bool,
    selection: Option<Value>,
    selection_changed: bool,
}

const EVENT_FUNCTIONS: [gid::CellId; 4] = [
    site::vocabulary::GET,
    site::vocabulary::SET,
    selection_capability::vocabulary::GET,
    selection_capability::vocabulary::SET,
];

/// Apply one event handler with its get/set functions bound to this
/// projection site. An absent result declines without
/// committing any pending annotation or selection change.
pub fn apply_event(
    app: &mut Editor,
    root: Root,
    path: Path,
    function: Value,
    event: Value,
) -> bool {
    let recorded = app
        .model
        .selection
        .as_ref()
        .filter(|selection| selection.root() == &root && selection.path() == path)
        .is_some_and(Selection::recorded);
    let current = app
        .model
        .selection
        .as_ref()
        .filter(|selection| selection.root() == &root && selection.path() == path)
        .map(Selection::payload);
    let annotation = app
        .model
        .workspace
        .view(&root)
        .and_then(|view| view.annotations.at(&path))
        .cloned();
    let staged = evaluate_event(
        &function,
        event,
        annotation,
        current,
        &app.sources(),
        grap::DEFAULT_FUEL,
    );
    let handled = staged.is_some();
    if let Some(staged) = staged {
        if staged.annotation_changed {
            let Some(view) = app.model.workspace.view_mut(&root) else {
                return false;
            };
            view.annotations.set(&path, staged.annotation);
        }
        if staged.selection_changed {
            match staged.selection {
                Some(payload) => {
                    let mut next = Selection::from_payload(&app.sources(), path, payload)
                        .with_root(root.clone());
                    next.preserve_recorded(recorded);
                    app.model.selection = Some(next);
                }
                None if app.model.selection.as_ref().is_some_and(|selection| {
                    selection.root() == &root && selection.path() == path
                }) =>
                {
                    app.model.selection = None;
                }
                None => {}
            }
        }
    }
    handled
}

fn evaluate_event(
    function: &Value,
    event: Value,
    annotation: Option<Value>,
    selection: Option<Value>,
    sources: &Sources<'_>,
    fuel: usize,
) -> Option<PendingChanges> {
    let staged = RefCell::new(PendingChanges {
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
            event_foreign(function, context, call, environment, &staged)
        };
        let overlay = grap::ForeignOverlay::new(&EVENT_FUNCTIONS, &call);
        grap::apply_scoped(
            function,
            [(layout::vocabulary::EVENT, event)],
            |cell| sources.grap_definitions(cell),
            &overlay,
            fuel,
        )
    };
    (!absent::is_absent(&evaluation.result)).then(|| staged.into_inner())
}

fn event_foreign(
    function: gid::CellId,
    context: &mut grap::Context,
    call: grap::Expression,
    environment: &grap::Environment,
    staged: &RefCell<PendingChanges>,
) -> Result<Value, grap::Halt> {
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
            .clone()
            .unwrap_or_else(absent::value));
    }
    if function == site::vocabulary::SET || function == selection_capability::vocabulary::SET {
        let Some(expression) = context.field(call, site::vocabulary::VALUE) else {
            return Ok(context.missing_argument(site::vocabulary::VALUE));
        };
        let value = context.eval(expression, environment)?;
        let value = (!absent::is_absent(&value)).then_some(value.clone());
        let result = value.clone().unwrap_or_else(absent::value);
        let mut staged = staged.borrow_mut();
        if function == site::vocabulary::SET {
            staged.annotation = value;
            staged.annotation_changed = true;
        } else {
            staged.selection = value;
            staged.selection_changed = true;
        }
        return Ok(result);
    }
    Ok(absent::value())
}

#[cfg(test)]
fn apply_at(
    path: &[gid::Step],
    annotations: &mut Annotations,
    function: &Value,
    sources: &Sources<'_>,
) -> grap::Evaluation {
    let store = Rc::new(RefCell::new(std::mem::take(annotations)));
    let evaluation = {
        let site = site::at(
            {
                let store = store.clone();
                let path = path.to_vec();
                move || store.borrow().at(&path).cloned()
            },
            {
                let store = store.clone();
                let path = path.to_vec();
                move |value| store.borrow_mut().set(&path, value)
            },
        );
        grap::apply(
            function,
            [],
            |cell| {
                site.get(cell)
                    .cloned()
                    .map(grap::Definition::ForeignFunction)
                    .into_iter()
                    .chain(sources.grap_definitions(cell))
                    .collect()
            },
            grap::DEFAULT_FUEL,
        )
    };
    *annotations = match Rc::try_unwrap(store) {
        Ok(store) => store.into_inner(),
        Err(_) => panic!("site overlay dropped"),
    };
    evaluation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations;
    use gid::{Document, Step};
    use progred_libraries::control;

    fn quote(value: Value) -> Value {
        grap::call(
            Value::from(control::vocabulary::QUOTE),
            [(grap::vocabulary::EXPRESSION, value)],
        )
    }

    #[test]
    fn a_click_writes_the_closed_over_path_and_not_another() {
        let stack = crate::stack::load::<()>();
        let doc = Document {
            root: None,
            cells: gid::Cells::new(),
        };
        let sources = Sources {
            doc: &doc,
            libraries: &stack.libraries,
        };
        let mut annotations = Annotations::default();
        let here = vec![Step::Follow(gid::Resolution::Document)];
        let elsewhere = Vec::new();
        let function = grap::lambda(
            [],
            grap::call(
                Value::from(site::vocabulary::SET),
                [(
                    site::vocabulary::VALUE,
                    quote(Value::record([(
                        site::vocabulary::FOLD,
                        Value::from(site::vocabulary::FOLDED),
                    )])),
                )],
            ),
        );
        let evaluation = apply_at(&here, &mut annotations, &function, &sources);
        assert_eq!(Some(&evaluation.result), annotations.at(&here));

        assert_eq!(
            annotations.field(&here, annotations::FOLD),
            Some(&Value::from(annotations::FOLDED))
        );
        assert!(annotations.at(&elsewhere).is_none());
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
                                [(site::vocabulary::VALUE, selection.clone())],
                            ),
                            missing.into(),
                            result,
                        ]),
                    )],
                ),
            )
        };
        let accepted = function(Value::record([]));
        let staged =
            evaluate_event(&accepted, Value::record([]), None, None, &sources, 100).unwrap();
        assert!(staged.annotation_changed && staged.selection_changed);
        assert_eq!(staged.annotation, Some(annotation.clone()));
        assert_eq!(staged.selection, Some(selection.clone()));
        assert!(
            evaluate_event(
                &function(missing.into()),
                Value::record([]),
                None,
                None,
                &sources,
                100
            )
            .is_none()
        );
        assert!(evaluate_event(&accepted, Value::record([]), None, None, &sources, 1).is_none());
    }
}
