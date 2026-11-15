//! A notebook: a bunch of things, one under another, each drawn as it would
//! be anywhere, a library as a library, a section as a section, a Grap
//! evaluation as its expression and its result. It says nothing about what
//! they are: the place for a new one is an ordinary empty place, offering
//! what the libraries offer there. They're drawn as a list, with a list's
//! editing; Enter on an entry opens a place for the next, and an empty
//! notebook offers its first.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{
    Completion, CompletionKind, CompletionRequest, CompletionScope, Picker, picker, ranked, row,
    selectable,
};
use crate::libraries::{Library, name};
use gid::{CellId, Position, Step};
use puri::handler::HasHandler;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

pub const ID: CellId = CellId::from_u128(0x133388d1fe4f37b924e8b2ea9247cffe);

pub mod vocabulary {
    use gid::CellId;

    /// A notebook's entries, in order.
    pub const ENTRIES: CellId = CellId::from_u128(0x26b6e7f1fc0d482da89e2a4835d2f789);

    pub mod kind {
        use gid::CellId;

        /// A notebook: `{isa: notebook, entries}`.
        pub const NOTEBOOK: CellId = CellId::from_u128(0x9111dde9a00f4106a817720187b45645);
    }
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "notebook",
        [
            (vocabulary::kind::NOTEBOOK, "notebook"),
            (vocabulary::ENTRIES, "entries"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_projections_for([(
        vocabulary::kind::NOTEBOOK,
        crate::display::runtime_partial(display),
    )])
    .with_completions(completions)
}

/// An empty document can start as a notebook, its first entry picked next.
fn completions(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.kind == CompletionKind::Value && request.scope == CompletionScope::Start)
        .then(|| vec![starter()])
}

/// A notebook draws its entries, then its other fields, after a handle of
/// its own. The record and its list of entries are one thing, the notebook;
/// each entry is a stop inside.
fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    crate::display::structure::kind_form(
        input,
        vocabulary::kind::NOTEBOOK,
        [vocabulary::ENTRIES],
        |[list]| {
            list?;
            Some(crate::display::part(
                [Step::Key(vocabulary::ENTRIES)],
                Some(crate::display::first(
                    input,
                    crate::display::runtime_partial(entries),
                )),
                None,
            ))
        },
    )
    .map(crate::display::handle)
}

/// The entries as a list, one under another, so inserting between them, the
/// hole a deleted one leaves, and the rest of a list's editing are a list's
/// own. Each is numbered in the list's margin, the number selecting it, and
/// a place for one being made is numbered where it will be, the entries
/// after it numbered as they will be. Enter on an entry starts another
/// beside it; an empty notebook offers its first.
fn entries<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let stored = input.value?.list_positions()?;
    let list = input.site();
    if stored.is_empty() && input.pending.is_none() {
        return Some(first_entry(list));
    }
    // What's numbered: the entries, and a place for one being made.
    let mut numbered_at = stored.clone();
    if let Some(crate::display::Pending::Child(Step::Element(pending))) = &input.pending
        && !numbered_at.contains(pending)
    {
        numbered_at.push(pending.clone());
        numbered_at.sort();
    }
    let digits = numbered_at.len().to_string().len();
    crate::display::structure::list_column_with(
        input,
        12.0,
        Some(crate::display::runtime_partial(entry)),
        move |at, item| {
            let Some(number) = numbered_at.iter().position(|entry| *entry == at) else {
                return item;
            };
            let number = crate::display::dim(numbered(number + 1, digits));
            match stored.iter().position(|entry| *entry == at) {
                Some(index) => row(
                    8.0,
                    [
                        selectable(number, &list.below(&[Step::Element(at.clone())])),
                        new_entry_on_enter(
                            item,
                            index.checked_sub(1).map(|before| stored[before].clone()),
                            at,
                            stored.get(index + 1).cloned(),
                        ),
                    ],
                ),
                // A place has nothing yet for its number to select.
                None => row(8.0, [number, item]),
            }
        },
    )
}

/// `number` as wide as `digits` digits, padded with figure spaces, so the
/// entries it numbers line up.
fn numbered(number: usize, digits: usize) -> String {
    let number = number.to_string();
    "\u{2007}".repeat(digits.saturating_sub(number.len())) + &number
}

/// A click on what an empty notebook draws opens a place for its first entry.
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

/// An entry is drawn as it would be anywhere; the place for one the list
/// leaves to fill offers what the libraries offer for an empty place, then
/// the editor's constructors and what's typed, read as a value.
fn entry<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    input.value.is_none().then(|| {
        picker(Picker::new(
            CompletionKind::Value,
            ranked(Rc::new(|request| {
                let everything = CompletionRequest {
                    scope: CompletionScope::Everything,
                    ..*request
                };
                Some(
                    request
                        .libraries
                        .and_then(|provider| provider(&everything))
                        .unwrap_or_default()
                        .into_iter()
                        .chain(crate::display::raw::constructors(request.kind))
                        .chain(crate::display::raw::typed(request))
                        .collect(),
                )
            })),
        ))
    })
}

/// Enter on the entry at `at`, after its own handlers decline it, opens a
/// place for a new entry after it, or with Shift before it, between its
/// neighbours.
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

/// Opens a place for an entry between `low` and `high`.
fn start_entry(
    session: &mut crate::editing::Session<'_>,
    list: &crate::editing::Site,
    low: Option<&Position>,
    high: Option<&Position>,
) -> bool {
    let Some(fresh) = gid::position::between(low, high) else {
        return false;
    };
    list.open(session).pend(&[Step::Element(fresh)]);
    true
}

/// Starts an empty document as a notebook, its first entry picked next.
fn starter() -> Completion {
    let first = gid::position::between(None, None).expect("an empty list has room");
    crate::libraries::completion::insert(
        vocabulary::kind::NOTEBOOK,
        ::grap::isa::record(
            vocabulary::kind::NOTEBOOK,
            [(vocabulary::ENTRIES, gid::Value::list([]))],
        ),
        Some(crate::selection::pending_at(&[
            Step::Key(vocabulary::ENTRIES),
            Step::Element(first),
        ])),
    )
    .with_detail(ID)
}

#[cfg(test)]
#[path = "tests/notebook.rs"]
mod tests;
