//! The document's workspace vocabulary: its panes and its own libraries. The
//! editor defines and interprets both at the root, and this library names
//! them; libraries can use the same ordinary data when constructing
//! templates.

use crate::display::Host;
use crate::libraries::{Library, name};
use gid::{CellId, Cells};

pub const ID: CellId = CellId::from_u128(0x7c295d8a64d3e257dc2c3932e43def74);

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LEFT, LIBRARIES, PANES, RIGHT};
}

pub fn library<W: Host>() -> Library<W> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::PANES, "panes"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::LIBRARIES, "libraries"),
        (vocabulary::KEYS, "keys"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    name::named_library(
        ID,
        "workspace",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
}
