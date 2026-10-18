//! Paths as ordinary GID data, interpreted in a caller-supplied root
//! and resolution context. List positions retain their session lifetime.

use crate::display::Host;
use crate::libraries::{Library, absent, name};
#[cfg(test)]
pub use ::grap::path::step_value;
pub use ::grap::path::{read, read_step, value};
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x7f18fc9a3362e8c7e4cd7e21501a71f6);

pub mod vocabulary {
    pub use ::grap::path::vocabulary::*;
}

pub fn library<W: Host>() -> Library<W> {
    use vocabulary::*;
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (KEY, "key"),
        (ELEMENT, "element"),
        (FOLLOW, "follow"),
        (DOCUMENT, "document"),
        (LIBRARY, "library"),
        (INDEXABLE, "indexable"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    cells.set_value(INVALID_PATH, absent::named_reason("invalid path"));
    name::named_library(
        ID,
        "path",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
}

#[cfg(test)]
mod tests;
