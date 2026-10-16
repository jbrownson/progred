//! Selection effects in a caller-supplied document and view. GET reads
//! the payload at the current projection site; SET takes an explicit
//! PATH and VALUE. The host supplies the implementation while dispatching.

use crate::libraries::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xbd9a8ecaa53276087806022499c6a61e);

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0x67de640ac4e9859c87668564bc008ea0);
    pub const SET: CellId = CellId::from_u128(0xa12f30681dc2a8465da7673a7e2da9a8);
    pub const PATH: CellId = CellId::from_u128(0xa1005961125b04b7285b8d5aa54a0566);
    pub const STAGE: CellId = CellId::from_u128(0x6a1fd3082b9c47e5f60d21a8c45e9b37);
    pub const EDGE: CellId = CellId::from_u128(0x2f74c8a1936e05bd4c17e2b98d60a5f4);
    pub const PENDING: CellId = CellId::from_u128(0x91d5e60b3a8f27c4058b39f6d2c471ea);
    pub const LABEL: CellId = CellId::from_u128(0x7be29f4680d1c5a3f2496e07b85d13c2);
    pub const NO_SELECTION: CellId = CellId::from_u128(0x3f6d3e25a51b978f0b607d6a8a4b8123);
}

pub fn edge() -> gid::Value {
    gid::Value::record([(vocabulary::STAGE, vocabulary::EDGE.into())])
}

pub fn pending() -> gid::Value {
    gid::Value::record([(vocabulary::STAGE, vocabulary::PENDING.into())])
}

pub fn at(path: &[gid::Step], payload: gid::Value) -> crate::site::Continuation {
    let path = path.to_vec();
    std::rc::Rc::new(move |_, site, staged| {
        staged.select(site.iter().chain(&path).cloned().collect(), payload.clone());
        true
    })
}

pub fn pending_at(path: &[gid::Step]) -> crate::site::Continuation {
    at(path, pending())
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
