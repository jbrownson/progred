//! Selection effects in a caller-supplied document and view. GET reads
//! the payload at the current projection site; SET takes an explicit
//! PATH and VALUE. The host supplies the implementation while dispatching,
//! and owns the selection's stages; this library names them.

use crate::libraries::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xbd9a8ecaa53276087806022499c6a61e);

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0x67de640ac4e9859c87668564bc008ea0);
    pub const SET: CellId = CellId::from_u128(0xa12f30681dc2a8465da7673a7e2da9a8);
    pub const PATH: CellId = CellId::from_u128(0xa1005961125b04b7285b8d5aa54a0566);
    pub use crate::selection::payload::vocabulary::{EDGE, LABEL, PENDING, STAGE};
    pub const NO_SELECTION: CellId = CellId::from_u128(0x3f6d3e25a51b978f0b607d6a8a4b8123);
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "selection get"),
        (vocabulary::SET, "selection set"),
        (vocabulary::PATH, "path"),
        (vocabulary::STAGE, "stage"),
        (vocabulary::EDGE, "edge"),
        (vocabulary::PENDING, "pending"),
        (vocabulary::LABEL, "label"),
        (vocabulary::NO_SELECTION, "no selection at this site"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "selection",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
    )
}

#[cfg(test)]
pub(crate) mod tests;
