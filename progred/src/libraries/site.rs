//! Per-site access to the editor's annotations. GET and SET never
//! take a path: the host supplies them while dispatching an event
//! at a projection site. PATH exposes the site's occurrence path in its
//! view as data.
//! The editor defines these cells and the fold states beside the code
//! that interprets them; this library names them.

use crate::display::Host;
use crate::libraries::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xab8d8d4b75ef3526d258d2689f95daba);

pub mod vocabulary {
    pub use crate::annotations::{EXPANDED, FOLD, FOLDED};
    pub use crate::site::vocabulary::{GET, NO_STATE, PATH, SET, VALUE};
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "site",
        [
            (vocabulary::GET, "get"),
            (vocabulary::SET, "set"),
            (vocabulary::PATH, "site path"),
            (vocabulary::VALUE, "value"),
            (vocabulary::FOLD, "fold"),
            (vocabulary::FOLDED, "folded"),
            (vocabulary::EXPANDED, "expanded"),
            (vocabulary::NO_STATE, "no site state"),
        ],
        ::grap::ForeignFunctions::default(),
    )
}

#[cfg(test)]
mod tests;
