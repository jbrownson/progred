//! Apply a Grap callable at a document path with GET/SET closed over
//! that place. The path stays in Rust.

#[cfg(test)]
use crate::annotations::Annotations;
use crate::model::Selected;
use crate::selection::Selection;
#[cfg(test)]
use crate::sources::Sources;
use crate::App;
use gid::{Path, Value};
use progred_libraries::{absent, layout, selection as selection_capability, site};
use parley::{FontContext, LayoutContext};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use vello::peniko::Brush;

/// Apply one event handler transactionally with capabilities closed
/// over its projection site. An absent or diagnostic result declines
/// without committing any staged annotation or selection change.
pub fn apply_event(app: &mut App, path: Path, function: Value, event: Value) -> bool {
    let recorded = app
        .model
        .tree_selection()
        .filter(|selection| selection.path() == path)
        .is_some_and(Selection::recorded);
    let current = app
        .model
        .tree_selection()
        .filter(|selection| selection.path() == path)
        .map(|selection| selection.payload().clone());
    let annotation = Rc::new(RefCell::new(app.model.annotations.at(&path).cloned()));
    let annotation_changed = Rc::new(Cell::new(false));
    let selection = Rc::new(RefCell::new(current));
    let selection_changed = Rc::new(Cell::new(false));
    // Foreign functions are owned and therefore `'static`. Move the
    // app's caches into shared owners for this synchronous evaluation,
    // then put the same caches back when it concludes.
    let fonts = Rc::new(RefCell::new(std::mem::replace(
        &mut app.font_cx,
        FontContext::new(),
    )));
    let layouts = Rc::new(RefCell::new(std::mem::replace(
        &mut app.layout_cx,
        LayoutContext::<Brush>::new(),
    )));
    let evaluation = {
        let site = site::at(
            {
                let annotation = annotation.clone();
                move || annotation.borrow().clone()
            },
            {
                let annotation = annotation.clone();
                let annotation_changed = annotation_changed.clone();
                move |value| {
                    *annotation.borrow_mut() = value;
                    annotation_changed.set(true);
                }
            },
        );
        let selected = selection_capability::at(
            {
                let selection = selection.clone();
                move || selection.borrow().clone()
            },
            {
                let selection = selection.clone();
                let selection_changed = selection_changed.clone();
                move |value| {
                    *selection.borrow_mut() = value;
                    selection_changed.set(true);
                }
            },
        );
        grap::apply(
            &function,
            [(layout::vocabulary::EVENT, event)],
            |cell| app.sources().value(cell).cloned(),
            &app
                .stack
                .foreign
                .clone()
                .merge(site)
                .merge(selected)
                .merge(crate::line_edit::functions(fonts.clone(), layouts.clone())),
            grap::DEFAULT_FUEL,
        )
    };
    app.font_cx = Rc::try_unwrap(fonts)
        .unwrap_or_else(|_| panic!("line-edit font capability escaped dispatch"))
        .into_inner();
    app.layout_cx = Rc::try_unwrap(layouts)
        .unwrap_or_else(|_| panic!("line-edit layout capability escaped dispatch"))
        .into_inner();
    let handled = evaluation.diagnostics.is_empty() && !absent::is_absent(&evaluation.result);
    if handled {
        if annotation_changed.get() {
            app.model
                .annotations
                .set(&path, annotation.borrow().clone());
        }
        if selection_changed.get() {
            match selection.borrow().clone() {
                Some(payload) => {
                    let mut next = Selection::from_payload(
                        &app.sources(),
                        &app.stack.projection,
                        path,
                        payload,
                    );
                    next.preserve_recorded(recorded);
                    app.model.selection = Some(Selected::Tree(next));
                }
                None
                    if app
                        .model
                        .tree_selection()
                        .is_some_and(|selection| selection.path() == path) =>
                {
                    app.model.selection = None;
                }
                None => {}
            }
        }
    }
    handled
}

#[cfg(test)]
fn apply_at(
    path: &[gid::Step],
    annotations: &mut Annotations,
    function: &Value,
    sources: &Sources<'_>,
    foreign: &grap::ForeignFunctions,
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
            |cell| sources.value(cell).cloned(),
            &foreign.clone().merge(site),
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
            library: &stack.library,
        };
        let mut annotations = Annotations::default();
        let here = vec![Step::Follow];
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
        let evaluation = apply_at(
            &here,
            &mut annotations,
            &function,
            &sources,
            &stack.foreign,
        );
        assert!(evaluation.diagnostics.is_empty());
        assert_eq!(
            annotations.field(&here, annotations::FOLD),
            Some(&Value::from(annotations::FOLDED))
        );
        assert!(annotations.at(&elsewhere).is_none());
    }
}
