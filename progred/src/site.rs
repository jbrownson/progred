//! Interpret site and selection effects while applying a Grap callable.
//! Reads observe staged writes; a declined handler commits neither.

use crate::occurrence::{Path, Step};
use crate::selection::Selection;
use crate::sources::Sources;
use crate::workspace::View;
use gid::Value;
use grap::absent;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) mod event;

/// The functions an event handler calls at its projection site, which this
/// module implements; the site and selection libraries name them.
pub mod vocabulary {
    use gid::CellId;

    /// Per-site state: GET reads it, SET stores VALUE, evaluated, and PATH
    /// answers the site's path as data.
    pub const GET: CellId = CellId::from_u128(0xbba7ddab82fa7d703c90b250785e3273);
    pub const SET: CellId = CellId::from_u128(0xd9329d07bcdc791919f5252846fa152c);
    pub const VALUE: CellId = CellId::from_u128(0x544d3b52ea73cd263c435ecddfe5e8bf);
    pub const PATH: CellId = CellId::from_u128(0x803e2b0d0c621eb9688569d0879c3b23);
    pub const NO_STATE: CellId = CellId::from_u128(0xee0b3857bf9f615333e0b0f5feebd92e);

    /// The selection: SELECTION_GET reads its payload at the site, and
    /// SELECTION_SET selects SELECTION_PATH with VALUE.
    pub const SELECTION_GET: CellId = CellId::from_u128(0x67de640ac4e9859c87668564bc008ea0);
    pub const SELECTION_SET: CellId = CellId::from_u128(0xa12f30681dc2a8465da7673a7e2da9a8);
    pub const SELECTION_PATH: CellId = CellId::from_u128(0xa1005961125b04b7285b8d5aa54a0566);
    pub const NO_SELECTION: CellId = CellId::from_u128(0x3f6d3e25a51b978f0b607d6a8a4b8123);
}

#[derive(Debug, PartialEq)]
pub struct PendingChanges {
    pub annotation: Option<Value>,
    pub annotation_changed: bool,
    pub selection: Option<(Path, Value)>,
    pub selection_changed: bool,
}

pub type Continuation = Rc<dyn Fn(&crate::editing::Read<'_>, &[Step], &mut PendingChanges) -> bool>;

impl PendingChanges {
    pub fn select(&mut self, path: Path, payload: Value) {
        if !absent::is_absent(&payload) {
            self.selection = Some((path, payload));
            self.selection_changed = true;
        } else if self
            .selection
            .as_ref()
            .is_some_and(|(selected, _)| selected == &path)
        {
            self.selection = None;
            self.selection_changed = true;
        }
    }
}

pub fn grap(
    function: impl Into<grap::RuntimeValue>,
    arguments: impl IntoIterator<Item = (gid::CellId, Value)> + Clone,
) -> impl Fn(&crate::editing::Read<'_>, &[Step], &mut PendingChanges) -> bool {
    let function = function.into();
    handler(move |sources, path, annotation, selection| {
        evaluate_with(path, annotation, selection, |overlay| {
            grap::apply_expression(
                &function,
                arguments.clone().into_iter().map(|(k, v)| (k, v.into())),
                sources,
                Some(overlay),
                crate::sources::DEFAULT_FUEL,
            )
        })
    })
}

fn handler(
    evaluate: impl Fn(
        &Sources<'_>,
        &[Step],
        Option<Value>,
        Option<(Path, Value)>,
    ) -> Option<PendingChanges>,
) -> impl Fn(&crate::editing::Read<'_>, &[Step], &mut PendingChanges) -> bool {
    move |sources, path, staged| match evaluate(
        &sources.sources,
        path,
        staged.annotation.clone(),
        staged.selection.clone(),
    ) {
        Some(next) => {
            staged.annotation = next.annotation;
            staged.annotation_changed |= next.annotation_changed;
            staged.selection = next.selection;
            staged.selection_changed |= next.selection_changed;
            true
        }
        None => false,
    }
}

const EVENT_FUNCTIONS: [gid::CellId; 5] = [
    vocabulary::GET,
    vocabulary::SET,
    vocabulary::PATH,
    vocabulary::SELECTION_GET,
    vocabulary::SELECTION_SET,
];

/// Apply one event handler with its get/set functions bound to this
/// projection site. Explicit decline or evaluator halt returns without
/// committing any pending annotation or selection change.
pub(crate) fn apply_scoped_event(
    session: &mut crate::editing::Session<'_>,
    view: &Rc<View>,
    path: Path,
    function: grap::RuntimeValue,
    event: Value,
) -> bool {
    let current = session
        .model
        .selection
        .as_ref()
        .filter(|selection| selection.view() == view)
        .map(|selection| (selection.path().to_vec(), selection.payload()));
    let annotation = session
        .model
        .workspace
        .pane_at(view, &path)
        .and_then(|state| state.annotations.at(&path))
        .cloned();
    let mut staged = PendingChanges {
        annotation,
        annotation_changed: false,
        selection: current,
        selection_changed: false,
    };
    let handled = grap(function, [(event::vocabulary::EVENT, event)])(
        &crate::editing::Read {
            sources: session.sources(),
        },
        &path,
        &mut staged,
    );
    let model = &mut *session.model;
    if handled && let Some(state) = model.workspace.pane_at_mut(view, &path) {
        install_scoped(
            staged,
            &Sources {
                doc: &model.doc,
                libraries: session.libraries,
            },
            view,
            &path,
            &mut state.annotations,
            &mut model.selection,
        );
        true
    } else {
        false
    }
}

pub(crate) fn install_scoped(
    staged: PendingChanges,
    sources: &Sources,
    view: &Rc<View>,
    path: &[Step],
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
                    selection.view() == view && selection.path() == path && selection.recorded()
                });
                let mut next = Selection::from_payload(sources, view, path, payload);
                next.set_recorded(recorded);
                *selection = Some(next);
            }
            None if selection
                .as_ref()
                .is_some_and(|selection| selection.view() == view) =>
            {
                *selection = None;
            }
            None => {}
        }
    }
}

fn evaluate_with(
    path: &[Step],
    annotation: Option<Value>,
    selection: Option<(Path, Value)>,
    apply: impl FnOnce(&grap::ForeignOverlay<'_>) -> grap::Evaluation,
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
                    call: &grap::Expression,
                    environment: &grap::Environment| {
            event_foreign(function, context, call, environment, path, &staged)
        };
        let overlay = grap::ForeignOverlay::from_value(&EVENT_FUNCTIONS, &call);
        apply(&overlay)
    };
    (evaluation.completed && !evaluation.result.declines()).then(|| staged.into_inner())
}

fn event_foreign(
    function: gid::CellId,
    context: &mut grap::Context,
    call: &grap::Expression,
    environment: &grap::Environment,
    path: &[Step],
    staged: &RefCell<PendingChanges>,
) -> Result<Value, grap::Halt> {
    match function {
        vocabulary::PATH => Ok(crate::occurrence::value(path)),
        vocabulary::GET => Ok(staged
            .borrow()
            .annotation
            .clone()
            .unwrap_or_else(|| absent::value(vocabulary::NO_STATE))),
        vocabulary::SET => set_state(context, call, environment, staged),
        vocabulary::SELECTION_GET => Ok(staged
            .borrow()
            .selection
            .as_ref()
            .filter(|(selected_path, _)| selected_path == path)
            .map(|(_, payload)| payload.clone())
            .unwrap_or_else(|| absent::value(vocabulary::NO_SELECTION))),
        vocabulary::SELECTION_SET => set_selection(context, call, environment, staged),
        _ => unreachable!("only site and selection capabilities are installed in this scope"),
    }
}

/// SET: stages VALUE, evaluated, as the site's state, or clears it when
/// VALUE is absent.
fn set_state(
    context: &mut grap::Context,
    call: &grap::Expression,
    environment: &grap::Environment,
    staged: &RefCell<PendingChanges>,
) -> Result<Value, grap::Halt> {
    let Some(expression) = context.field(call, vocabulary::VALUE) else {
        return Ok(context.missing_argument(call, vocabulary::VALUE));
    };
    let value = context.eval_to_value(expression, environment)?;
    let value = (!absent::is_absent(&value)).then_some(value.clone());
    Ok(context.effect(|| {
        let mut staged = staged.borrow_mut();
        staged.annotation = value;
        staged.annotation_changed = true;
        Value::record([])
    }))
}

/// SELECTION_SET: stages selecting SELECTION_PATH with VALUE, or, VALUE
/// absent, deselecting it if it is selected.
fn set_selection(
    context: &mut grap::Context,
    call: &grap::Expression,
    environment: &grap::Environment,
    staged: &RefCell<PendingChanges>,
) -> Result<Value, grap::Halt> {
    let Some(expression) = context.field(call, vocabulary::SELECTION_PATH) else {
        return Ok(context.missing_argument(call, vocabulary::SELECTION_PATH));
    };
    let encoded = context.eval_to_value(expression, environment)?;
    let Some(path) = crate::occurrence::from_value(&encoded) else {
        return Ok(absent::with_detail(
            grap::path::vocabulary::INVALID_PATH,
            vocabulary::VALUE,
            encoded,
        ));
    };
    let Some(expression) = context.field(call, vocabulary::VALUE) else {
        return Ok(context.missing_argument(call, vocabulary::VALUE));
    };
    let value = context.eval_to_value(expression, environment)?;
    let mut staged = staged.borrow_mut();
    if !absent::is_absent(&value)
        || staged
            .selection
            .as_ref()
            .is_some_and(|(selected, _)| selected == &path)
    {
        context.effect(|| {
            staged.select(path, value.clone());
        });
    }
    Ok(Value::record([]))
}

#[cfg(test)]
#[path = "tests/site.rs"]
mod tests;
