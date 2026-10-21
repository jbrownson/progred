//! A Grap notebook: a list of entry cells, each drawn as its expression and
//! its result. Entries are cells like any other, so one refers to another
//! the standard ways, in either direction, and a change to one recomputes
//! the entries that read it. They're drawn as a list, with a list's editing;
//! Enter on an entry starts a new one after it, and an empty notebook offers
//! its first.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{Completion, alternatives, col, indent, row, selectable, shared};
use gid::{Position, Resolution, Step, Value};
use puri::handler::HasHandler;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

pub mod vocabulary {
    use gid::CellId;

    /// A notebook's entries: a list of cells, each evaluated where it's
    /// drawn.
    pub const NOTEBOOK: CellId = CellId::from_u128(0x30371e11863404e70052ccff54276702);
}

/// A record naming a notebook draws its entries, then its other fields.
pub(super) fn display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    crate::display::structure::record_form(input, [vocabulary::NOTEBOOK], |[notebook]| {
        notebook?;
        Some(crate::display::descend(
            [Step::Key(vocabulary::NOTEBOOK)],
            crate::display::Projected::here(
                crate::display::runtime_partial(entries),
                &input.default_projection,
            ),
        ))
    })
}

/// The entries as a list, one under another, so inserting between them, the
/// hole a deleted one leaves, and the rest of a list's editing are a list's
/// own. Enter on an entry starts another beside it; an empty notebook offers
/// its first.
fn entries<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let positions = input.value?.list_positions()?;
    match positions.is_empty() && input.pending.is_none() {
        true => Some(first_entry(input.site())),
        false => crate::display::structure::list_column_with(
            input,
            12.0,
            Some(crate::display::runtime_partial(entry)),
            move |at, item| match positions.iter().position(|entry| *entry == at) {
                Some(index) => new_entry_on_enter(
                    item,
                    index.checked_sub(1).map(|before| positions[before].clone()),
                    at,
                    positions.get(index + 1).cloned(),
                ),
                None => item,
            },
        ),
    }
}

/// A click on what an empty notebook draws starts its first entry.
fn first_entry<W: Host>(list: crate::editing::Site) -> Layout<W> {
    let target = W::Hover::from(list.hover());
    let start: crate::display::Action =
        Rc::new(move |session| start_entry(session, &list, None, None));
    crate::display::widget::hover::on_hover(
        crate::display::widget::interaction::on_activate(
            crate::display::dim("new entry"),
            target.clone(),
            start,
        ),
        target,
    )
}

/// An entry, or the place for one the list leaves to fill.
fn entry<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    match input.value {
        Some(reference) => entry_of(input, reference),
        None => Some(new_entry_hole()),
    }
}

/// A place for an entry, as between two or where one was deleted: its one
/// offer starts an entry there, a new cell whose expression starts from
/// what was typed.
fn new_entry_hole<W: Host>() -> Layout<W> {
    crate::display::picker(crate::display::Picker::new(
        crate::display::CompletionKind::Value,
        Rc::new(|request| {
            let typed = request.query.to_owned();
            vec![crate::display::widget::offers::entry(
                request,
                Completion::new("new entry", move |session| {
                    if let Some(picking) = session.pending() {
                        picking.commit(
                            gid::new_cell_id().into(),
                            Some(crate::selection::at(
                                &[Step::Follow(Resolution::Document)],
                                crate::selection::payload::pending(&typed),
                            )),
                        );
                    }
                }),
            )]
        }),
    ))
}

/// An entry is its cell's expression, then, once the cell has one, what it
/// evaluates to, beside it or below it where there's no room.
fn entry_of<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    reference: &::grap::RuntimeValue,
) -> Option<Layout<W>> {
    let definition = input.env.resolve(reference.as_cell()?);
    let source = definition
        .as_ref()
        .map_or(Resolution::Document, |definition| definition.resolution);
    let expression = super::expression_path([Step::Follow(source)], &input.default_projection);
    if definition.is_none() {
        return Some(expression);
    }
    let result = crate::display::at_evaluated(
        [Step::Key(
            crate::libraries::presentation::vocabulary::RESULT,
        )],
        input
            .env
            .evaluate_runtime_memo(reference, ::grap::DEFAULT_FUEL, &[]),
        input.env.evaluation(reference, ::grap::DEFAULT_FUEL, &[]),
    );
    let site = input.site();
    let (expression, arrow, result) = (
        shared(expression),
        shared(selectable(crate::display::arrow(), &site)),
        shared(result),
    );
    Some(alternatives([
        row(6.0, [expression.clone(), arrow.clone(), result.clone()]),
        col(
            0,
            2.0,
            [row(6.0, [expression, arrow]), indent(20.0, result)],
        ),
    ]))
}

/// Enter on the entry at `at`, after its own handlers decline it, starts a
/// new entry after it, or with Shift before it, between its neighbours.
fn new_entry_on_enter<W: Host>(
    entry: Layout<W>,
    before: Option<Position>,
    at: Position,
    after: Option<Position>,
) -> Layout<W> {
    crate::display::widget::around(
        entry,
        Rc::new(move |context| {
            let list = context.site();
            let (before, at, after) = (before.clone(), at.clone(), after.clone());
            Box::new(move |child| {
                crate::display::widget::before_place(child, move |_, output| {
                    output.handler().on_key(move |world: &mut W, event| {
                        new_entry(
                            &mut world.session(),
                            event,
                            &list,
                            before.as_ref(),
                            &at,
                            after.as_ref(),
                        )
                    });
                })
            })
        }),
    )
}

fn new_entry(
    session: &mut crate::editing::Session<'_>,
    event: &KeyboardEvent,
    list: &crate::editing::Site,
    before: Option<&Position>,
    at: &Position,
    after: Option<&Position>,
) -> bool {
    let entered = event.state.is_down()
        && event.key == Key::Named(NamedKey::Enter)
        && crate::modifiers::at_most_shift(&event.modifiers)
        && list
            .below(&[Step::Element(at.clone())])
            .open(session)
            .rests_within();
    if !entered {
        false
    } else if event.modifiers.shift() {
        start_entry(session, list, before, Some(at))
    } else {
        start_entry(session, list, Some(at), after)
    }
}

/// Starts an entry between `low` and `high`, a new cell whose expression is
/// picked next.
fn start_entry(
    session: &mut crate::editing::Session<'_>,
    list: &crate::editing::Site,
    low: Option<&Position>,
    high: Option<&Position>,
) -> bool {
    let Some(fresh) = gid::position::between(low, high) else {
        return false;
    };
    let entry = list.below(&[Step::Element(fresh)]);
    let mut edit = entry.open(session);
    edit.replace(gid::new_cell_id().into()) && {
        edit.pend(&[Step::Follow(Resolution::Document)]);
        true
    }
}

/// Starts an empty document as a notebook of one entry, whose expression is
/// picked next.
pub(super) fn starter() -> Completion {
    let first = gid::position::between(None, None).expect("an empty list has room");
    crate::libraries::completion::generated(
        vocabulary::NOTEBOOK,
        {
            let first = first.clone();
            move || {
                Value::record([(
                    vocabulary::NOTEBOOK,
                    Value::List(
                        [(first.clone(), gid::new_cell_id().into())]
                            .into_iter()
                            .collect(),
                    ),
                )])
            }
        },
        Some(crate::selection::pending_at(&[
            Step::Key(vocabulary::NOTEBOOK),
            Step::Element(first),
            Step::Follow(Resolution::Document),
        ])),
    )
    .with_detail(super::ID)
}

#[cfg(test)]
mod tests;
