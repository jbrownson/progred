//! Kinds: a record's `isa` field names the kind of thing it is, so it stays
//! that kind while it's unfinished. Grap's calls, lambdas and unquotes say
//! theirs; a record that doesn't is data. Shared like a name, so any
//! library's kinds can use it. An `isa` names a kind defined elsewhere, so
//! it reads as the kind's name; one with nothing in it offers the kinds the
//! document's library defines.
//!
//! A record no library draws otherwise reads by what it is and what it's
//! called, `def fraction {…}`: its kind, its name, and then its other
//! fields, those its kind's def lists first, in the def's order, each a place
//! to fill where the record lacks it, then any others. A field being added
//! offers the def's fields the record lacks.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::structure::Fields;
use crate::display::{
    Completion, CompletionKind, CompletionRequest, CompletionScope, Offered, Picker, descend,
    picker, ranked, row, selectable,
};
use crate::libraries::{Library, def, name};
use gid::{CellId, Step};
use std::rc::Rc;

pub const ID: CellId = CellId::from_u128(0x9f968bf188764d69af06cc2fd3c10909);

pub mod vocabulary {
    pub use ::grap::isa::ISA;
}

// The convention itself lives in Grap, which tells its code from data by
// it; this library remains its owner in vocabulary.

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "isa",
        [(vocabulary::ISA, "isa")],
        ::grap::ForeignFunctions::default(),
    )
    .with_projection(crate::display::runtime_partial(kind))
    .with_fallback(crate::display::runtime_partial(|input| {
        let defined = defined_fields(input.value?, &|cell| {
            Some(input.env.resolve(cell)?.value.into())
        });
        // A name its kind lists goes in the heading, not among the fields.
        let named = defined.contains(&name::vocabulary::NAME);
        let defined: Vec<_> = defined
            .into_iter()
            .filter(|field| *field != name::vocabulary::NAME)
            .collect();
        form(
            input,
            Fields::default().leading(defined.clone()).slots(defined),
            named,
        )
    }))
    .with_completions(completions)
}

/// The fields `record`'s kind's def lists, in order; none where its kind has
/// no def.
fn defined_fields(
    record: &::grap::RuntimeValue,
    resolve: &dyn Fn(CellId) -> Option<::grap::RuntimeValue>,
) -> Vec<CellId> {
    record
        .kind()
        .and_then(resolve)
        .and_then(|kind| def::Def::read(&kind))
        .map(|def| def.fields)
        .unwrap_or_default()
}

/// A record by its kind and its name, either where it lacks the other, then
/// its other fields as `fields` draws them, between braces, which show even
/// where there are none, so a record that's only a name reads as a record,
/// `button {}`. They fold, as a record's do, Space folding them while the
/// record is selected. Where it's `named`, as its kind says its
/// records are, a missing name is a place to type one, showing `name` dim.
/// A record with neither declines. The kind and name are stops of their
/// own, inside the record's.
pub(crate) fn form<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    fields: Fields<W>,
    named: bool,
) -> Option<Layout<W>> {
    let value = input.value?;
    value.record_keys()?;
    let kind = value
        .contains_field(vocabulary::ISA)
        .then(|| descend([Step::Key(vocabulary::ISA)], None, None));
    let name = (named || value.contains_field(name::vocabulary::NAME)).then(|| {
        descend(
            [Step::Key(name::vocabulary::NAME)],
            Some(crate::display::first(
                input,
                name::projection(Some(crate::identity::name_or_short_id(
                    input
                        .env
                        .name(name::vocabulary::NAME)
                        .map(|name| name.spelling().into_owned()),
                    name::vocabulary::NAME,
                ))),
            )),
            None,
        )
    });
    let heading: Vec<_> = kind.into_iter().chain(name).collect();
    (!heading.is_empty()).then_some(())?;
    let rest = fields.shown_empty().rest(input, |key| {
        key == vocabulary::ISA || key == name::vocabulary::NAME
    })?;
    // The braces fold, to `kind name {…}`, as a record's do.
    let rest = crate::display::fold::starting_open(crate::display::Delim::Brace, rest);
    Some(crate::display::structure::authoring(selectable(
        crate::display::hug(row(6.0, heading), rest, 6.0),
        &input.site(),
    )))
}

/// A field being added to a record of a kind offers the fields its def lists
/// that the record lacks, in the def's order.
fn completions(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.kind == CompletionKind::Field && request.scope == CompletionScope::Literal)
        .then_some(())?;
    let record = request.value()?;
    let fields = record.as_record()?;
    let defined = defined_fields(&record.clone().into(), &|cell| {
        Some((request.resolve)(cell)?.value.clone().into())
    });
    Some(
        defined
            .into_iter()
            .filter(|field| fields.get(field).is_none())
            .map(|field| {
                Completion::new(field, Offered::Value(field.into()), move |session| {
                    if let Some(picking) = session.pending() {
                        picking.commit_label(field, None, Some(crate::selection::pending_at(&[])));
                    }
                })
                .with_detail("field")
            })
            .collect(),
    )
}

/// A record's `isa`: the kind's name, or where it's empty, a place for a
/// kind, offering the kinds the document's libraries define, then those the
/// loaded libraries do, each saying whose it is.
fn kind<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    (input.env.occurrence()?.steps().next()? == &crate::occurrence::Step::Key(vocabulary::ISA))
        .then_some(())?;
    if input.value.is_some() {
        return name::shallow_cell(input);
    }
    Some(picker(Picker::new(
        CompletionKind::Value,
        ranked(Rc::new(|request| {
            let resolve = |cell| (request.resolve)(cell);
            let own = def::document_defs(request.root, &|cell| Some(resolve(cell)?.value));
            let loaded = (request.loaded)().into_iter().filter(|cell| {
                !own.contains(cell)
                    && resolve(*cell)
                        .and_then(|definition| definition.value.as_record())
                        .and_then(::grap::isa::of)
                        == Some(def::vocabulary::DEF)
            });
            Some(
                own.iter()
                    .copied()
                    .chain(loaded)
                    .map(|kind| {
                        let offer = crate::libraries::completion::select(kind, kind.into());
                        match resolve(kind).and_then(|definition| definition.library) {
                            Some(library) => offer.with_detail(library),
                            None => offer.with_detail("kind"),
                        }
                    })
                    .collect(),
            )
        })),
    )))
}

#[cfg(test)]
#[path = "tests/isa.rs"]
mod tests;
