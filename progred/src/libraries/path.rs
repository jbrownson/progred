//! Paths as ordinary GID data, interpreted in a caller-supplied root
//! and resolution context. List positions retain their session lifetime.

use crate::display::Host;
use crate::libraries::{Library, name};
pub use ::grap::path::{read, read_step, value};
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x7f18fc9a3362e8c7e4cd7e21501a71f6);

pub mod vocabulary {
    pub use ::grap::path::vocabulary::*;
}

pub fn library<W: Host>() -> Library<W> {
    use vocabulary::*;
    name::named_library(
        ID,
        "path",
        [
            (KEY, "key"),
            (ELEMENT, "element"),
            (FOLLOW, "follow"),
            (DOCUMENT, "document"),
            (LIBRARY, "library"),
            (INDEXABLE, "indexable"),
            (INVALID_PATH, "invalid path"),
        ],
        ::grap::ForeignFunctions::default(),
    )
}

#[cfg(test)]
mod tests;
