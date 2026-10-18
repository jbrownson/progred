//! Per-site access to the editor's annotation trie. GET and SET never
//! take a path: the host supplies them while dispatching an event
//! at a projection site. PATH exposes that site's document path as data.
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
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "get"),
        (vocabulary::SET, "set"),
        (vocabulary::PATH, "site path"),
        (vocabulary::VALUE, "value"),
        (vocabulary::FOLD, "fold"),
        (vocabulary::FOLDED, "folded"),
        (vocabulary::EXPANDED, "expanded"),
        (vocabulary::NO_STATE, "no site state"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    name::named_library(
        ID,
        "site",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
}

#[cfg(test)]
mod tests;
