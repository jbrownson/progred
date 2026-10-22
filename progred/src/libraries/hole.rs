//! Holes: a reference to the hole cell stands where a shape requires a value
//! that isn't written yet, so an unfinished call is still a call. A hole is
//! never offered: a constructor writes one where it can't yet write the
//! value, and deleting what a projection draws as required leaves one. The
//! editor treats a hole as missing wherever it's drawn, so its place shows
//! its picker, and Grap answers one with an absent whose reason is the hole.

use crate::display::Host;
use crate::libraries::{Library, name};
use gid::{CellId, Value};

pub const ID: CellId = CellId::from_u128(0x871bd524e6f74e2c996b862e8595d247);

pub mod vocabulary {
    pub use ::grap::hole::HOLE;
}

// The convention itself lives in Grap, which answers a hole with an absent;
// this library remains its owner in vocabulary.

/// A hole, as stored.
pub fn value() -> Value {
    vocabulary::HOLE.into()
}

/// Whether `value` is a hole.
pub fn is(value: &Value) -> bool {
    value.as_cell() == Some(vocabulary::HOLE)
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "hole",
        [(vocabulary::HOLE, "hole")],
        ::grap::ForeignFunctions::default(),
    )
}
