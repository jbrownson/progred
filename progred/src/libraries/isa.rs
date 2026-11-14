//! Kinds: a record's `isa` field names the kind of thing it is, so it stays
//! that kind while it's unfinished. Grap's calls, lambdas and unquotes say
//! theirs; a record that doesn't is data. Shared like a name, so any
//! library's kinds can use it. An `isa` with nothing in it offers the kinds
//! the document's library defines.

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
    .with_projection(crate::display::runtime_partial(kind_place))
}

/// A record's empty `isa`: a place for a kind, offering the kinds the
/// document's library defines.
fn kind_place<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    input.value.is_none().then_some(())?;
    (input.env.occurrence()?.steps().next()? == &crate::occurrence::Step::Key(vocabulary::ISA))
        .then_some(())?;
    Some(picker(Picker::new(
        CompletionKind::Value,
        ranked(Rc::new(|request| {
            Some(
                crate::libraries::def::document_defs(request.root)
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
