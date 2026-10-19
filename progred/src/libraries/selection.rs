//! Selection effects in a caller-supplied document and view. GET reads
//! the payload at the current projection site; SET takes an explicit
//! PATH and VALUE. The editor defines these cells and the selection's
//! stages beside the code that interprets them, and supplies the
//! implementation while dispatching; this library names them.

use crate::display::Host;
use crate::libraries::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xbd9a8ecaa53276087806022499c6a61e);

pub mod vocabulary {
    pub use crate::selection::payload::vocabulary::{EDGE, LABEL, PENDING, STAGE};
    pub use crate::site::vocabulary::{
        NO_SELECTION, SELECTION_GET as GET, SELECTION_PATH as PATH, SELECTION_SET as SET,
    };
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "selection",
        [
            (vocabulary::GET, "selection get"),
            (vocabulary::SET, "selection set"),
            (vocabulary::PATH, "path"),
            (vocabulary::STAGE, "stage"),
            (vocabulary::EDGE, "edge"),
            (vocabulary::PENDING, "pending"),
            (vocabulary::LABEL, "label"),
            (vocabulary::NO_SELECTION, "no selection at this site"),
        ],
        ::grap::ForeignFunctions::default(),
    )
}

#[cfg(test)]
pub(crate) mod tests;
