//! Kinds: a record's `isa` field names the kind of thing it is, so it stays
//! that kind while it's unfinished. Grap's calls, lambdas and unquotes say
//! theirs; a record that doesn't is data. Shared like a name, so any
//! library's kinds can use it.

use crate::display::Host;
use crate::libraries::{Library, name};
use gid::CellId;

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
}
