//! Kinds: a record's `isa` field names the kind of thing it is, so it stays
//! that kind while it's unfinished. Grap's calls, lambdas and unquotes say
//! theirs; a record that doesn't is data. Shared like a name, so any
//! library's kinds can use it. An `isa` names a kind defined elsewhere, so
//! it reads as the kind's name; one with nothing in it offers the kinds the
//! document's library defines.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{CompletionKind, Picker, picker, ranked};
use crate::libraries::{Library, name};
use gid::CellId;
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
}

/// A record's `isa`: the kind's name, or where it's empty, a place for a
/// kind, offering the kinds the document's library defines.
fn kind<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    (input.env.occurrence()?.steps().next()? == &crate::occurrence::Step::Key(vocabulary::ISA))
        .then_some(())?;
    if input.value.is_some() {
        return name::shallow_cell(input);
    }
    Some(picker(Picker::new(
        CompletionKind::Value,
        ranked(Rc::new(|request| {
            Some(
                crate::libraries::def::document_defs(request.root, |cell| {
                    Some((request.resolve)(cell)?.value)
                })
                .into_iter()
                .map(|kind| {
                    crate::libraries::completion::select(kind, kind.into()).with_detail("kind")
                })
                .collect(),
            )
        })),
    )))
}

#[cfg(test)]
#[path = "tests/isa.rs"]
mod tests;
